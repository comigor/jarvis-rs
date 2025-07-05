package main

import (
	"context"
	"fmt"
	"io"
	"net/http"

	"github.com/comigor/jarvis-go/internal/logger"

	"github.com/comigor/jarvis-go/internal/agent"
	"github.com/comigor/jarvis-go/internal/config"
	"github.com/comigor/jarvis-go/internal/llm"
)

func main() {

	// Load configuration
	cfg, err := config.Load()
	if err != nil {
		logger.L.Error("failed to load configuration", "error", err)
	}

	// Set logger level
	logger.SetLevel(cfg.Server.Logs.Level)

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
			logger.L.Error("read body error", "err", err)
			http.Error(w, "failed to read request body", http.StatusBadRequest)
			return
		}
		logger.L.Info("inference request", "body", string(body))

		response, err := agent.Process(context.Background(), string(body))
		if err != nil {
			logger.L.Error("process error", "err", err, "body", string(body))
			http.Error(w, "failed to process request", http.StatusInternalServerError)
			return
		}

		if _, writeErr := w.Write([]byte(response)); writeErr != nil {
			logger.L.Warn("response write error", "err", writeErr)
		}
	})

	// debug tool endpoint removed

	// Start server
	serverAddr := fmt.Sprintf("%s:%s", cfg.Server.Host, cfg.Server.Port)
	logger.L.Info("starting server", "address", serverAddr)
	if err := http.ListenAndServe(serverAddr, mux); err != nil {
		logger.L.Error("failed to start server", "error", err)
	}
}
