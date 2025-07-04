package agent

import (
	"context"
	"encoding/json"
	"errors"
	"strconv" // Added for Itoa
	"testing"

	"github.com/jarvis-go/internal/config"
	"github.com/sashabaranov/go-openai"
	"github.com/stretchr/testify/require"

	"github.com/mark3labs/mcp-go/mcp"
)

// This mirrors MCPClientInterface in agent.go
type mockMCPClient struct {
	InitializeFunc func(ctx context.Context, req mcp.InitializeRequest) (*mcp.InitializeResult, error)
	ListToolsFunc  func(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error)
	CallToolFunc   func(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error)
	CloseFunc      func() error
}

func (m *mockMCPClient) Initialize(ctx context.Context, req mcp.InitializeRequest) (*mcp.InitializeResult, error) {
	if m.InitializeFunc != nil {
		return m.InitializeFunc(ctx, req)
	}
	return &mcp.InitializeResult{}, nil
}

func (m *mockMCPClient) ListTools(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) {
	if m.ListToolsFunc != nil {
		return m.ListToolsFunc(ctx, req)
	}
	return &mcp.ListToolsResult{Tools: []mcp.Tool{}}, nil
}

func (m *mockMCPClient) CallTool(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error) {
	if m.CallToolFunc != nil {
		return m.CallToolFunc(ctx, request)
	}
	return &mcp.CallToolResult{
		Content: []mcp.Content{mcp.TextContent{Type: "text", Text: "mock default success for " + request.Params.Name}},
	}, nil
}

func (m *mockMCPClient) Close() error {
	if m.CloseFunc != nil {
		return m.CloseFunc()
	}
	return nil
}

type mockLLM struct {
	calls []openai.ChatCompletionResponse
	err   error
	// For validating tools passed to LLM if needed in future tests
	ReceivedTools []openai.Tool
}

func (m *mockLLM) CreateChatCompletion(ctx context.Context, r openai.ChatCompletionRequest) (openai.ChatCompletionResponse, error) {
	if m.err != nil {
		return openai.ChatCompletionResponse{}, m.err
	}
	if len(m.calls) == 0 {
		panic("mockLLM: no more responses configured for request: " + r.Messages[0].Content)
	}
	m.ReceivedTools = r.Tools // Store received tools for potential assertion
	resp := m.calls[0]
	m.calls = m.calls[1:]
	return resp, nil
}

// TestAgentProcess_LLMRespondsDirectly tests the scenario where the LLM responds directly without tool usage.
func TestAgentProcess_LLMRespondsDirectly(t *testing.T) {
	llmDirectResponse := "Hello, I am a helpful AI."
	cfg := config.Config{
		LLM:        config.LLMConfig{Model: "gpt"},
		MCPServers: []config.MCPServerConfig{}, // No MCP servers, so no tools for LLM
	}

	mockLLMClient := &mockLLM{
		calls: []openai.ChatCompletionResponse{
			{Choices: []openai.ChatCompletionChoice{{Message: openai.ChatCompletionMessage{Content: llmDirectResponse}}}},
		},
	}
	agentInstance := New(mockLLMClient, cfg)
	require.NotNil(t, agentInstance)
	// In the FSM model, New() populates availableLLMTools. If no servers, it's empty.
	require.Empty(t, agentInstance.availableLLMTools, "Agent should have no tools available if no MCP servers are configured.")

	out, err := agentInstance.Process(context.Background(), "User says hi")
	require.NoError(t, err)
	require.Equal(t, llmDirectResponse, out)
	// Check that the LLM was called without tools if availableLLMTools was empty
	require.Empty(t, mockLLMClient.ReceivedTools)
}

// TestAgentProcess_LLMRequestsMCPTool_Success tests full flow: LLM requests tool, MCP client executes, LLM gives final response.
func TestAgentProcess_LLMRequestsMCPTool_Success(t *testing.T) {
	toolName := "get_weather"
	toolArgsJSON := `{"location": "London"}`
	mcpToolResultText := "The weather in London is sunny."
	finalLLMResponse := "Based on the weather tool, it's sunny in London."

	cfg := config.Config{
		LLM:        config.LLMConfig{Model: "gpt"},
		MCPServers: []config.MCPServerConfig{{URL: "http://fake-weather-server.com"}},
	}

	mockLLMClient := &mockLLM{
		calls: []openai.ChatCompletionResponse{
			{ // First LLM response: requests tool call
				Choices: []openai.ChatCompletionChoice{{
					Message: openai.ChatCompletionMessage{
						ToolCalls: []openai.ToolCall{{
							ID:   "call_123",
							Type: openai.ToolTypeFunction,
							Function: openai.FunctionCall{
								Name:      toolName,
								Arguments: toolArgsJSON,
							},
						}},
					},
				}},
			},
			{ // Second LLM response: final answer after tool execution
				Choices: []openai.ChatCompletionChoice{{
					Message: openai.ChatCompletionMessage{Content: finalLLMResponse},
				}},
			},
		},
	}

	mockClient := &mockMCPClient{
		ListToolsFunc: func(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) {
			return &mcp.ListToolsResult{Tools: []mcp.Tool{
				{Name: toolName, Description: "Gets weather", RawInputSchema: json.RawMessage(`{"type":"object","properties":{"location":{"type":"string"}}}`)},
			}}, nil
		},
		CallToolFunc: func(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			require.Equal(t, toolName, request.Params.Name)
			expectedArgs := map[string]any{"location": "London"}
			require.Equal(t, expectedArgs, request.Params.Arguments)
			return &mcp.CallToolResult{
				Content: []mcp.Content{mcp.TextContent{Type: "text", Text: mcpToolResultText}},
			}, nil
		},
	}

	// agent.New will call ListTools on the mock client if we inject it.
	// For this test structure, we let New run (it might not find tools if not using this mock instance directly),
	// then override the agent's mcpClients and availableLLMTools.
	agentInstance := New(mockLLMClient, cfg)
	require.NotNil(t, agentInstance)

	agentInstance.mcpClients = []MCPClientInterface{mockClient} // Override
	agentInstance.availableLLMTools = []openai.Tool{            // Override
		{Type: openai.ToolTypeFunction, Function: &openai.FunctionDefinition{Name: toolName, Description: "Gets weather", Parameters: json.RawMessage(`{"type":"object","properties":{"location":{"type":"string"}}}`)}},
	}

	out, err := agentInstance.Process(context.Background(), "What's the weather in London?")
	require.NoError(t, err)
	require.Equal(t, finalLLMResponse, out)
	require.Len(t, mockLLMClient.ReceivedTools, 1, "The first LLM call should have received tools")
	require.Equal(t, toolName, mockLLMClient.ReceivedTools[0].Function.Name)
}

// TestAgentProcess_LLMRequestsMCPTool_MCPClientFails tests when MCP tool call fails.
func TestAgentProcess_LLMRequestsMCPTool_MCPClientFails(t *testing.T) {
	toolName := "broken_tool"
	toolArgsJSON := `{}`
	mcpErrorText := "MCP tool execution failed badly."
	// This is the text that will be sent back to the LLM as the tool's output.
	expectedToolOutputSentToLLM := "MCP tool call failed for all configured servers or tool not found (FSM helper)."
	// It's possible the FSM helper itself returns the error string from errors.New, let's assume the more specific one for now.
	// If executeMCPTool returns errors.New(mcpErrorText).Error(), then that's what's sent.
	// The current executeMCPTool returns the string "MCP tool call failed..." if all clients fail OR if the CallTool itself returns an error that doesn't get parsed into TextContent.
	// Let's refine this: if CallTool returns an error, executeMCPTool should probably return that error's string.

	// For this test, let's assume executeMCPTool will return the raw mcpErrorText if the call fails.
	// The FSM's StateExecutingTools.OnEntry will then use this as content for the Tool message.
	expectedToolOutputSentToLLM = mcpErrorText

	finalLLMResponseAfterError := "Sorry, I couldn't use the broken_tool due to an error: " + expectedToolOutputSentToLLM

	cfg := config.Config{
		LLM:        config.LLMConfig{Model: "gpt"},
		MCPServers: []config.MCPServerConfig{{URL: "http://fake-broken-server.com"}},
	}

	mockLLMClient := &mockLLM{
		calls: []openai.ChatCompletionResponse{
			{
				Choices: []openai.ChatCompletionChoice{{
					Message: openai.ChatCompletionMessage{
						ToolCalls: []openai.ToolCall{{
							ID:       "call_456",
							Type:     openai.ToolTypeFunction,
							Function: openai.FunctionCall{Name: toolName, Arguments: toolArgsJSON},
						}},
					},
				}},
			},
			{
				Choices: []openai.ChatCompletionChoice{{
					Message: openai.ChatCompletionMessage{Content: finalLLMResponseAfterError},
				}},
			},
		},
	}

	mockFailedMCPClient := &mockMCPClient{
		ListToolsFunc: func(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) {
			return &mcp.ListToolsResult{Tools: []mcp.Tool{
				{Name: toolName, Description: "A tool that is broken", RawInputSchema: json.RawMessage(`{"type":"object","properties":{}}`)},
			}}, nil
		},
		CallToolFunc: func(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			return nil, errors.New(mcpErrorText)
		},
	}

	agentInstance := New(mockLLMClient, cfg)
	require.NotNil(t, agentInstance)
	agentInstance.mcpClients = []MCPClientInterface{mockFailedMCPClient} // Override
	agentInstance.availableLLMTools = []openai.Tool{                     // Override
		{Type: openai.ToolTypeFunction, Function: &openai.FunctionDefinition{Name: toolName, Description: "A tool that is broken", Parameters: json.RawMessage(`{"type":"object","properties":{}}`)}},
	}

	out, err := agentInstance.Process(context.Background(), "Use the broken tool")
	require.NoError(t, err)
	require.Equal(t, finalLLMResponseAfterError, out)
}

func TestAgentProcess_SequentialToolCalls(t *testing.T) {
	toolA_Name := "tool_A"
	toolA_ArgsJSON := `{"input_A": "value_A"}`
	toolA_ResultText := "Result from Tool A"

	toolB_Name := "tool_B"
	toolB_ArgsJSON := `{"input_B": "value_B_from_A_result"}` // LLM might use Tool A's result for Tool B
	toolB_ResultText := "Result from Tool B"

	finalLLMResponse := "After using Tool A and Tool B, the answer is complete."

	cfg := config.Config{
		LLM:        config.LLMConfig{Model: "gpt"},
		MCPServers: []config.MCPServerConfig{{URL: "http://fake-multi-tool-server.com"}},
	}

	mockLLMClient := &mockLLM{
		calls: []openai.ChatCompletionResponse{
			{ // 1. LLM requests Tool A
				Choices: []openai.ChatCompletionChoice{{Message: openai.ChatCompletionMessage{ToolCalls: []openai.ToolCall{
					{ID: "call_A", Type: openai.ToolTypeFunction, Function: openai.FunctionCall{Name: toolA_Name, Arguments: toolA_ArgsJSON}},
				}}}},
			},
			{ // 2. LLM processes Tool A's result and requests Tool B
				Choices: []openai.ChatCompletionChoice{{Message: openai.ChatCompletionMessage{ToolCalls: []openai.ToolCall{
					{ID: "call_B", Type: openai.ToolTypeFunction, Function: openai.FunctionCall{Name: toolB_Name, Arguments: toolB_ArgsJSON}},
				}}}},
			},
			{ // 3. LLM processes Tool B's result and gives final answer
				Choices: []openai.ChatCompletionChoice{{Message: openai.ChatCompletionMessage{Content: finalLLMResponse}}},
			},
		},
	}

	mockMCP := &mockMCPClient{
		CallToolFunc: func(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			if request.Params.Name == toolA_Name {
				return &mcp.CallToolResult{Content: []mcp.Content{mcp.TextContent{Type: "text", Text: toolA_ResultText}}}, nil
			}
			if request.Params.Name == toolB_Name {
				return &mcp.CallToolResult{Content: []mcp.Content{mcp.TextContent{Type: "text", Text: toolB_ResultText}}}, nil
			}
			return nil, errors.New("unknown tool in sequential test mock")
		},
	}

	agentInstance := New(mockLLMClient, cfg)
	require.NotNil(t, agentInstance)

	// Override clients and available tools for the agent
	agentInstance.mcpClients = []MCPClientInterface{mockMCP}
	agentInstance.availableLLMTools = []openai.Tool{
		{Type: openai.ToolTypeFunction, Function: &openai.FunctionDefinition{Name: toolA_Name, Parameters: json.RawMessage(`{"type":"object","properties":{}}`)}},
		{Type: openai.ToolTypeFunction, Function: &openai.FunctionDefinition{Name: toolB_Name, Parameters: json.RawMessage(`{"type":"object","properties":{}}`)}},
	}

	out, err := agentInstance.Process(context.Background(), "Sequential task")
	require.NoError(t, err)
	require.Equal(t, finalLLMResponse, out)
}

func TestAgentProcess_MaxTurnsExceeded(t *testing.T) {
	toolName := "looping_tool"
	toolArgsJSON := `{}`

	cfg := config.Config{
		LLM:        config.LLMConfig{Model: "gpt"},
		MCPServers: []config.MCPServerConfig{{URL: "http://fake-loop-server.com"}},
	}

	// Mock LLM to always request the same tool
	responses := make([]openai.ChatCompletionResponse, 0, 6) // 5 tool calls + 1 for initial
	for i := 0; i < 6; i++ {                                 // Agent's maxTurns is 5, so 5 tool requests + initial = 6 LLM calls if it were to exceed by one
		responses = append(responses, openai.ChatCompletionResponse{
			Choices: []openai.ChatCompletionChoice{{Message: openai.ChatCompletionMessage{ToolCalls: []openai.ToolCall{
				{ID: "call_loop_" + strconv.Itoa(i), Type: openai.ToolTypeFunction, Function: openai.FunctionCall{Name: toolName, Arguments: toolArgsJSON}},
			}}}},
		})
	}
	mockLLMClient := &mockLLM{calls: responses}

	mockMCP := &mockMCPClient{
		CallToolFunc: func(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			return &mcp.CallToolResult{Content: []mcp.Content{mcp.TextContent{Type: "text", Text: "looping tool result"}}}, nil
		},
	}

	agentInstance := New(mockLLMClient, cfg)
	require.NotNil(t, agentInstance)
	agentInstance.mcpClients = []MCPClientInterface{mockMCP}
	agentInstance.availableLLMTools = []openai.Tool{
		{Type: openai.ToolTypeFunction, Function: &openai.FunctionDefinition{Name: toolName, Parameters: json.RawMessage(`{"type":"object","properties":{}}`)}},
	}

	out, err := agentInstance.Process(context.Background(), "Start loop")
	require.Error(t, err) // Expect an error
	require.Contains(t, err.Error(), "exceeded maximum interaction turns")
	require.Equal(t, "", out) // No final content
}
