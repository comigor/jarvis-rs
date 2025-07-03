package config

import (
	"github.com/spf13/viper"
)

// Config holds the application configuration
type Config struct {
	LLM        LLMConfig
	Server     ServerConfig
	MCPServers []string `mapstructure:"mcp_servers"`
}

// LLMConfig holds the LLM configuration
type LLMConfig struct {
	Provider string `mapstructure:"provider"`
	BaseURL  string `mapstructure:"base_url"`
	APIKey   string `mapstructure:"api_key"`
	Model    string `mapstructure:"model"`
}

// ServerConfig holds the server configuration
type ServerConfig struct {
	Host string `mapstructure:"host"`
	Port string `mapstructure:"port"`
}

// Load loads the configuration from the config.yaml file
func Load() (*Config, error) {
	viper.SetConfigName("config")
	viper.SetConfigType("yaml")
	viper.AddConfigPath(".")

	if err := viper.ReadInConfig(); err != nil {
		return nil, err
	}

	var config Config
	if err := viper.Unmarshal(&config); err != nil {
		return nil, err
	}

	return &config, nil
}
