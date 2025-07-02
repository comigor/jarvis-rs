package agent

import (
	"context"
	"testing"

	"github.com/jarvis-g2o/internal/config"
	"github.com/jarvis-g2o/pkg/tools"
)

func TestAgentProcess_LLMError(t *testing.T) {
	a := New(&mockLLM{err: context.DeadlineExceeded}, config.LLMConfig{Model: "gpt"}, tools.NewToolManager())
	if _, err := a.Process(context.Background(), "hi"); err == nil {
		t.Fatalf("expected error")
	}
}
