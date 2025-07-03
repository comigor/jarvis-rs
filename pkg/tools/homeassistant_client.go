package tools

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"

	"github.com/jarvis-g2o/internal/config"
	"go.uber.org/zap"
)

// HomeAssistantClient is a client for the Home Assistant API
type HomeAssistantClient struct {
	cfg    config.HomeAssistantConfig
	client *http.Client
}

// NewHomeAssistantClient creates a new HomeAssistantClient
func NewHomeAssistantClient(cfg config.HomeAssistantConfig) *HomeAssistantClient {
	return &HomeAssistantClient{
		cfg:    cfg,
		client: &http.Client{},
	}
}

// CallService calls a service in Home Assistant
func (c *HomeAssistantClient) CallService(ctx context.Context, domain, service string, data map[string]interface{}) error {
	base := strings.TrimSuffix(c.cfg.URL, "/")
	endpoint := fmt.Sprintf("%s/api/services/%s/%s", base, domain, service)

	body, err := json.Marshal(data)
	zap.S().Infow("homeassistant call", "url", endpoint, "body", string(body))

	if err != nil {
		return err
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, endpoint, bytes.NewBuffer(body))
	if err != nil {
		return err
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", c.cfg.Token))
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		zap.S().Warnw("homeassistant non-200", "url", endpoint, "status", resp.StatusCode)
		return fmt.Errorf("unexpected status code: %d", resp.StatusCode)
	}

	return nil
}

// ListEntities retrieves all entity IDs from Home Assistant
func (c *HomeAssistantClient) ListEntities(ctx context.Context) ([]string, error) {
	url := fmt.Sprintf("%s/api/states", c.cfg.URL)

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", c.cfg.Token))
	req.Header.Set("Accept", "application/json")

	resp, err := c.client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("unexpected status code: %d", resp.StatusCode)
	}

	var states []struct {
		EntityID string `json:"entity_id"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&states); err != nil {
		return nil, err
	}

	ids := make([]string, len(states))
	for i, s := range states {
		ids[i] = s.EntityID
	}

	return ids, nil
}
