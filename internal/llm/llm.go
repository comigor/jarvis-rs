package llm

import (
	"github.com/comigor/jarvis-go/internal/config"
	"github.com/sashabaranov/go-openai"
)

// NewClient creates a new OpenAI client
func NewClient(cfg config.LLMConfig) *openai.Client {
	config := openai.DefaultConfig(cfg.APIKey)
	config.BaseURL = cfg.BaseURL

	return openai.NewClientWithConfig(config)
}
