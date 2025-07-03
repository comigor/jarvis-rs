package agent

import (
	"context"
	"testing"

	"github.com/jarvis-g2o/internal/config"
	// "github.com/jarvis-g2o/pkg/tools" // Removed
)

func TestAgentProcess_LLMError(t *testing.T) {
	cfg := config.Config{
		LLM: config.LLMConfig{Model: "gpt"},
		MCPServers: []config.MCPServerConfig{ // Corrected to use MCPServerConfig
			{URL: "http://fake-mcp-server.example.com"},
		},
	}
	// This test focuses on LLM error propagation, agent's MCP client init state is secondary here.
	a := New(&mockLLM{err: context.DeadlineExceeded}, cfg)
	if _, err := a.Process(context.Background(), "hi"); err == nil {
		t.Fatalf("expected error")
	}
}
