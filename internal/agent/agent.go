package agent

import (
	"context"
	"encoding/json" // Will be needed for MCP tool call argument parsing
	"strings"       // Will be needed for URL scheme parsing

	"github.com/jarvis-g2o/internal/config"
	"github.com/jarvis-g2o/pkg/llm"

	// MCP specific imports
	"github.com/mark3labs/mcp-go/client"
	"github.com/mark3labs/mcp-go/client/transport"
	"github.com/mark3labs/mcp-go/mcp"

	"github.com/sashabaranov/go-openai"
	"go.uber.org/zap"
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

		// Create client with headers
		if strings.HasPrefix(serverCfg.URL, "ws://") || strings.HasPrefix(serverCfg.URL, "wss://") {
			var sseOpts []transport.ClientOption
			if len(serverCfg.Headers) > 0 {
				sseOpts = append(sseOpts, transport.WithHeaders(serverCfg.Headers))
			}
			mcpC, err = client.NewSSEMCPClient(serverCfg.URL, sseOpts...)
		} else if strings.HasPrefix(serverCfg.URL, "http://") || strings.HasPrefix(serverCfg.URL, "https://") {
			var httpOpts []transport.StreamableHTTPCOption
			if len(serverCfg.Headers) > 0 {
				httpOpts = append(httpOpts, transport.WithHTTPHeaders(serverCfg.Headers))
			}
			mcpC, err = client.NewStreamableHttpClient(serverCfg.URL, httpOpts...)
		} else {
			zap.S().Warnf("Unsupported MCP server URL scheme: %s. Skipping.", serverCfg.URL)
			continue
		}

		if err != nil {
			zap.S().Errorf("Failed to create MCP client for server %s: %v", serverCfg.URL, err)
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
					// OpenAI tools require a parameters schema. If nil, provide an empty object schema.
					var paramsSchema json.RawMessage
					if mcpTool.RawInputSchema != nil && len(mcpTool.RawInputSchema) > 0 && string(mcpTool.RawInputSchema) != "null" {
						paramsSchema = mcpTool.RawInputSchema
					} else {
						// Attempt to use InputSchema. Since its exact type is tricky for a nil check that satisfies vet,
						// we'll try marshalling it. If it's a zero-struct, Marshal should produce "{}" or similar.
						// If it's truly meant to be empty or not provided, this path will be taken.
						schemaBytes, marshalErr := json.Marshal(mcpTool.InputSchema)
						if marshalErr != nil {
							zap.S().Errorf("Failed to marshal InputSchema for tool '%s': %v. Using empty schema.", mcpTool.Name, marshalErr)
							paramsSchema = json.RawMessage(`{"type": "object", "properties": {}}`)
						} else {
							// Check if the marshalled schema is more than just an empty object "{}" or null "null"
							// as some struct types might marshal to "{}" if all fields are zero/empty.
							// For a schema, "{}" is often a valid "any object" schema.
							// OpenAI might be fine with `{"type": "object", "properties": {}}` which is effectively what an empty schema means.
							// So, if mcpTool.InputSchema marshals without error, we use it, even if it's "{}".
							// The critical part is avoiding using it if it's truly absent in a way that Marshal fails or indicates emptiness beyond "{}".
							// The original mcp.Tool might have InputSchema as a non-nil struct even if empty.
							paramsSchema = json.RawMessage(schemaBytes)
							if string(paramsSchema) == "{}" || string(paramsSchema) == "null" {
								// If it marshals to an empty object or null, and RawInputSchema was also nil/empty,
								// then we can definitively say there's no meaningful schema.
								if mcpTool.RawInputSchema == nil || len(mcpTool.RawInputSchema) == 0 || string(mcpTool.RawInputSchema) == "null" {
									zap.S().Warnf("Tool '%s' from MCP server %s has an empty or null schema (InputSchema: %s). Using default empty object schema for LLM.", mcpTool.Name, serverCfg.URL, string(paramsSchema))
									paramsSchema = json.RawMessage(`{"type": "object", "properties": {}}`)
								}
							}
						}
					}
					// Final check: if after all this paramsSchema is still nil (e.g. RawInputSchema was nil, InputSchema was nil or failed marshal)
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
// This function now implements the full LLM tool calling flow.
func (a *Agent) Process(ctx context.Context, request string) (string, error) {
	messages := []openai.ChatCompletionMessage{
		{Role: openai.ChatMessageRoleUser, Content: request},
	}

	// Initial LLM call, now with tools
	resp, err := a.llmClient.CreateChatCompletion(
		ctx,
		openai.ChatCompletionRequest{
			Model:    a.cfg.Model,
			Messages: messages,
			Tools:    a.availableLLMTools,
		},
	)

	zap.S().Infow("LLM initial response", "resp", resp)
	if err != nil {
		return "", err
	}

	// Check if the LLM wants to call a tool
	if len(resp.Choices) > 0 && len(resp.Choices[0].Message.ToolCalls) > 0 {
		// For now, process the first tool call if multiple are returned
		toolCall := resp.Choices[0].Message.ToolCalls[0]
		toolName := toolCall.Function.Name
		argumentsJSON := toolCall.Function.Arguments

		zap.S().Infow("LLM requested tool call", "toolName", toolName, "arguments", argumentsJSON)

		var toolArgs map[string]any
		if err := json.Unmarshal([]byte(argumentsJSON), &toolArgs); err != nil {
			zap.S().Errorf("Failed to unmarshal tool arguments JSON: %v", err)
			// Potentially send this error back to the LLM or return an error message
			return "Error parsing tool arguments from LLM.", err
		}

		if len(a.mcpClients) == 0 {
			return "LLM requested a tool, but no MCP clients are available.", nil
		}

		var mcpCallSuccessful bool
		var toolOutput string

		// Iterate through MCP clients and attempt the tool call
		// The first successful call will be used.
		// Note: This doesn't explicitly check ListTools first here, assumes server will reject if tool unknown.
		// A more robust approach might involve checking ListTools or having a tool-to-client routing map.
		for _, mcpClientInstance := range a.mcpClients {
			zap.S().Infow("Attempting CallTool with an MCP client", "toolName", toolName)
			callToolRequest := mcp.CallToolRequest{
				Params: mcp.CallToolParams{
					Name:      toolName,
					Arguments: toolArgs,
				},
			}
			mcpResult, callErr := mcpClientInstance.CallTool(ctx, callToolRequest)
			if callErr != nil {
				zap.S().Warnw("MCP CallTool failed for a client", "tool", toolName, "error", callErr)
				continue // Try next client
			}

			// Process result
			if mcpResult != nil {
				mcpCallSuccessful = true
				if mcpResult.IsError {
					zap.S().Warnf("MCP tool '%s' executed with IsError=true", toolName)
					// Try to extract text from error content
					for _, contentItem := range mcpResult.Content {
						if textContent, ok := contentItem.(mcp.TextContent); ok {
							toolOutput = textContent.Text
							break
						}
					}
					if toolOutput == "" { // If no text content in error
						toolOutput = "Tool execution resulted in an error without specific text."
					}
				} else { // Success
					for _, contentItem := range mcpResult.Content {
						if textContent, ok := contentItem.(mcp.TextContent); ok {
							toolOutput = textContent.Text
							break // Take first text content
						}
					}
					if toolOutput == "" { // If no text content even on success
						resultBytes, marshalErr := json.Marshal(mcpResult)
						if marshalErr != nil {
							zap.S().Errorw("Failed to marshal successful MCP result with no text content", "error", marshalErr)
							toolOutput = "Tool executed successfully, but result could not be formatted."
						} else {
							toolOutput = string(resultBytes)
						}
					}
				}
				break // Break from client loop on first successful (or erroring but processed) call
			}
		}

		if !mcpCallSuccessful {
			toolOutput = "MCP tool call failed for all configured servers or tool not found."
			// It might be better to return an error here or a more structured response to LLM
		}

		zap.S().Infow("Sending tool output back to LLM", "toolName", toolName, "output", toolOutput)
		// Send the tool output back to the LLM
		messages = append(messages, resp.Choices[0].Message) // Add previous assistant message with ToolCall
		messages = append(messages, openai.ChatCompletionMessage{
			Role:       openai.ChatMessageRoleTool,
			Content:    toolOutput,
			ToolCallID: toolCall.ID,
		})

		finalResp, err := a.llmClient.CreateChatCompletion(ctx, openai.ChatCompletionRequest{
			Model:    a.cfg.Model,
			Messages: messages,
			Tools:    a.availableLLMTools, // Also provide tools here, LLM might chain calls
		})
		if err != nil {
			zap.S().Errorf("Error in final LLM call after tool execution: %v", err)
			return "", err
		}
		if len(finalResp.Choices) > 0 {
			return finalResp.Choices[0].Message.Content, nil
		}
		return "LLM processed tool output but returned no content.", nil

	} else if len(resp.Choices) > 0 && resp.Choices[0].Message.Content != "" {
		// LLM responded directly without calling a tool
		return resp.Choices[0].Message.Content, nil
	}

	return "No response content from LLM.", nil
}
