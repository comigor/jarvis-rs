package agent

import (
	"context"
	"testing"

	"github.com/jarvis-g2o/internal/config"
	"github.com/jarvis-g2o/pkg/tools"
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

type mockTool struct{}

func (t *mockTool) Name() string        { return "mock_tool" }
func (t *mockTool) Description() string { return "mock" }
func (t *mockTool) Run(args string) (string, error) {
	return "tool-result", nil
}

func TestAgentProcess_NoTool(t *testing.T) {
	llmResp := openai.ChatCompletionResponse{
		Choices: []openai.ChatCompletionChoice{{
			Message: openai.ChatCompletionMessage{Content: "hi"},
		}},
	}
	a := New(&mockLLM{calls: []openai.ChatCompletionResponse{llmResp}}, config.LLMConfig{Model: "gpt"}, tools.NewToolManager())

	out, err := a.Process(context.Background(), "hello")
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if out != "hi" {
		t.Fatalf("want hi got %s", out)
	}
}

func TestAgentProcess_WithTool(t *testing.T) {
	tm := tools.NewToolManager()
	tm.RegisterTool(&mockTool{})

	first := openai.ChatCompletionResponse{Choices: []openai.ChatCompletionChoice{{
		Message: openai.ChatCompletionMessage{
			ToolCalls: []openai.ToolCall{{
				ID:   "1",
				Type: openai.ToolTypeFunction,
				Function: openai.FunctionCall{
					Name:      "mock_tool",
					Arguments: "{}",
				},
			}},
		}}}}
	second := openai.ChatCompletionResponse{Choices: []openai.ChatCompletionChoice{{
		Message: openai.ChatCompletionMessage{Content: "done"},
	}}}
	a := New(&mockLLM{calls: []openai.ChatCompletionResponse{first, second}}, config.LLMConfig{Model: "gpt"}, tm)

	out, err := a.Process(context.Background(), "do")
	if err != nil {
		t.Fatalf("err %v", err)
	}
	if out != "done" {
		t.Fatalf("want done got %s", out)
	}
}
