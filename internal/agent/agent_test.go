package agent

import (
	"context"
	"testing"

	"encoding/json"
	"errors"

	"github.com/jarvis-g2o/internal/config"
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
}

func (m *mockLLM) CreateChatCompletion(ctx context.Context, r openai.ChatCompletionRequest) (openai.ChatCompletionResponse, error) {
	if m.err != nil {
		return openai.ChatCompletionResponse{}, m.err
	}
	if len(m.calls) == 0 {
		panic("mockLLM: no more responses configured for request: " + r.Messages[0].Content)
	}
	resp := m.calls[0]
	m.calls = m.calls[1:]

	// Check if tools were provided to the LLM, for test validation
	if len(r.Tools) > 0 {
		// If this mock response is supposed to be a tool call, ensure it has one.
		// If not, ensure it doesn't. This helps validate test setup.
		// For simplicity, this check is basic. More advanced checks could verify specific tools.
		// fmt.Printf("LLM call with %d tools provided.\n", len(r.Tools))
	}
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
	require.Empty(t, agentInstance.availableLLMTools, "Agent should have no tools available if no MCP servers are configured or they offer no tools.")

	out, err := agentInstance.Process(context.Background(), "User says hi")
	require.NoError(t, err)
	require.Equal(t, llmDirectResponse, out)
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

	// Mock LLM: First call requests a tool, second call gives final answer
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

	// Mock MCP Client
	mockClient := &mockMCPClient{
		ListToolsFunc: func(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) {
			// This tool needs to be available for the agent to register it for the LLM
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

	// Create agent. The New() function will use the ListToolsFunc from the mockClient if we could inject it.
	// For now, we create the agent, then override its mcpClients and availableLLMTools.
	agentInstance := New(mockLLMClient, cfg)
	require.NotNil(t, agentInstance)

	// Manually set the mcpClients and availableLLMTools for this test
	agentInstance.mcpClients = []MCPClientInterface{mockClient}
	agentInstance.availableLLMTools = []openai.Tool{
		{Type: openai.ToolTypeFunction, Function: &openai.FunctionDefinition{Name: toolName, Description: "Gets weather", Parameters: json.RawMessage(`{"type":"object","properties":{"location":{"type":"string"}}}`)}},
	}

	out, err := agentInstance.Process(context.Background(), "What's the weather in London?")
	require.NoError(t, err)
	require.Equal(t, finalLLMResponse, out)
}

// TestAgentProcess_LLMRequestsMCPTool_MCPClientFails tests when MCP tool call fails.
func TestAgentProcess_LLMRequestsMCPTool_MCPClientFails(t *testing.T) {
	toolName := "broken_tool"
	toolArgsJSON := `{}`
	mcpErrorText := "MCP tool execution failed badly."
	finalLLMResponseAfterError := "Sorry, I couldn't use the broken_tool due to an error: MCP tool execution failed badly."

	cfg := config.Config{
		LLM:        config.LLMConfig{Model: "gpt"},
		MCPServers: []config.MCPServerConfig{{URL: "http://fake-broken-server.com"}},
	}

	mockLLMClient := &mockLLM{
		calls: []openai.ChatCompletionResponse{
			{ // First LLM response: requests tool call
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
			{ // Second LLM response: LLM acknowledges the tool error
				Choices: []openai.ChatCompletionChoice{{
					Message: openai.ChatCompletionMessage{Content: finalLLMResponseAfterError},
				}},
			},
		},
	}

	mockClient := &mockMCPClient{
		ListToolsFunc: func(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) {
			return &mcp.ListToolsResult{Tools: []mcp.Tool{
				{Name: toolName, Description: "A tool that is broken", RawInputSchema: json.RawMessage(`{"type":"object","properties":{}}`)},
			}}, nil
		},
		CallToolFunc: func(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			return nil, errors.New(mcpErrorText) // MCP client's CallTool returns an error
		},
	}

	agentInstance := New(mockLLMClient, cfg)
	require.NotNil(t, agentInstance)
	agentInstance.mcpClients = []MCPClientInterface{mockClient}
	agentInstance.availableLLMTools = []openai.Tool{
		{Type: openai.ToolTypeFunction, Function: &openai.FunctionDefinition{Name: toolName, Description: "A tool that is broken", Parameters: json.RawMessage(`{"type":"object","properties":{}}`)}},
	}

	out, err := agentInstance.Process(context.Background(), "Use the broken tool")
	require.NoError(t, err)
	require.Equal(t, finalLLMResponseAfterError, out)
}
