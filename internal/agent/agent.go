package agent

import (
	"context"
	"encoding/json"

	"github.com/jarvis-g2o/internal/config"
	"github.com/jarvis-g2o/pkg/tools"
	"github.com/sashabaranov/go-openai"
)

// Agent is the main agent struct
type Agent struct {
	llmClient   *openai.Client
	cfg         config.LLMConfig
	toolManager *tools.ToolManager
}

// New creates a new agent
func New(llmClient *openai.Client, cfg config.LLMConfig, toolManager *tools.ToolManager) *Agent {
	return &Agent{
		llmClient:   llmClient,
		cfg:         cfg,
		toolManager: toolManager,
	}
}

// Process processes a request and returns a response
func (a *Agent) Process(ctx context.Context, request string) (string, error) {
	// First, we send the request to the LLM to see if it wants to use a tool.
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
			Tools: a.getTools(),
		},
	)

	if err != nil {
		return "", err
	}

	// If the LLM wants to use a tool, it will return a tool call.
	// Otherwise, it will return a message.
	if len(resp.Choices[0].Message.ToolCalls) > 0 {
		// For now, we only support one tool call at a time.
		toolCall := resp.Choices[0].Message.ToolCalls[0]
		tool, err := a.toolManager.GetTool(toolCall.Function.Name)
		if err != nil {
			return "", err
		}

		// We call the tool with the arguments provided by the LLM.
		toolResult, err := tool.Run(toolCall.Function.Arguments)
		if err != nil {
			return "", err
		}

		// We send the tool result back to the LLM to get a final response.
		messages := []openai.ChatCompletionMessage{
			{
				Role:    openai.ChatMessageRoleUser,
				Content: request,
			},
			resp.Choices[0].Message, // assistant with tool call
			{
				Role:       openai.ChatMessageRoleTool,
				Content:    toolResult,
				ToolCallID: toolCall.ID,
			},
		}

		resp, err = a.llmClient.CreateChatCompletion(ctx, openai.ChatCompletionRequest{
			Model:    a.cfg.Model,
			Messages: messages,
		})

		if err != nil {
			return "", err
		}
	}

	return resp.Choices[0].Message.Content, nil
}

func (a *Agent) getTools() []openai.Tool {
	cmdSchema := json.RawMessage(`{"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}`)
	return []openai.Tool{
		{
			Type: openai.ToolTypeFunction,
			Function: &openai.FunctionDefinition{
				Name:        "home_assistant",
				Description: "Interacts with Home Assistant to control smart home devices.",
				Parameters:  cmdSchema,
			},
		},
		{
			Type: openai.ToolTypeFunction,
			Function: &openai.FunctionDefinition{
				Name:        "google_calendar",
				Description: "Interacts with Google Calendar to manage events.",
				Parameters:  cmdSchema,
			},
		},
		{
			Type: openai.ToolTypeFunction,
			Function: &openai.FunctionDefinition{
				Name:        "home_assistant_list",
				Description: "Lists Home Assistant entity IDs (devices).",
				Parameters:  json.RawMessage(`{"type":"object","properties":{}}`),
			},
		},
	}
}
