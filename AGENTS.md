# AGENT QUICK-START

1. Requires Go 1.24.4+. Build: `make build` (`bin/jarvis`); run: `make run`.
2. After every change `make build` **or** `go vet ./... && go test ./...`; fix all errors.
3. Full tests: `go test ./...`. Single test: `go test -run ^TestName$ ./path/...`.
4. Lint/format: `go vet ./...`, `go fmt ./...`; prefer `golangci-lint run` when available.
5. Tidy deps: `go mod tidy && go mod vendor`.
6. Import groups: stdlib, third-party, `github.com/comigor/jarvis-go/...`; blank line between; run `goimports`.
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

## General Process Tips & Learnings

*   **Build and Vet Frequently:** Before submitting or marking a coding task as complete, always run the project's build and linting/vetting commands (e.g., `make build`, `go vet ./...`). This helps catch errors early, even if intermediate steps in a plan don't explicitly require it.
*   **Verify External Library Interfaces:** When working with external libraries, especially for their data structures (structs) or method signatures:
    *   If the development environment provides tools to inspect the actual source code of the downloaded dependencies (e.g., `grep` on the module cache), use them to confirm definitions. This is more reliable than relying solely on public documentation or memory, which might be for a different version.
    *   Pay close attention to error messages from the compiler or static analysis tools (`go vet`). They provide direct feedback on how the code is being interpreted in the current environment.
*   **Iterative Problem Solving:** When faced with persistent errors related to external libraries (e.g., "field undefined" or "no index operator"), iteratively hypothesize the cause, attempt a fix, and re-verify with build/vet tools. If direct inspection isn't possible, use `go vet` errors as the primary guide to understanding the actual types being used.
