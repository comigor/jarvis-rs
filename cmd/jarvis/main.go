package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"github.com/google/uuid"

	"github.com/comigor/jarvis-go/internal/agent"
	"github.com/comigor/jarvis-go/internal/config"
	"github.com/comigor/jarvis-go/internal/history"
	"github.com/comigor/jarvis-go/internal/logger"
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
        // main inference endpoint (JSON {session_id,input})
        mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
                if r.Method != http.MethodPost {
                        http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
                        return
                }

                var req struct {
                        SessionID string `json:"session_id"`
                        Input     string `json:"input"`
                }
                if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
                        http.Error(w, "invalid JSON", http.StatusBadRequest)
                        return
                }

                sessionID := req.SessionID
                if sessionID == "" {
                        sessionID = uuid.New().String()
                }

                // Save user message
                history.Save(history.Message{SessionID: sessionID, Role: "user", Content: req.Input, CreatedAt: time.Now()})

                // Process input via agent
                output, err := agent.Process(r.Context(), sessionID, req.Input)
                if err != nil {
                        logger.L.Error("agent error", "err", err)
                        http.Error(w, "processing error", http.StatusInternalServerError)
                        return
                }

                // Save assistant message
                history.Save(history.Message{SessionID: sessionID, Role: "assistant", Content: output, CreatedAt: time.Now()})

                w.Header().Set("Content-Type", "application/json")
                _ = json.NewEncoder(w).Encode(map[string]string{"session_id": sessionID, "output": output})
        })



	// debug tool endpoint removed

	// Start server
	serverAddr := fmt.Sprintf("%s:%s", cfg.Server.Host, cfg.Server.Port)
	logger.L.Info("starting server", "address", serverAddr)
	if err := http.ListenAndServe(serverAddr, mux); err != nil {
		logger.L.Error("failed to start server", "error", err)
	}
}
