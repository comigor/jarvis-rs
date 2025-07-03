package agent

import (
	"context"
	"encoding/json"
	"errors" // Added for errors.New
	"fmt"    // For fmt.Errorf

	"github.com/jarvis-g2o/internal/config"
	"github.com/jarvis-g2o/pkg/llm"

	"github.com/mark3labs/mcp-go/client"
	"github.com/mark3labs/mcp-go/client/transport"
	"github.com/mark3labs/mcp-go/mcp"

	"github.com/sashabaranov/go-openai"
	"go.uber.org/zap"

	"github.com/qmuntal/stateless" // FSM library
)

// FSM States
type FSMState stateless.State

var (
	StateReadyToCallLLM      FSMState = "ReadyToCallLLM"
	StateAwaitingLLMResponse FSMState = "AwaitingLLMResponse"
	StateExecutingTools      FSMState = "ExecutingTools"
	StateDone                FSMState = "Done"  // Terminal: successful completion
	StateError               FSMState = "Error" // Terminal: error state
)

// FSM Triggers
type FSMTrigger stateless.Trigger

var (
	TriggerProcessInput            FSMTrigger = "ProcessInput"
	TriggerLLMRespondedWithContent FSMTrigger = "LLMRespondedWithContent"
	TriggerLLMRequestedTools       FSMTrigger = "LLMRequestedTools"
	TriggerToolsExecutionCompleted FSMTrigger = "ToolsExecutionCompleted"
	TriggerToolsExecutionFailed    FSMTrigger = "ToolsExecutionFailed" // For errors during tool execution phase
	TriggerErrorOccurred           FSMTrigger = "ErrorOccurred"        // For general errors like LLM call failure
)

// MCPClientInterface defines the methods our agent expects from an MCP client.
type MCPClientInterface interface {
	Initialize(ctx context.Context, req mcp.InitializeRequest) (*mcp.InitializeResult, error)
	ListTools(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error)
	CallTool(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error)
	Close() error
}

// Agent is the main agent struct
type Agent struct {
	llmClient         llm.Client
	cfg               config.LLMConfig
	mcpClients        []MCPClientInterface
	availableLLMTools []openai.Tool
}

// New creates a new agent.
func New(llmClient llm.Client, appCfg config.Config) *Agent {
	initializedMcpClients := make([]MCPClientInterface, 0, len(appCfg.MCPServers))
	aggregatedLLMTools := make([]openai.Tool, 0)
	toolNameSet := make(map[string]struct{}) // To ensure unique tool names for the LLM

	backgroundCtx := context.Background() // For setup tasks like Initialize and ListTools

	for _, serverCfg := range appCfg.MCPServers {
		var mcpC *client.Client // Concrete client type from mcp-go
		var err error

		// Create client based on serverCfg.Type
		switch serverCfg.Type {
		case config.ClientTypeSSE:
			var sseOpts []transport.ClientOption
			if len(serverCfg.Headers) > 0 {
				sseOpts = append(sseOpts, transport.WithHeaders(serverCfg.Headers))
			}
			mcpC, err = client.NewSSEMCPClient(serverCfg.URL, sseOpts...)
		case config.ClientTypeStreamableHTTP:
			var httpOpts []transport.StreamableHTTPCOption
			if len(serverCfg.Headers) > 0 {
				httpOpts = append(httpOpts, transport.WithHTTPHeaders(serverCfg.Headers))
			}
			mcpC, err = client.NewStreamableHttpClient(serverCfg.URL, httpOpts...)
		default:
			if serverCfg.Type == "" {
				zap.S().Warnf("MCP server type not specified for URL %s. Skipping. Please set 'type' in config.yaml ('sse' or 'streamable_http').", serverCfg.URL)
			} else {
				zap.S().Warnf("Unsupported MCP server type '%s' for URL %s. Skipping. Supported types are 'sse' or 'streamable_http'.", serverCfg.Type, serverCfg.URL)
			}
			continue
		}

		if err != nil {
			zap.S().Errorf("Failed to create MCP client for server %s (type: %s): %v", serverCfg.URL, serverCfg.Type, err)
			continue
		}

		// Start the client transport
		err = mcpC.Start(backgroundCtx)
		if err != nil {
			zap.S().Errorf("Failed to start MCP client transport for server %s: %v", serverCfg.URL, err)
			mcpC.Close() // Attempt to close if start failed
			continue
		}

		// Initialize client
		initReq := mcp.InitializeRequest{
			Params: mcp.InitializeParams{Capabilities: mcp.ClientCapabilities{}}, // TODO: Populate capabilities
		}
		_, err = mcpC.Initialize(backgroundCtx, initReq)
		if err != nil {
			zap.S().Errorf("Failed to initialize MCP client for server %s: %v", serverCfg.URL, err)
			mcpC.Close() // Attempt to close if initialization failed
			continue
		}
		initializedMcpClients = append(initializedMcpClients, mcpC)

		// List tools from this client
		listToolsReq := mcp.ListToolsRequest{}
		serverTools, listErr := mcpC.ListTools(backgroundCtx, listToolsReq)
		if listErr != nil {
			zap.S().Warnf("Failed to list tools for MCP client %s: %v", serverCfg.URL, listErr)
			// Continue with the client even if ListTools fails, it might support other operations.
		}

		if serverTools != nil {
			for _, mcpTool := range serverTools.Tools {
				if _, exists := toolNameSet[mcpTool.Name]; !exists {
					var paramsSchema json.RawMessage
					if mcpTool.RawInputSchema != nil && len(mcpTool.RawInputSchema) > 0 && string(mcpTool.RawInputSchema) != "null" {
						paramsSchema = mcpTool.RawInputSchema
					} else {
						schemaBytes, marshalErr := json.Marshal(mcpTool.InputSchema)
						if marshalErr != nil {
							zap.S().Errorf("Failed to marshal InputSchema for tool '%s': %v. Using empty schema.", mcpTool.Name, marshalErr)
							paramsSchema = json.RawMessage(`{"type": "object", "properties": {}}`)
						} else {
							paramsSchema = json.RawMessage(schemaBytes)
							if string(paramsSchema) == "{}" || string(paramsSchema) == "null" {
								if mcpTool.RawInputSchema == nil || len(mcpTool.RawInputSchema) == 0 || string(mcpTool.RawInputSchema) == "null" {
									zap.S().Warnf("Tool '%s' from MCP server %s has an empty or null schema (InputSchema: %s). Using default empty object schema for LLM.", mcpTool.Name, serverCfg.URL, string(paramsSchema))
									paramsSchema = json.RawMessage(`{"type": "object", "properties": {}}`)
								}
							}
						}
					}
					if paramsSchema == nil {
						paramsSchema = json.RawMessage(`{"type": "object", "properties": {}}`)
						zap.S().Warnf("Tool '%s' from MCP server %s resulted in nil schema. Using default empty object schema.", mcpTool.Name, serverCfg.URL)
					}

					toolNameSet[mcpTool.Name] = struct{}{}
					llmTool := openai.Tool{
						Type: openai.ToolTypeFunction,
						Function: &openai.FunctionDefinition{
							Name:        mcpTool.Name,
							Description: mcpTool.Description,
							Parameters:  paramsSchema,
						},
					}
					aggregatedLLMTools = append(aggregatedLLMTools, llmTool)
					zap.S().Infof("Registered tool '%s' from MCP server %s for LLM", mcpTool.Name, serverCfg.URL)
				} else {
					zap.S().Warnf("Tool '%s' from MCP server %s already registered from another server. Skipping.", mcpTool.Name, serverCfg.URL)
				}
			}
		}
	}

	if len(initializedMcpClients) == 0 && len(appCfg.MCPServers) > 0 {
		zap.S().Warnf("No MCP clients were successfully initialized despite %d servers configured.", len(appCfg.MCPServers))
	}
	if len(aggregatedLLMTools) == 0 && len(appCfg.MCPServers) > 0 && len(initializedMcpClients) > 0 {
		zap.S().Info("MCP Clients initialized, but no tools found or registered from any MCP server for LLM.")
	}

	return &Agent{
		llmClient:         llmClient,
		cfg:               appCfg.LLM,
		mcpClients:        initializedMcpClients,
		availableLLMTools: aggregatedLLMTools,
	}
}

// Process processes a request and returns a response.
// Process uses a Finite State Machine to manage the conversation flow with the LLM and tool calls.
func (a *Agent) Process(ctx context.Context, request string) (string, error) {
	// FSM context data
	type fsmContext struct {
		messages     []openai.ChatCompletionMessage
		llmResponse  *openai.ChatCompletionResponse
		toolCalls    []openai.ToolCall
		toolResults  []openai.ChatCompletionMessage
		finalContent string
		lastError    error
		currentTurn  int
		maxTurns     int
	}

	fsmCtx := &fsmContext{
		messages: []openai.ChatCompletionMessage{{Role: openai.ChatMessageRoleUser, Content: request}},
		maxTurns: 5, // Max interaction turns (LLM -> Tool -> LLM = 1 turn)
	}

	fsm := stateless.NewStateMachine(StateReadyToCallLLM)

	// State: ReadyToCallLLM
	// Action: Call LLM with current messages.
	// Transitions:
	//   - On LLMRequestedTools -> StateExecutingTools
	//   - On LLMRespondedWithContent -> StateDone
	//   - On ErrorOccurred -> StateError
	fsm.Configure(StateReadyToCallLLM).
		PermitReentry(TriggerProcessInput). // Added to ensure OnEntry is called by the initial Fire
		OnEntry(func(ctx context.Context, args ...any) error {
			// Check if this OnEntry is due to the initial TriggerProcessInput
			// We only want the LLM call logic to run once per "real" entry, not on the artificial starter trigger if args are empty.
			// However, the first call to Fire will pass `ctx` as an arg.
			// The main logic of OnEntry should proceed.

			if fsmCtx.currentTurn >= fsmCtx.maxTurns {
				zap.S().Warnf("Max interaction turns (%d) reached.", fsmCtx.maxTurns)
				fsmCtx.lastError = errors.New("exceeded maximum interaction turns")
				fsm.Fire(TriggerErrorOccurred, ctx) // Use specific trigger if stateless supports it directly in OnEntry
				return nil                          // Or return the error if OnEntry allows it to halt further processing
			}
			fsmCtx.currentTurn++
			zap.S().Infof("FSM: Entering StateReadyToCallLLM, turn %d", fsmCtx.currentTurn)

			llmResp, err := a.llmClient.CreateChatCompletion(
				ctx,
				openai.ChatCompletionRequest{
					Model:    a.cfg.Model,
					Messages: fsmCtx.messages,
					Tools:    a.availableLLMTools,
				},
			)
			if err != nil {
				zap.S().Errorf("LLM call failed: %v", err)
				fsmCtx.lastError = err
				return fsm.Fire(TriggerErrorOccurred, ctx)
			}
			fsmCtx.llmResponse = &llmResp
			zap.S().Infow("LLM response received", "response", llmResp)

			if len(llmResp.Choices) > 0 && len(llmResp.Choices[0].Message.ToolCalls) > 0 {
				return fsm.Fire(TriggerLLMRequestedTools, ctx)
			}
			return fsm.Fire(TriggerLLMRespondedWithContent, ctx)
		}).
		Permit(TriggerLLMRequestedTools, StateExecutingTools).
		Permit(TriggerLLMRespondedWithContent, StateDone).
		Permit(TriggerErrorOccurred, StateError)

	// State: ExecutingTools
	// Action: Process tool calls from LLM response, execute them via MCP.
	// Transitions:
	//   - On ToolsExecutionCompleted -> StateReadyToCallLLM (to send results back)
	//   - On ToolsExecutionFailed -> StateError
	fsm.Configure(StateExecutingTools).
		OnEntry(func(ctx context.Context, args ...any) error {
			zap.S().Info("FSM: Entering StateExecutingTools")
			if fsmCtx.llmResponse == nil || len(fsmCtx.llmResponse.Choices) == 0 {
				fsmCtx.lastError = errors.New("cannot execute tools, no LLM response available")
				return fsm.Fire(TriggerErrorOccurred, ctx)
			}

			llmMessage := fsmCtx.llmResponse.Choices[0].Message
			fsmCtx.messages = append(fsmCtx.messages, llmMessage) // Add assistant's message (with tool call requests) to history
			fsmCtx.toolCalls = llmMessage.ToolCalls
			fsmCtx.toolResults = make([]openai.ChatCompletionMessage, 0, len(fsmCtx.toolCalls))

			if len(a.mcpClients) == 0 && len(fsmCtx.toolCalls) > 0 {
				zap.S().Warn("LLM requested tools, but no MCP clients are available.")
				// Create error results for each tool call
				for _, tc := range fsmCtx.toolCalls {
					fsmCtx.toolResults = append(fsmCtx.toolResults, openai.ChatCompletionMessage{
						Role:       openai.ChatMessageRoleTool,
						Content:    "Error: No MCP clients available to execute tool " + tc.Function.Name,
						ToolCallID: tc.ID,
						Name:       tc.Function.Name,
					})
				}
				return fsm.Fire(TriggerToolsExecutionCompleted, ctx) // Proceed to send these errors back to LLM
			}

			for _, toolCall := range fsmCtx.toolCalls {
				var toolArgs map[string]any
				if err := json.Unmarshal([]byte(toolCall.Function.Arguments), &toolArgs); err != nil {
					zap.S().Errorf("Failed to unmarshal tool arguments for %s: %v", toolCall.Function.Name, err)
					fsmCtx.toolResults = append(fsmCtx.toolResults, openai.ChatCompletionMessage{
						Role:       openai.ChatMessageRoleTool,
						Content:    "Error: Could not parse arguments for tool " + toolCall.Function.Name,
						ToolCallID: toolCall.ID,
						Name:       toolCall.Function.Name,
					})
					continue
				}

				toolOutput := a.executeMCPTool(ctx, toolCall.Function.Name, toolArgs)
				fsmCtx.toolResults = append(fsmCtx.toolResults, openai.ChatCompletionMessage{
					Role:       openai.ChatMessageRoleTool,
					Content:    toolOutput,
					ToolCallID: toolCall.ID,
					Name:       toolCall.Function.Name,
				})
			}
			// Append all tool results to messages before transitioning back to ReadyToCallLLM
			fsmCtx.messages = append(fsmCtx.messages, fsmCtx.toolResults...)
			return fsm.Fire(TriggerToolsExecutionCompleted, ctx)
		}).
		Permit(TriggerToolsExecutionCompleted, StateReadyToCallLLM).
		Permit(TriggerErrorOccurred, StateError) // Could also be ToolsExecutionFailed if we want distinct error handling

	// State: Done
	// Action: Extract final content from LLM response. This is a terminal state.
	fsm.Configure(StateDone).
		OnEntry(func(ctx context.Context, args ...any) error {
			zap.S().Info("FSM: Entering StateDone")
			if fsmCtx.llmResponse != nil && len(fsmCtx.llmResponse.Choices) > 0 {
				// If the last LLM response had tool calls, this state might be entered incorrectly.
				// This should only be entered if the LLM provides content without tool calls.
				llmMessage := fsmCtx.llmResponse.Choices[0].Message
				if len(llmMessage.ToolCalls) == 0 {
					fsmCtx.finalContent = llmMessage.Content
				} else {
					// This case should ideally not happen if transitions are correct.
					// LLM requested tools, but we ended up in Done.
					zap.S().Error("FSM: Reached StateDone but last LLM response had tool calls.")
					fsmCtx.lastError = errors.New("FSM logic error: StateDone reached with pending tool calls")
					// No direct firing to StateError from OnEntry, rely on Process loop check
				}
			} else if fsmCtx.lastError == nil { // Only set if no other error caused entry to Done
				fsmCtx.lastError = errors.New("FSM: StateDone reached without a final LLM content response")
			}
			return nil
		})

	// State: Error
	// Action: This is a terminal state. The error is already in fsmCtx.lastError.
	fsm.Configure(StateError).
		OnEntry(func(ctx context.Context, args ...any) error {
			zap.S().Info("FSM: Entering StateError")
			if fsmCtx.lastError == nil {
				fsmCtx.lastError = errors.New("FSM: reached error state without a specific error")
			}
			return nil
		})

	// Start the FSM
	initialArgs := []any{ctx}                            // Pass context to OnEntry actions
	err := fsm.Fire(TriggerProcessInput, initialArgs...) // Initial trigger, though ReadyToCallLLM's OnEntry does the first LLM call.
	// Consider if TriggerProcessInput is needed or if OnEntry of initial state is enough.
	// For now, ReadyToCallLLM's OnEntry is self-starting based on current fsmCtx.messages.

	// The FSM transitions happen synchronously within Fire calls triggered by OnEntry actions.
	// We need to ensure the initial OnEntry for ReadyToCallLLM is invoked.
	// If the FSM is not started by an external Fire after NewStateMachine,
	// then the first state's OnEntry might need to be called manually or use a specific "start" trigger.
	// Let's assume the first state's OnEntry is called.
	// The stateless library might require an explicit `fsm.Permit(TriggerProcessInput, StateReadyToCallLLM)`
	// and then the first `fsm.Fire(TriggerProcessInput, ...)` would land it in ReadyToCallLLM,
	// and then its OnEntry would fire.

	// For this setup, let's ensure ReadyToCallLLM's OnEntry is triggered.
	// If FSM starts in StateReadyToCallLLM, its OnEntry should fire if configured for the state itself.
	// The library examples show `machine.Fire(trigger, params...)`.
	// The OnEntry actions are tied to state transitions.
	// A common pattern is: initial_state -> (fire trigger) -> state_with_on_entry_action.
	// So, we might need a pre-initial state or ensure the first Fire lands us in ReadyToCallLLM
	// and its OnEntry then makes the first LLM call.

	// Re-evaluating the start:
	// The FSM is created in StateReadyToCallLLM. Its OnEntry action initiates the first LLM call.
	// The transitions are synchronous. The FSM will run until it hits a terminal state (Done, Error)
	// or an OnEntry action doesn't fire a new trigger (which shouldn't happen with this config).

	// To start processing, the OnEntry of the initial state (StateReadyToCallLLM) must be triggered.
	// The `stateless` library typically triggers OnEntry when a state is entered.
	// For the initial state, this means it's usually called upon FSM creation if an OnEntry is defined for it,
	// or the first `Fire` call will transition to it (or itself) and trigger OnEntry.
	// My current configuration of StateReadyToCallLLM.OnEntry makes the first LLM call.
	// The line `err := fsm.Fire(TriggerProcessInput, initialArgs...)` was there before,
	// but `TriggerProcessInput` isn't explicitly handled by StateReadyToCallLLM to re-trigger OnEntry in a simple way.
	// The critical part is that the OnEntry of StateReadyToCallLLM *is* executed.
	// Let's assume `NewStateMachine` itself doesn't trigger OnEntry, so we need an initial fire.
	// We can make StateReadyToCallLLM permit re-entry on a generic start trigger.
	// However, the current design where OnEntry directly makes the call is simpler if it works as expected
	// for the initial state. If `NewStateMachine` doesn't trigger initial OnEntry,
	// then the fsmCtx would not have its first LLM call made.

	// The FSM transitions should drive it to a terminal state or max turns.
	// The fsm.Fire calls within OnEntry actions are key.

	// Activate the FSM to process the initial state's OnEntry action and subsequent transitions.
	// Pass the context to be available for actions triggered by Activate.
	activateErr := fsm.ActivateCtx(ctx)
	if activateErr != nil {
		// This error would be from an action called during activation, e.g., the first OnEntry.
		// Or if Activate itself has an issue.
		zap.S().Errorf("FSM activation failed: %v", activateErr)
		// If lastError was set by an action, it might be more specific.
		if fsmCtx.lastError != nil {
			return "", fsmCtx.lastError
		}
		return "", fmt.Errorf("FSM activation error: %w", activateErr)
	}

	// Check current state of FSM after all synchronous operations have completed.
	currentState, err := fsm.State(ctx) // Pass context and handle error
	if err != nil {
		// This would be an error with the FSM itself, not a business logic error
		zap.S().Errorf("FSM error when retrieving state: %v", err)
		return "", fmt.Errorf("FSM internal error: %w", err)
	}

	if currentState == StateDone {
		// If StateDone was reached due to an error that set fsmCtx.lastError (e.g. max turns leading to error trigger)
		if fsmCtx.lastError != nil && fsmCtx.finalContent == "" {
			return "", fsmCtx.lastError
		}
		return fsmCtx.finalContent, nil
	}
	if currentState == StateError {
		if fsmCtx.lastError != nil {
			return "", fsmCtx.lastError
		}
		return "", errors.New("FSM ended in StateError without a specific error")
	}
	// If FSM ended in a non-terminal state (e.g. max turns was hit and OnEntry fired to StateError, which should be caught above)
	if fsmCtx.lastError != nil { // This covers maxTurns error specifically if it didn't transition to StateError properly
		return "", fsmCtx.lastError
	}
	// Fallback for truly unexpected state
	return "", fmt.Errorf("FSM ended in an unexpected state: %v", currentState) // Use %v for interface types
}

// executeMCPTool is a helper to call an MCP tool and process its result.
func (a *Agent) executeMCPTool(ctx context.Context, toolName string, toolArgs map[string]any) string {
	var toolOutput string
	var mcpCallSuccessful bool

	for _, mcpClientInstance := range a.mcpClients {
		zap.S().Infow("Attempting CallTool via FSM helper", "toolName", toolName)
		callToolRequest := mcp.CallToolRequest{
			Params: mcp.CallToolParams{
				Name:      toolName,
				Arguments: toolArgs,
			},
		}
		mcpResult, callErr := mcpClientInstance.CallTool(ctx, callToolRequest)
		if callErr != nil {
			zap.S().Warnw("MCP CallTool failed for a client (FSM helper)", "tool", toolName, "error", callErr)
			continue
		}
		if mcpResult != nil {
			mcpCallSuccessful = true
			if mcpResult.IsError {
				zap.S().Warnf("MCP tool '%s' executed with IsError=true (FSM helper)", toolName)
				for _, contentItem := range mcpResult.Content {
					if textContent, ok := contentItem.(mcp.TextContent); ok {
						toolOutput = textContent.Text
						break
					}
				}
				if toolOutput == "" {
					toolOutput = "Tool execution resulted in an error without specific text."
				}
			} else {
				for _, contentItem := range mcpResult.Content {
					if textContent, ok := contentItem.(mcp.TextContent); ok {
						toolOutput = textContent.Text
						break
					}
				}
				if toolOutput == "" {
					resultBytes, merr := json.Marshal(mcpResult)
					if merr != nil {
						toolOutput = "Tool executed successfully, but result could not be formatted."
					} else {
						toolOutput = string(resultBytes)
					}
				}
			}
			break
		}
	}
	if !mcpCallSuccessful {
		toolOutput = "MCP tool call failed for all configured servers or tool not found (FSM helper)."
	}
	return toolOutput
}
