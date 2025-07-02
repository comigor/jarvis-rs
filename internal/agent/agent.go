
package agent

import (
	"context"

	"github.com/jarvis-g2o/internal/config"
	"github.com/sashabaranov/go-openai"
)

// Agent is the main agent struct
type Agent struct {
	llmClient *openai.Client
	cfg       config.LLMConfig
}

// New creates a new agent
func New(llmClient *openai.Client, cfg config.LLMConfig) *Agent {
	return &Agent{
		llmClient: llmClient,
		cfg:       cfg,
	}
}

// Process processes a request and returns a response
func (a *Agent) Process(ctx context.Context, request string) (string, error) {
	resp, err := a.llmClient.CreateChatCompletion(
		ctx,
		openai.ChatCompletionRequest{
			Model: a.cfg.Model,
			Messages: []openai.ChatCompletionMessage{
				{
					Role:    openai.ChatMessageRoleUser,
					Content: request,
				},
			},
		},
	)

	if err != nil {
		return "", err
	}

	return resp.Choices[0].Message.Content, nil
}
