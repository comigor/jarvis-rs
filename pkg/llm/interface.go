package llm

import (
	"context"

	"github.com/sashabaranov/go-openai"
)

// Client is minimal subset of openai.Client used by the agent; it is easy to mock in tests.
type Client interface {
	CreateChatCompletion(ctx context.Context, req openai.ChatCompletionRequest) (openai.ChatCompletionResponse, error)
}
