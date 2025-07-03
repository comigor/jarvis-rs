package agent

import (
	"context"
	"testing"

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
		panic("mockLLM: no more responses configured")
	}
	resp := m.calls[0]
	m.calls = m.calls[1:]
	return resp, nil
}

func TestAgentProcess_NoTool(t *testing.T) {
	llmResp := openai.ChatCompletionResponse{
		Choices: []openai.ChatCompletionChoice{{
			Message: openai.ChatCompletionMessage{Content: "hi direct from LLM"},
		}},
	}
	cfg := config.Config{
		LLM: config.LLMConfig{Model: "gpt"},
		MCPServers: []config.MCPServerConfig{ // Updated to new structure
			{URL: "http://fake-mcp-server1.example.com", Headers: map[string]string{"X-Test": "true"}},
		},
	}
	agentInstance := New(&mockLLM{calls: []openai.ChatCompletionResponse{llmResp}}, cfg)
	require.NotNil(t, agentInstance)

	out, err := agentInstance.Process(context.Background(), "hello")
	require.NoError(t, err)
	require.Equal(t, "hi direct from LLM", out)
}

func TestAgentProcess_MCPToolCall_Success(t *testing.T) {
	llmJSONResponse := `{"tool_name": "test_tool", "arguments": {"param":"value"}}`
	llmResp := openai.ChatCompletionResponse{
		Choices: []openai.ChatCompletionChoice{{
			Message: openai.ChatCompletionMessage{Content: llmJSONResponse},
		}},
	}

	cfg := config.Config{
		LLM: config.LLMConfig{Model: "gpt"},
		MCPServers: []config.MCPServerConfig{ // Does not matter much as we override clients
			{URL: "http://dummy-server.com"},
		},
	}

	agentInstance := New(&mockLLM{calls: []openai.ChatCompletionResponse{llmResp}}, cfg)
	require.NotNil(t, agentInstance)

	mockClient := &mockMCPClient{
		ListToolsFunc: func(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) {
			return &mcp.ListToolsResult{Tools: []mcp.Tool{{Name: "test_tool"}}}, nil
		},
		CallToolFunc: func(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			require.Equal(t, "test_tool", request.Params.Name)
			expectedArgs := map[string]any{"param": "value"}
			require.Equal(t, expectedArgs, request.Params.Arguments)
			return &mcp.CallToolResult{
				Content: []mcp.Content{mcp.TextContent{Type: "text", Text: "success from mock_tool"}},
			}, nil
		},
	}
	agentInstance.mcpClients = []MCPClientInterface{mockClient} // Override with mock

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
		MCPServers: []config.MCPServerConfig{{URL: "http://dummy1.com"}, {URL: "http://dummy2.com"}},
	}
	agentInstance := New(&mockLLM{calls: []openai.ChatCompletionResponse{llmResp}}, cfg)
	require.NotNil(t, agentInstance)

	mockErr := context.DeadlineExceeded
	mockClient1 := &mockMCPClient{
		ListToolsFunc: func(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) {
			return &mcp.ListToolsResult{Tools: []mcp.Tool{{Name: "test_tool"}}}, nil
		},
		CallToolFunc: func(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error) {
			return nil, mockErr
		},
	}
	mockClient2 := &mockMCPClient{
		ListToolsFunc: func(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) {
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
		MCPServers: []config.MCPServerConfig{}, // No servers
	}
	agentInstance := New(&mockLLM{calls: []openai.ChatCompletionResponse{llmResp}}, cfg)
	require.NotNil(t, agentInstance)
	require.Empty(t, agentInstance.mcpClients)

	out, err := agentInstance.Process(context.Background(), "do something with mcp")
	require.NoError(t, err)
	require.Equal(t, "LLM suggested an MCP tool, but no MCP clients are available.", out)
}

func TestAgentProcess_MCPToolCall_ToolNotFoundOnAnyServer(t *testing.T) {
	llmJSONResponse := `{"tool_name": "non_existent_tool", "arguments": {}}`
	llmResp := openai.ChatCompletionResponse{
		Choices: []openai.ChatCompletionChoice{{
			Message: openai.ChatCompletionMessage{Content: llmJSONResponse},
		}},
	}
	cfg := config.Config{
		LLM:        config.LLMConfig{Model: "gpt"},
		MCPServers: []config.MCPServerConfig{{URL: "http://dummy1.com"}},
	}
	agentInstance := New(&mockLLM{calls: []openai.ChatCompletionResponse{llmResp}}, cfg)
	require.NotNil(t, agentInstance)

	mockClient := &mockMCPClient{
		ListToolsFunc: func(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error) {
			return &mcp.ListToolsResult{Tools: []mcp.Tool{{Name: "some_other_tool"}}}, nil // Does not list "non_existent_tool"
		},
	}
	agentInstance.mcpClients = []MCPClientInterface{mockClient}

	out, err := agentInstance.Process(context.Background(), "do something with mcp")
	require.NoError(t, err)
	require.Equal(t, "LLM suggested tool 'non_existent_tool', but it was not found on any available MCP server or the call failed.", out)
}
