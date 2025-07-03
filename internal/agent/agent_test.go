package agent

import (
	"context"
	"testing"

	"github.com/jarvis-g2o/internal/config"
	// "github.com/jarvis-g2o/pkg/tools" // Removed
	"github.com/sashabaranov/go-openai"
	"github.com/stretchr/testify/require" // For better assertions

	"github.com/mark3labs/mcp-go/mcp" // For mcp types
)

// mockMCPClient is a mock implementation of agent.MCPClientInterface (defined in agent.go).
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
	return &mcp.InitializeResult{}, nil // Default success
}

func (m *mockMCPClient) ListTools(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) {
	if m.ListToolsFunc != nil {
		return m.ListToolsFunc(ctx, req)
	}
	// Default: return an empty list of tools
	return &mcp.ListToolsResult{Tools: []mcp.Tool{}}, nil
}

func (m *mockMCPClient) CallTool(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error) {
	if m.CallToolFunc != nil {
		return m.CallToolFunc(ctx, request)
	}
	// Default success with some basic result
	return &mcp.CallToolResult{
		Content: []mcp.Content{
			mcp.TextContent{
				Type: "text",
				Text: "mock tool result for " + request.Params.Name,
			},
		},
	}, nil
}

func (m *mockMCPClient) Close() error {
	if m.CloseFunc != nil {
		return m.CloseFunc()
	}
	return nil // Default success
}

type mockLLM struct {
	calls []openai.ChatCompletionResponse
	err   error
}

func (m *mockLLM) CreateChatCompletion(ctx context.Context, r openai.ChatCompletionRequest) (openai.ChatCompletionResponse, error) {
	if m.err != nil {
		return openai.ChatCompletionResponse{}, m.err
	}
	resp := m.calls[0]
	m.calls = m.calls[1:]
	return resp, nil
}

// mockTool removed

func TestAgentProcess_NoTool(t *testing.T) {
	llmResp := openai.ChatCompletionResponse{
		Choices: []openai.ChatCompletionChoice{{
			Message: openai.ChatCompletionMessage{Content: "hi"},
		}},
	}
	cfg := config.Config{
		LLM:        config.LLMConfig{Model: "gpt"},
		MCPServers: []string{"http://fake-mcp-server1.example.com"}, // Provide a server URL for New to attempt client creation
	}

	// Store original NewMCPClientFunc and restore it after the test
	// This is a common pattern if you are modifying global state for mocks,
	// but here we are injecting mocks directly into a modified New function or by passing them.
	// For this test, we expect New to be called with cfg, and it will try to create real clients.
	// We are not directly mocking the client creation part in *this* specific test,
	// but rather ensuring the agent works if no MCP call is made.
	// The internal mcpClients in the agent will be whatever New creates (potentially none if creation fails).
	// A better approach for more control would be to allow injecting mock MCPClientInterface instances into the agent.

	// Let's simplify: For this test, we assume NewAgent can be created,
	// and we are testing the scenario where no MCP call is made by the LLM.
	// The agent's internal mcpClients list might be empty if server URLs are invalid or New fails internally,
	// but that's okay for this specific test's purpose (testing the no-MCP-call path).

	// To make New testable with mocks, we'd need to refactor New or how Agent stores clients.
	// Option 1: Agent has a public way to set mock clients (less ideal for production code).
	// Option 2: New takes a factory function for creating MCPClientInterface (better).

	// For now, let's assume the test passes if the agent is created and the LLM path works.
	// We will create specific tests for MCP logic using mock clients directly.
	// This test becomes more of an integration test for the "LLM-only" path.
	agentInstance := New(&mockLLM{calls: []openai.ChatCompletionResponse{llmResp}}, cfg)
	require.NotNil(t, agentInstance)

	out, err := agentInstance.Process(context.Background(), "hello")
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if out != "hi" {
		t.Fatalf("want hi got %s", out)
	}
}

// TestAgentProcess_WithTool removed as tool functionality is removed.
// Future tests should cover MCP interactions.

func TestAgentProcess_MCPToolCall_Success(t *testing.T) {
	llmJSONResponse := `{"tool_name": "test_tool", "arguments": {"param":"value"}}`
	llmResp := openai.ChatCompletionResponse{
		Choices: []openai.ChatCompletionChoice{{
			Message: openai.ChatCompletionMessage{Content: llmJSONResponse},
		}},
	}

	cfg := config.Config{
		LLM:        config.LLMConfig{Model: "gpt"},
		MCPServers: []string{"http://dummy-server.com"}, // Needed for New, but we'll override mcpClients
	}

	agentInstance := New(&mockLLM{calls: []openai.ChatCompletionResponse{llmResp}}, cfg)
	require.NotNil(t, agentInstance)

	mockClient := &mockMCPClient{
		ListToolsFunc: func(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) { // Added req
			return &mcp.ListToolsResult{Tools: []mcp.Tool{{Name: "test_tool"}}}, nil
		},
		CallToolFunc: func(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			require.Equal(t, "test_tool", request.Params.Name)
			expectedArgs := map[string]any{"param": "value"}
			require.Equal(t, expectedArgs, request.Params.Arguments)
			return &mcp.CallToolResult{
				Content: []mcp.Content{ // Corrected: Use Content field
					mcp.TextContent{
						Type: "text",
						Text: "success from mock_tool",
					},
				},
			}, nil
		},
	}
	agentInstance.mcpClients = []MCPClientInterface{mockClient}

	out, err := agentInstance.Process(context.Background(), "do something with mcp")
	require.NoError(t, err)
	require.Equal(t, "success from mock_tool", out)
}

func TestAgentProcess_MCPToolCall_AllClientsFail(t *testing.T) {
	llmJSONResponse := `{"tool_name": "test_tool", "arguments": {"param":"value"}}`
	llmResp := openai.ChatCompletionResponse{
		Choices: []openai.ChatCompletionChoice{{
			Message: openai.ChatCompletionMessage{Content: llmJSONResponse},
		}},
	}

	cfg := config.Config{
		LLM:        config.LLMConfig{Model: "gpt"},
		MCPServers: []string{"http://dummy1.com", "http://dummy2.com"}, // For New
	}

	agentInstance := New(&mockLLM{calls: []openai.ChatCompletionResponse{llmResp}}, cfg)
	require.NotNil(t, agentInstance)

	mockErr := context.DeadlineExceeded // Some distinct error

	mockClient1 := &mockMCPClient{
		ListToolsFunc: func(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) { // Added req
			// This client lists the tool, but CallTool will fail
			return &mcp.ListToolsResult{Tools: []mcp.Tool{{Name: "test_tool"}}}, nil
		},
		CallToolFunc: func(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			return nil, mockErr
		},
	}
	mockClient2 := &mockMCPClient{
		ListToolsFunc: func(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) { // Added req
			// This client also lists the tool, but CallTool will also fail
			return &mcp.ListToolsResult{Tools: []mcp.Tool{{Name: "test_tool"}}}, nil
		},
		CallToolFunc: func(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			return nil, mockErr
		},
	}

	agentInstance.mcpClients = []MCPClientInterface{mockClient1, mockClient2}

	out, err := agentInstance.Process(context.Background(), "do something with mcp")
	require.NoError(t, err)
	require.Equal(t, "LLM suggested tool 'test_tool', but it was not found on any available MCP server or the call failed.", out)
}

func TestAgentProcess_MCPToolCall_NoClientsAvailable(t *testing.T) {
	llmJSONResponse := `{"tool_name": "test_tool", "arguments": {"param":"value"}}`
	llmResp := openai.ChatCompletionResponse{
		Choices: []openai.ChatCompletionChoice{{
			Message: openai.ChatCompletionMessage{Content: llmJSONResponse},
		}},
	}

	cfg := config.Config{
		LLM:        config.LLMConfig{Model: "gpt"},
		MCPServers: []string{}, // No servers configured, so New will result in empty mcpClients
	}

	// Create agent, New() should handle empty MCPServers gracefully
	agentInstance := New(&mockLLM{calls: []openai.ChatCompletionResponse{llmResp}}, cfg)
	require.NotNil(t, agentInstance)
	// New() should result in an empty mcpClients slice if MCPServers is empty.
	require.Empty(t, agentInstance.mcpClients, "Agent's mcpClients should be empty if no MCPServers are provided.")

	out, err := agentInstance.Process(context.Background(), "do something with mcp")
	require.NoError(t, err)
	require.Equal(t, "LLM suggested an MCP tool, but no MCP clients are available.", out)
}
