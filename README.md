# J.A.R.V.I.S.

Smart-home agent server built in Go 1.24.4.

## Quick start
```bash
# build binary
make build
# run API on :8080
make run
```

POST plain-text prompts to `/`:
```bash
curl -X POST http://localhost:8080/ --data 'Turn on kitchen light'
```

## Tests & lint
```bash
make test              # all tests
go test ./... -run Foo # single test
make vet               # go vet lint (alias)
```

## Configuration
Create `config.yaml`:
```yaml
server:
  host: 0.0.0.0
  port: "8080"

llm:
  base_url: https://api.openai.com/v1
  api_key: YOUR_KEY
  model: gpt-4o-mini
  # Optional: Sets the base system prompt for the LLM.
  # If not set, a hardcoded default system prompt will be used as the base.
  # Afterwards, any system prompts discovered from connected MCP servers will be
  # appended to this base prompt (each on a new line).
  # system_prompt: "You are a master chef specializing in Italian cuisine."

mcp_servers:
  # - url: "http://localhost:3000/mcp"
  #   headers:
  #     Authorization: "Bearer ..."
  - type: "sse"
    url: "http://localhost:8123/mcp_server/sse"
```
