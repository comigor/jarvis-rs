
package llm

import (
	"github.com/jarvis-g2o/internal/config"
	"github.com/sashabaranov/go-openai"
)

// NewClient creates a new OpenAI client
func NewClient(cfg config.LLMConfig) *openai.Client {
	config := openai.DefaultConfig(cfg.APIKey)
	config.BaseURL = cfg.BaseURL

	return openai.NewClientWithConfig(config)
}
