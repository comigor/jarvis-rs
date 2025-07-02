# AGENT QUICK-START (≈20 lines)

1. Build: `make build` (creates `bin/jarvis`), run: `make run`.
2. Full test suite: `go test ./...`  ‑ use `go test -run ^TestName$ ./path/...` for a single test.
3. Lint/format: `go vet ./...`, `go fmt ./...`; prefer `golangci-lint run` if installed.
4. Dependency tidy: `go mod tidy && go mod vendor`.
5. Import order: stdlib, third-party, then `github.com/jarvis-g2o/...`, separated by blank lines; run `goimports`.
6. Formatting: always `gofmt` (tabs, 8-space width), no extra comments unless exported symbol.
7. Types & naming: CamelCase; exported identifiers start upper-case, unexported lower-case; interfaces end with *er* (e.g. `Closer`).
8. Errors: return early, wrap with `fmt.Errorf("%w", err)`; no panics outside `main`.
9. Context: pass `context.Context` as first arg when IO/remote calls occur; never store in struct.
10. Logging: use `go.uber.org/zap`; no `fmt.Printf` in production code.
11. Tests: table-driven; keep files `*_test.go`; avoid network I/O, use fakes.
12. Concurrency: use `conc` helpers or `sync`; guard shared data; avoid goroutine leaks.
13. Config: use `viper`; keep defaults in `internal/config`.
14. Tools: register in `pkg/tools`; managers in `pkg/tools/manager.go`.
15. Binary entrypoint: `cmd/jarvis/main.go`; keep `main` thin.
16. No Cursor (.cursor) or Copilot rules present; honor this guide instead.
17. Commit style: imperative present tense, ≤72 char summary.
18. Keep `.go` files under 500 lines; split packages logically.
19. Public API must have doc comments starting with the identifier.
20. Prefer composition over inheritance; avoid cyclic imports.