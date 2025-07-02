package tools

import (
	"context"
	"encoding/json"

	"github.com/jarvis-g2o/internal/config"
)

// HomeAssistantListTool lists all entity IDs available in Home Assistant
// It helps users discover the correct entity_id to interact with.
// No arguments are required.
type HomeAssistantListTool struct {
	client *HomeAssistantClient
}

// NewHomeAssistantListTool creates a new HomeAssistantListTool
func NewHomeAssistantListTool(cfg config.HomeAssistantConfig) *HomeAssistantListTool {
	return &HomeAssistantListTool{client: NewHomeAssistantClient(cfg)}
}

// Name returns the name of the tool
func (t *HomeAssistantListTool) Name() string { return "home_assistant_list" }

// Description returns the description of the tool
func (t *HomeAssistantListTool) Description() string {
	return "Lists Home Assistant entity IDs (devices)."
}

// Run executes the listing operation and returns JSON array of entity_ids
func (t *HomeAssistantListTool) Run(_ string) (string, error) {
	ids, err := t.client.ListEntities(context.Background())
	if err != nil {
		return "", err
	}
	b, err := json.Marshal(ids)
	if err != nil {
		return "", err
	}
	return string(b), nil
}
