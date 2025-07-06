package config

import (
	"os"
	"testing"
)

const sampleStdioConfig = `
llm:
  provider: openai
  base_url: https://api.example.com
  api_key: dummy
  model: gpt-4o
server:
  host: 0.0.0.0
  port: "8080"
mcp_servers:
  - type: stdio
    command: ./mock
    args: ["--flag"]
    env:
      FOO: bar
`

// TestLoad_Stdio verifies that Load correctly unmarshals stdio server configuration.
func TestLoad_Stdio(t *testing.T) {
	// Write config to temp file
	tmp, err := os.CreateTemp(t.TempDir(), "cfg-*.yaml")
	if err != nil {
		t.Fatalf("temp file: %v", err)
	}
	if _, err := tmp.WriteString(sampleStdioConfig); err != nil {
		t.Fatalf("write: %v", err)
	}
	tmp.Close()

	t.Setenv("CONFIG_PATH", tmp.Name())

	cfg, err := Load()
	if err != nil {
		t.Fatalf("Load returned error: %v", err)
	}
	if len(cfg.MCPServers) != 1 {
		t.Fatalf("expected 1 server, got %d", len(cfg.MCPServers))
	}
	s := cfg.MCPServers[0]
	if s.Type != ClientTypeStdio {
		t.Fatalf("expected type stdio, got %s", s.Type)
	}
	if s.Command != "./mock" {
		t.Fatalf("unexpected command: %s", s.Command)
	}
	if len(s.Args) != 1 || s.Args[0] != "--flag" {
		t.Fatalf("unexpected args: %v", s.Args)
	}
	if v := s.Env["foo"]; v != "bar" {
		t.Fatalf("env not parsed: %v", s.Env)
	}
}
