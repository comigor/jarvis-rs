# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Build and Run
```bash
make build          # Build release binary to bin/jarvis
make run            # Run application directly
cargo run           # Alternative run command
```

### Testing and Quality
```bash
make test           # Run all tests
cargo test          # Alternative test command
cargo test TestName # Run specific test
make lint           # Run clippy (linter) and rustfmt (formatter)
```

## Architecture Overview

J.A.R.V.I.S. is a smart-home agent server that processes natural language commands and executes them via MCP (Model Context Protocol) tools. The architecture is built entirely in Rust, leveraging modern async programming with tokio and axum.

### Core Flow
1. Axum HTTP server receives POST requests with plain text prompts
2. Agent processes input using finite state machine (FSM) for conversation management
3. LLM integration via OpenAI-compatible API generates responses and tool calls
4. MCP clients execute tools on external systems
5. Conversation history persisted in SQLite with in-memory fallback

### Key Components

**Agent (`src/agent/executor.rs`)**
- Implements custom FSM-based conversation flow with states: ReadyToCallLlm → AwaitingLlmResponse → ExecutingTools → Done/Error
- Manages MCP client connections and tool discovery
- Handles LLM tool calling workflow with proper error handling and turn limits (max 5 turns)
- Aggregates system prompts from MCP servers with configurable base prompt

**FSM (`src/agent/fsm.rs`)**
- Custom finite state machine implementation for conversation flow
- Handles state transitions and context management
- Enforces turn limits to prevent infinite loops
- Proper error propagation and terminal state handling

**Configuration (`src/config/mod.rs`)**
- Supports multiple MCP client types: SSE, Streamable HTTP, and Stdio
- YAML-based configuration with environment variable override support
- LLM configuration with custom base URLs and system prompts

**History (`src/history/storage.rs`)**
- SQLite-based persistence with automatic fallback to in-memory storage
- Session-based message tracking with chronological ordering
- Graceful degradation when database is unavailable

**LLM Client (`src/llm/client.rs`)**
- OpenAI-compatible API integration using async-openai
- Supports tool calling and chat completions
- Configurable temperature and token limits

**MCP Integration (`src/mcp_client.rs`)**
- Built on the official `rmcp` crate for robust MCP support
- Supports connecting to multiple MCP servers simultaneously
- Tool discovery and registration at startup
- System prompt aggregation from MCP servers
- Runtime tool execution with proper error handling
- Support for SSE, HTTP streaming, and stdio transports

### Testing Strategy

The codebase includes comprehensive tests with feature parity validation:

**Unit Tests**
- FSM state transitions and logic (`src/agent/fsm.rs`)
- Configuration loading and validation (`src/config/mod.rs`)
- History storage and retrieval (`src/history/storage.rs`)

**Integration Tests**
- Agent flow testing (`tests/agent_flows.rs`)
- MCP client integration (`tests/mcp_integration.rs`)
- Server integration tests (`tests/server_integration.rs`)

**Parity Tests**
- Go-to-Rust feature parity validation (`tests/agent_parity_tests.rs`)
- Ensures identical behavior for all critical scenarios:
  - Direct LLM responses
  - Successful tool execution
  - MCP client failures
  - Sequential tool calls
  - Max turns exceeded handling

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

### Implementation Notes
- Pure Rust implementation (Go codebase removed)
- Async/await throughout for optimal performance
- Comprehensive error handling with proper error types
- Turn limit enforcement prevents infinite LLM ↔ Tool loops
- Graceful degradation when MCP servers are unavailable
- Session-based conversation history persistence