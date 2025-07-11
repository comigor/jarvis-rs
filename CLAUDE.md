# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Build and Run
```bash
make build          # Build binary to bin/jarvis
make run            # Run application directly
go run ./cmd/jarvis # Alternative run command
```

### Testing and Quality
```bash
make test           # Run all tests
go test ./...       # Alternative test command
go test ./... -run TestName # Run specific test
make lint           # Run go vet and gofmt (formatting)
```

## Architecture Overview

J.A.R.V.I.S. is a smart-home agent server that processes natural language commands and executes them via MCP (Model Context Protocol) tools. The architecture follows clean separation with these key components:

### Core Flow
1. HTTP server receives POST requests with plain text prompts
2. Agent processes input using finite state machine (FSM) for conversation management
3. LLM integration via OpenAI-compatible API generates responses and tool calls
4. MCP clients execute tools on external systems
5. Conversation history persisted in SQLite with in-memory fallback

### Key Components

**Agent (`internal/agent/agent.go`)**
- Implements FSM-based conversation flow with states: ReadyToCallLLM → ExecutingTools → Done/Error
- Manages MCP client connections and tool discovery
- Handles LLM tool calling workflow with proper error handling and turn limits
- Aggregates system prompts from MCP servers with configurable base prompt

**Configuration (`internal/config/config.go`)**
- Supports multiple MCP client types: SSE, Streamable HTTP, and Stdio
- YAML-based configuration with environment variable override support
- LLM configuration with custom base URLs and system prompts

**History (`internal/history/history.go`)**
- SQLite-based persistence with automatic fallback to in-memory storage
- Session-based message tracking with chronological ordering
- Graceful degradation when database is unavailable

### MCP Integration
The system supports connecting to multiple MCP servers simultaneously:
- Tool discovery and registration at startup
- System prompt aggregation from MCP servers
- Runtime tool execution with proper error handling
- Support for SSE, HTTP streaming, and stdio transports

### Configuration Requirements
Create `config.yaml` with:
- Server host/port settings
- LLM configuration (OpenAI-compatible API)
- MCP server connections with authentication
- Optional custom system prompts

### API Usage
Send POST requests to `/` with JSON:
```json
{
  "session_id": "optional-session-id",
  "input": "Turn on kitchen light"
}
```

### Testing Strategy
- Unit tests for core components (agent, config)
- Error handling tests for MCP failures
- FSM state transition validation
- Configuration loading edge cases