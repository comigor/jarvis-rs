
package tools

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/jarvis-g2o/internal/config"
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
	return "Interacts with Home Assistant to control smart home devices."
}

// Run runs the tool
func (t *HomeAssistantTool) Run(args string) (string, error) {
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
