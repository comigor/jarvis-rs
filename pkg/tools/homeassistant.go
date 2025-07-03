package tools

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/jarvis-g2o/internal/config"
	"go.uber.org/zap"
)

// HomeAssistantTool is a tool for interacting with Home Assistant
type HomeAssistantTool struct {
	client *HomeAssistantClient
}

// NewHomeAssistantTool creates a new HomeAssistantTool
func NewHomeAssistantTool(cfg config.HomeAssistantConfig) *HomeAssistantTool {
	return &HomeAssistantTool{
		client: NewHomeAssistantClient(cfg),
	}
}

// Name returns the name of the tool
func (t *HomeAssistantTool) Name() string {
	return "home_assistant"
}

// Description returns the description of the tool
func (t *HomeAssistantTool) Description() string {
	return "Controls Home Assistant devices. ALWAYS call 'home_assistant_list' first to obtain valid entity_ids, then invoke this tool with them."
}

// Params returns argument struct example for schema generation
func (t *HomeAssistantTool) Params() any {
    type args struct {
        Domain  string                 `json:"domain"`
        Service string                 `json:"service"`
        Data    map[string]interface{} `json:"data,omitempty"`
    }
    return &args{}
}

	return &args{}
}

// Run runs the tool
func (t *HomeAssistantTool) Run(args string) (string, error) {
	zap.S().Infow("homeassistant tool invoked", "args", args)

	var toolArgs struct {
		Domain  string                 `json:"domain"`
		Service string                 `json:"service"`
		Data    map[string]interface{} `json:"data"`
	}

	if err := json.Unmarshal([]byte(args), &toolArgs); err != nil {
		return "", err
	}

	if err := t.client.CallService(context.Background(), toolArgs.Domain, toolArgs.Service, toolArgs.Data); err != nil {
		return "", err
	}

	return fmt.Sprintf("Successfully called service %s.%s", toolArgs.Domain, toolArgs.Service), nil
}
