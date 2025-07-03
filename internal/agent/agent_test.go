package agent

import (
	"context"
	"testing"

	"github.com/jarvis-g2o/internal/config"
	// "github.com/jarvis-g2o/pkg/tools" // Removed
	"github.com/sashabaranov/go-openai"
)

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
		MCPServers: []string{"http://fake-mcp-server.example.com"}, // Add a dummy MCP server
	}
	a := New(&mockLLM{calls: []openai.ChatCompletionResponse{llmResp}}, cfg)

	out, err := a.Process(context.Background(), "hello")
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if out != "hi" {
		t.Fatalf("want hi got %s", out)
	}
}

// TestAgentProcess_WithTool removed as tool functionality is removed.
// Future tests should cover MCP interactions.
