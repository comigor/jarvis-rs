package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/jarvis-g2o/internal/agent"
	"github.com/jarvis-g2o/internal/config"
	"github.com/jarvis-g2o/pkg/llm"
	"github.com/jarvis-g2o/pkg/tools"
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

	// Initialize ToolManager
	toolManager := tools.NewToolManager()
	toolManager.RegisterTool(tools.NewHomeAssistantTool(cfg.HomeAssistant))
	toolManager.RegisterTool(&tools.GoogleCalendarTool{})
	toolManager.RegisterTool(tools.NewHomeAssistantListTool(cfg.HomeAssistant))

	// Initialize agent
	agent := agent.New(llmClient, cfg.LLM, toolManager)

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

	// debug tool endpoint
	r.Post("/debug/tool", func(w http.ResponseWriter, r *http.Request) {
		var req struct {
			Tool string `json:"tool"`
			Args string `json:"args"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			sugar.Errorw("decode error", "err", err)
			http.Error(w, "bad json", http.StatusBadRequest)
			return
		}

		t, err := toolManager.GetTool(req.Tool)
		if err != nil {
			sugar.Errorw("tool lookup", "tool", req.Tool, "err", err)
			http.Error(w, "tool not found", http.StatusBadRequest)
			return
		}

		res, err := t.Run(req.Args)
		if err != nil {
			sugar.Errorw("tool run", "tool", req.Tool, "err", err)
			http.Error(w, "tool error", http.StatusInternalServerError)
			return
		}

		w.Write([]byte(res))
	})
	// Start server
	serverAddr := fmt.Sprintf("%s:%s", cfg.Server.Host, cfg.Server.Port)
	sugar.Infof("starting server on %s", serverAddr)
	if err := http.ListenAndServe(serverAddr, r); err != nil {
		sugar.Fatalf("failed to start server: %v", err)
	}
}
