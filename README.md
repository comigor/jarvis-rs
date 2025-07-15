# J.A.R.V.I.S.

Smart-home agent server built in Rust with MCP (Model Context Protocol) support.

## Features

- **Natural Language Processing**: Process plain-text commands via HTTP API
- **MCP Integration**: Connect to multiple Model Context Protocol servers
- **Tool Routing**: Intelligent tool-to-client mapping for distributed MCP environments
- **Conversation Management**: FSM-based conversation flow with history persistence
- **Multiple Transports**: Support for SSE, HTTP, and stdio MCP connections
- **Robust Testing**: 95+ comprehensive tests covering all functionality

## Quick Start

### Prerequisites
- Rust 1.88+ (Edition 2024)
- Cargo

### Build and Run
```bash
# Build release binary
make build

# Run development server
make run
```

The server will start on `http://localhost:8080` by default.

### API Usage
Send POST requests with JSON to `/`:
```bash
curl -X POST http://localhost:8080/ \
  -H "Content-Type: application/json" \
  -d '{"input": "Turn on kitchen light"}'
```

With session ID:
```bash
curl -X POST http://localhost:8080/ \
  -H "Content-Type: application/json" \
  -d '{"session_id": "my-session", "input": "What is the weather like?"}'
```

## Configuration

Create `config.yaml` in the project root:

```yaml
server:
  host: "0.0.0.0"
  port: 8080
  database_path: "history.db"
  logs:
    level: "info"

llm:
  provider: "openai"
  base_url: "https://api.openai.com/v1"
  api_key: "YOUR_OPENAI_API_KEY"
  model: "gpt-4o-mini"
  # Optional: Custom system prompt
  # system_prompt: "You are a helpful smart home assistant."

mcp_servers:
  # SSE (Server-Sent Events) connection
  - name: "home-assistant"
    type: "sse"
    url: "http://localhost:8123/mcp_server/sse"
    headers:
      Authorization: "Bearer YOUR_HA_TOKEN"
  
  # HTTP streaming connection
  - name: "weather-service"
    type: "streamable_http"
    url: "http://localhost:3001/mcp"
    headers:
      API-Key: "YOUR_WEATHER_API_KEY"
  
  # Stdio process connection
  - name: "filesystem-tools"
    type: "stdio"
    command: "python"
    args: ["-m", "mcp_filesystem_server"]
    env:
      MCP_FILESYSTEM_ROOT: "/home/user/documents"
```

### Environment Variables
- `HISTORY_DB_PATH`: Override database path
- `RUST_LOG`: Set log level (`error`, `warn`, `info`, `debug`, `trace`)

## Development

### Testing
```bash
# Run all tests
make test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
```

### Linting
```bash
# Run clippy and format checks
make lint

# Auto-format code
cargo fmt
```

### Architecture

J.A.R.V.I.S. follows a clean architecture with these key components:

#### Core Flow
1. **HTTP Server**: Receives POST requests with natural language input
2. **Agent**: Processes input using FSM for conversation management  
3. **LLM Integration**: Communicates with OpenAI-compatible APIs
4. **MCP Clients**: Execute tools on external systems via MCP protocol
5. **Tool Routing**: Maps tools to correct MCP clients automatically
6. **History**: Persists conversations in SQLite with fallback

#### Key Components

- **Agent** (`src/agent/`): FSM-based conversation flow with tool execution
- **MCP Client** (`src/mcp_client.rs`): Handles multiple MCP transport types
- **Tool Mapping**: Routes tools to correct clients based on discovery
- **Configuration** (`src/config/`): YAML-based config with environment overrides
- **History** (`src/history/`): SQLite persistence with in-memory fallback

### MCP Integration

The system supports connecting to multiple MCP servers simultaneously:

- **Tool Discovery**: Automatically discovers available tools from each server
- **Tool Routing**: Maps each tool to its originating MCP client
- **System Prompts**: Aggregates prompts from MCP servers
- **Transport Support**: SSE, HTTP streaming, and stdio connections
- **Error Handling**: Graceful degradation when servers are unavailable

### Testing Strategy

- **Unit Tests**: Core components (agent, config, FSM, history)
- **Integration Tests**: Full agent flows and server endpoints  
- **MCP Tests**: Tool discovery, execution, and error scenarios
- **Mock Framework**: Comprehensive mocks for LLM and MCP clients

## Troubleshooting

### Common Issues

**Configuration not found**: Ensure `config.yaml` exists in the project root

**MCP connection failed**: Check that MCP servers are running and URLs are correct

**Tool execution errors**: Verify tool-to-client mappings in logs with `RUST_LOG=debug`

**Database errors**: Check file permissions for `database_path` or use `:memory:`

### Logs

Enable debug logging to see detailed execution flow:
```bash
RUST_LOG=debug cargo run
```

For specific component logging:
```bash
RUST_LOG=jarvis_rust::agent=debug,jarvis_rust::mcp=trace cargo run
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Run `make test` and `make lint`
5. Submit a pull request

## License

This project is licensed under the MIT License.