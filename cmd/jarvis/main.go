package main

import (
	"context"
	"fmt"
	"io"
	"log"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/jarvis-g2o/internal/agent"
	"github.com/jarvis-g2o/internal/config"
	"github.com/jarvis-g2o/pkg/llm"
	"go.uber.org/zap"
)

func main() {
	// Initialize logger
	logger, err := zap.NewProduction()
	if err != nil {
		log.Fatalf("can't initialize zap logger: %v", err)
	}
	zap.ReplaceGlobals(logger)
	sugar := logger.Sugar()

	// Load configuration
	cfg, err := config.Load()
	if err != nil {
		sugar.Fatalf("failed to load configuration: %v", err)
	}

	// Initialize LLM client
	llmClient := llm.NewClient(cfg.LLM)

	// Initialize agent (ToolManager removed)
	agent := agent.New(llmClient, *cfg) // cfg is now *config.Config

	// Initialize router
	r := chi.NewRouter()

	// main inference endpoint
	r.Post("/", func(w http.ResponseWriter, r *http.Request) {
		body, err := io.ReadAll(r.Body)
		if err != nil {
			sugar.Errorw("read body error", "err", err)
			http.Error(w, "failed to read request body", http.StatusBadRequest)
			return
		}
		sugar.Infow("inference request", "body", string(body))

		response, err := agent.Process(context.Background(), string(body))
		if err != nil {
			sugar.Errorw("process error", "err", err, "body", string(body))
			http.Error(w, "failed to process request", http.StatusInternalServerError)
			return
		}

		w.Write([]byte(response))
	})

	// debug tool endpoint removed

	// Start server
	serverAddr := fmt.Sprintf("%s:%s", cfg.Server.Host, cfg.Server.Port)
	sugar.Infof("starting server on %s", serverAddr)
	if err := http.ListenAndServe(serverAddr, r); err != nil {
		sugar.Fatalf("failed to start server: %v", err)
	}
}
