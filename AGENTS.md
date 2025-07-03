# AGENT QUICK-START (≈20 lines)

1. Requires Go 1.24.4+. Build: `make build` (`bin/jarvis`); run: `make run`.
2. After every change `make build` **or** `go vet ./... && go test ./...`; fix all errors.
3. Full tests: `go test ./...`. Single test: `go test -run ^TestName$ ./path/...`.
4. Lint/format: `go vet ./...`, `go fmt ./...`; prefer `golangci-lint run` when available.
5. Tidy deps: `go mod tidy && go mod vendor`.
6. Import groups: stdlib, third-party, `github.com/jarvis-g2o/...`; blank line between; run `goimports`.
7. Formatting: `gofmt` (tabs, width 8); avoid comments unless exporting a symbol.
8. Naming: CamelCase; exported upper-case, unexported lower; interfaces end *er* (`Closer`).
9. Errors: return early, wrap with `fmt.Errorf("%w", err)`; no panics outside `main`.
10. Context: first arg `context.Context` for IO/remote calls; never store in structs.
11. Logging: use `go.uber.org/zap`; no `fmt.Printf` in production.
12. Tests: table-driven files `*_test.go`; no network; use fakes.
13. Concurrency: prefer `conc` helpers or `sync`; guard shared data; avoid leaks.
14. Config: `viper`; defaults live in `internal/config`.
15. Tools: register in `pkg/tools`; managers in `pkg/tools/manager.go`.
16. Binary entry: `cmd/jarvis/main.go`; keep `main` thin.
17. Files ≤500 LOC; split packages logically.
18. Public API needs doc comments starting with the identifier.
19. Commits: imperative present tense, ≤72 chars; opencode footer on auto-commits.
20. No Cursor or Copilot rules found; follow this guide.
