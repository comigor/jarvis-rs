package main

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"net/http"

	"github.com/comigor/jarvis-go/internal/agent"
	"github.com/comigor/jarvis-go/internal/config"
	"github.com/comigor/jarvis-go/internal/llm"
)

func main() {

	// Load configuration
	cfg, err := config.Load()
	if err != nil {
		slog.Error("failed to load configuration", "error", err)
	}

	// Initialize LLM client
	llmClient := llm.NewClient(cfg.LLM)

	// Initialize agent (ToolManager removed)
	agent := agent.New(llmClient, *cfg) // cfg is now *config.Config

	// Initialize router
	mux := http.NewServeMux()

	// main inference endpoint
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		body, err := io.ReadAll(r.Body)
		if err != nil {
			slog.Error("read body error", "err", err)
			http.Error(w, "failed to read request body", http.StatusBadRequest)
			return
		}
		slog.Info("inference request", "body", string(body))

		response, err := agent.Process(context.Background(), string(body))
		if err != nil {
			slog.Error("process error", "err", err, "body", string(body))
			http.Error(w, "failed to process request", http.StatusInternalServerError)
			return
		}

		w.Write([]byte(response))
	})

	// debug tool endpoint removed

	// Start server
	serverAddr := fmt.Sprintf("%s:%s", cfg.Server.Host, cfg.Server.Port)
	slog.Info("starting server", "address", serverAddr)
	if err := http.ListenAndServe(serverAddr, mux); err != nil {
		slog.Error("failed to start server", "error", err)
	}
}
