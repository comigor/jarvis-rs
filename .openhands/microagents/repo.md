# repo.md

This file provides guidance to agents when working with code in this repository.

---

## Common commands

| Task | Command |
|------|---------|
| Build binary | `make build` → creates `bin/jarvis` |
| Run API locally | `make run` (defaults to `:8080`) |
| Full test suite | `make test` or `go test ./...` |
| Single test | `go test -run ^TestName$ ./path/...` |
| Lint & fmt | `make lint` (runs `go vet` + `gofmt`) |
| Format imports | `goimports -w .` |
| Dependency tidy | `go mod tidy && go mod vendor` |

All targets assume Go ≥ 1.24.4 is available.

---

## High-level architecture

```
cmd/
  jarvis/          → program entry; very thin main()
internal/
  agent/           → core request processor (FSM + MCP + LLM)
  config/          → config loading via viper; defines Config struct
  llm/             → OpenAI-compatible client wrapper
  logger/          → slog initialisation (JSON handler)
  history/         → SQLite-backed session/message persistence
```

1. **Startup** (`cmd/jarvis/main.go`)
   • Creates a JSON `slog` logger and sets it as default.
   • Loads `config.yaml` (see README for sample).
   • Builds an `llm.Client` and an `agent.Agent`, then starts an HTTP server.

2. **HTTP layer**
   • Single `POST /` endpoint reads plain-text prompt and passes it to `agent.Agent.Process`.

3. **Agent FSM** (`internal/agent/agent.go`)
   • Stateless FSM with states `ReadyToCallLLM → ExecutingTools → Done/Error`.
   • Aggregates system prompts (base + any supplied by connected MCP servers).
   • Talks to the OpenAI client (`internal/llm`) and may call external MCP tools when the LLM requests them.

4. **MCP integration**
   • For each configured `mcp_servers` entry the agent creates a streaming client (`github.com/mark3labs/mcp-go`).
   • Discovers available tools + optional `system_prompts` and exposes them to the LLM.

5. **Configuration** (`config.yaml`)
   • `server` (host/port) ‑ HTTP listener.
   • `llm` (base_url, api_key, model, system_prompt).
   • `mcp_servers` list with `name`, `type` (`sse`, `streamable_http` or `stdio`), and then type-specific `command`, `args`, `url`, `headers`.

---

## Conventions & rules (from AGENTS.md)

* Always run `make build` before finishing a task.
* Keep `main` thin; place logic in internal packages.
* Import order: stdlib, third-party, `github.com/comigor/jarvis-go/...`.
* Tabs, 8-column width, `gofmt` enforced.
* Use `slog` for logging; no `fmt.Printf` in production code.
* Tests are table-driven, live next to code as `*_test.go` and should not hit the network.
* Interface names end with *er* (`Closer`), exported symbols need doc comments.
* Commits: imperative present tense, ≤72 chars.
