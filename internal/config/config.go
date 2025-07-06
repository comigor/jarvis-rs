package config

import (
	"os"

	"github.com/spf13/viper"
)

// MCPClientType defines the type of MCP client to use for a server.
type MCPClientType string

const (
	// ClientTypeSSE uses the NewSSEMCPClient.
	ClientTypeSSE MCPClientType = "sse"
	// ClientTypeStreamableHTTP uses the NewStreamableHttpClient.
	ClientTypeStreamableHTTP MCPClientType = "streamable_http"
	// ClientTypeStdio uses the NewStdioMCPClient.
	ClientTypeStdio MCPClientType = "stdio"
)

// MCPServerConfig holds the configuration for a single MCP server, including custom headers and client type.
type MCPServerConfig struct {
	Name    string            `mapstructure:"name"`
	URL     string            `mapstructure:"url"`
	Type    MCPClientType     `mapstructure:"type"` // "sse", "streamable_http" or "stdio"
	Headers map[string]string `mapstructure:"headers"`
	// Stdio specific fields
	Command string            `mapstructure:"command"`
	Args    []string          `mapstructure:"args"`
	Env     map[string]string `mapstructure:"env"`
}

// Config holds the application configuration
type Config struct {
	LLM        LLMConfig
	Server     ServerConfig
	MCPServers []MCPServerConfig `mapstructure:"mcp_servers"`
}

// LLMConfig holds the LLM configuration
type LLMConfig struct {
	Provider     string `mapstructure:"provider"`
	BaseURL      string `mapstructure:"base_url"`
	APIKey       string `mapstructure:"api_key"`
	Model        string `mapstructure:"model"`
	SystemPrompt string `mapstructure:"system_prompt"` // System prompt to use for the LLM
}

// LogsConfig holds logging configuration
type LogsConfig struct {
	Level string `mapstructure:"level"`
}

// ServerConfig holds the server configuration
type ServerConfig struct {
	Host string     `mapstructure:"host"`
	Port string     `mapstructure:"port"`
	Logs LogsConfig `mapstructure:"logs"`
}

// Load loads the configuration from the config.yaml file
func Load() (*Config, error) {
	if cfgPath, ok := os.LookupEnv("CONFIG_PATH"); ok && cfgPath != "" {
		viper.SetConfigFile(cfgPath)
	} else {
		viper.SetConfigName("config")
		viper.SetConfigType("yaml")
		viper.AddConfigPath(".")
	}

	if err := viper.ReadInConfig(); err != nil {
		return nil, err
	}

	var config Config
	if err := viper.Unmarshal(&config); err != nil {
		return nil, err
	}

	return &config, nil
}
