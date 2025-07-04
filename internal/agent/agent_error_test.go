package agent

import (
	"context"
	"testing"

	"github.com/comigor/jarvis-go/internal/config"
	// openai import might be needed if mockLLM is used from agent_test, but it's redefined here.
	// For simplicity, assuming mockLLM here is self-contained or we use the one from agent_test.
	// If using agent_test.mockLLM, ensure no import cycles.
	// For this file, we only need a mock that can return an error.
	"github.com/sashabaranov/go-openai"
)

// Simplified mockLLM for error testing, assuming agent_test.mockLLM might not be directly accessible
// or to keep this test self-contained for error path.
type errorMockLLM struct {
	err error
}

func (m *errorMockLLM) CreateChatCompletion(ctx context.Context, r openai.ChatCompletionRequest) (openai.ChatCompletionResponse, error) {
	return openai.ChatCompletionResponse{}, m.err
}

func TestAgentProcess_LLMError(t *testing.T) {
	cfg := config.Config{
		LLM: config.LLMConfig{Model: "gpt"},
		MCPServers: []config.MCPServerConfig{ // Use new structure
			{URL: "http://fake-mcp-server.example.com"},
		},
	}
	// Use the errorMockLLM for this specific test
	a := New(&errorMockLLM{err: context.DeadlineExceeded}, cfg)
	if _, err := a.Process(context.Background(), "hi"); err == nil {
		t.Fatalf("expected error")
	}
}
