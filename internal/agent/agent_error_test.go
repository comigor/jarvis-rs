package agent

import (
	"context"
	"testing"

	"github.com/jarvis-g2o/internal/config"
	// "github.com/jarvis-g2o/pkg/tools" // Removed
)

func TestAgentProcess_LLMError(t *testing.T) {
	cfg := config.Config{
		LLM:        config.LLMConfig{Model: "gpt"},
		MCPServers: []string{"http://fake-mcp-server.example.com"}, // Add a dummy MCP server
	}
	a := New(&mockLLM{err: context.DeadlineExceeded}, cfg)
	if _, err := a.Process(context.Background(), "hi"); err == nil {
		t.Fatalf("expected error")
	}
}
