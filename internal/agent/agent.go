package agent

import (
	"context"
	"encoding/json" // For CallToolRequest input and marshaling results
	"strings"       // For parsing LLM response

	"github.com/jarvis-g2o/internal/config"
	"github.com/jarvis-g2o/pkg/llm"
	"github.com/mark3labs/mcp-go/client"
	"github.com/mark3labs/mcp-go/mcp" // For request/response types
	// "github.com/mark3labs/mcp-go/client/transport" // For client options if needed later
	"github.com/sashabaranov/go-openai"
	"go.uber.org/zap"
)

// MCPClientInterface defines the methods our agent expects from an MCP client.
// This allows for mocking in tests and flexibility in client implementations.
// NOTE: This interface would ideally live in its own file, e.g., mcp_interface.go
type MCPClientInterface interface {
	Initialize(ctx context.Context, req mcp.InitializeRequest) (*mcp.InitializeResult, error)
	ListTools(ctx context.Context, req mcp.ListToolsRequest) (*mcp.ListToolsResult, error)
	CallTool(ctx context.Context, request mcp.CallToolRequest) (*mcp.CallToolResult, error)
	Close() error
}

// Agent is the main agent struct
type Agent struct {
	llmClient  llm.Client
	cfg        config.LLMConfig
	mcpServers []string             // For reference/logging
	mcpClients []MCPClientInterface // One client per configured MCP server
}

// New creates a new agent
func New(llmClient llm.Client, cfg config.Config) *Agent {
	initializedMcpClients := make([]MCPClientInterface, 0, len(cfg.MCPServers))
	backgroundCtx := context.Background() // Use a background context for initial client setup

	for _, serverURL := range cfg.MCPServers {
		var mcpC *client.Client // Use the concrete type from mcp-go for creation
		var err error

		// Determine client type based on URL scheme
		if strings.HasPrefix(serverURL, "ws://") || strings.HasPrefix(serverURL, "wss://") {
			mcpC, err = client.NewSSEMCPClient(serverURL) // Options can be added here if needed
		} else if strings.HasPrefix(serverURL, "http://") || strings.HasPrefix(serverURL, "https://") {
			mcpC, err = client.NewStreamableHttpClient(serverURL) // Options can be added here if needed
		} else {
			zap.S().Warnf("Unsupported MCP server URL scheme: %s. Skipping.", serverURL)
			continue
		}

		if err != nil {
			zap.S().Errorf("Failed to create MCP client for server %s: %v", serverURL, err)
			continue
		}

		// Initialize the client
		initReq := mcp.InitializeRequest{
			// Request: auto-filled by client? Or mcp.Request{ID: "some-id", Method: "initialize"}
			Params: mcp.InitializeParams{ // Corrected based on struct definition
				// ProtocolVersion: "0.1.0", // Or whatever is appropriate
				Capabilities: mcp.ClientCapabilities{
					// TODO: Populate with actual meaningful capabilities our agent supports
				},
				// ClientInfo: mcp.Implementation{ /* ... */}, // Optional?
			},
		}
		_, err = mcpC.Initialize(backgroundCtx, initReq)
		if err != nil {
			zap.S().Errorf("Failed to initialize MCP client for server %s: %v", serverURL, err)
			mcpC.Close() // Attempt to close if initialization failed
			continue
		}

		// TODO: Potentially call mcpC.Start(context.Background()) if the transport requires active starting.
		// This would need a separate context, perhaps managed by the agent's lifecycle.
		// For now, assuming Start is not strictly needed or handled by Initialize/CallTool.

		initializedMcpClients = append(initializedMcpClients, mcpC)
	}

	if len(initializedMcpClients) == 0 && len(cfg.MCPServers) > 0 {
		zap.S().Warnf("No MCP clients were successfully initialized despite %d servers configured.", len(cfg.MCPServers))
	}

	return &Agent{
		llmClient:  llmClient,
		cfg:        cfg.LLM,
		mcpServers: cfg.MCPServers, // Keep original list for reference if needed
		mcpClients: initializedMcpClients,
	}
}

// Process processes a request and returns a response
func (a *Agent) Process(ctx context.Context, request string) (string, error) {
	// Send the request to the LLM.
	// Tool usage is removed; MCP client logic will be added later.
	resp, err := a.llmClient.CreateChatCompletion(
		ctx,
		openai.ChatCompletionRequest{
			Model: a.cfg.Model,
			Messages: []openai.ChatCompletionMessage{
				{
					Role:    openai.ChatMessageRoleUser,
					Content: request,
				},
			},
			// Tools parameter removed
		},
	)

	zap.S().Infow("LLM response", "resp", resp)
	if err != nil {
		return "", err
	}

	// For now, return the LLM's direct response.
	// MCP interaction logic will be added in a subsequent step.
	if len(resp.Choices) > 0 && resp.Choices[0].Message.Content != "" {
		llmPrimaryResponseContent := resp.Choices[0].Message.Content // Changed variable name

		var jsonUnmarshalErr error // Explicitly declare error variable

		// Step 1: Parse LLM response for desired tool and arguments.
		// Expecting LLM to return a JSON string like: {"tool_name": "desired_tool", "arguments": {"key": "value"}}
		var llmToolCall struct {
			ToolName  string         `json:"tool_name"`
			Arguments map[string]any `json:"arguments"`
		}
		jsonUnmarshalErr = json.Unmarshal([]byte(llmPrimaryResponseContent), &llmToolCall)
		if jsonUnmarshalErr != nil {
			// Not a valid JSON for tool call, or LLM provided a direct answer.
			zap.S().Debugw("LLM response is not a structured tool call JSON, returning as direct answer.", "response", llmPrimaryResponseContent, "parse_error", jsonUnmarshalErr)
			return llmPrimaryResponseContent, nil
		}

		if llmToolCall.ToolName == "" {
			zap.S().Debugw("LLM response parsed as JSON but no tool_name specified, returning as direct answer.", "response", llmPrimaryResponseContent)
			return llmPrimaryResponseContent, nil // Or handle as an error if a tool call was expected
		}

		zap.S().Infow("LLM suggested MCP tool call", "tool", llmToolCall.ToolName, "args", llmToolCall.Arguments)

		if len(a.mcpClients) == 0 {
			return "LLM suggested an MCP tool, but no MCP clients are available.", nil
		}

		// Step 2: Iterate through MCP clients, list tools, and try to call the desired tool.
		for i, mcpClientInstance := range a.mcpClients {
			zap.S().Infow("Attempting with MCP client", "serverIndex", i, "serverURL", a.mcpServers[i])

			// List tools available on this server
			listToolsReq := mcp.ListToolsRequest{} // Empty request for now, assuming defaults
			serverTools, listErr := mcpClientInstance.ListTools(ctx, listToolsReq)
			if listErr != nil {
				zap.S().Warnw("Failed to list tools for MCP client", "serverIndex", i, "error", listErr)
				continue // Try next client
			}

			toolFoundOnServer := false
			for _, serverTool := range serverTools.Tools {
				if serverTool.Name == llmToolCall.ToolName {
					toolFoundOnServer = true
					break
				}
			}

			if !toolFoundOnServer {
				zap.S().Infow("Tool not found on this MCP server", "tool", llmToolCall.ToolName, "serverIndex", i)
				continue // Try next client
			}

			zap.S().Infow("Tool found on server, attempting call", "tool", llmToolCall.ToolName, "serverIndex", i)
			callToolRequest := mcp.CallToolRequest{
				Params: mcp.CallToolParams{ // Corrected structure
					Name:      llmToolCall.ToolName,
					Arguments: llmToolCall.Arguments, // map[string]any
				},
			}

			mcpResult, callErr := mcpClientInstance.CallTool(ctx, callToolRequest)
			if callErr != nil {
				zap.S().Warnw("MCP CallTool failed for client", "serverIndex", i, "tool", llmToolCall.ToolName, "error", callErr)
				continue // Try next client
			}

			if mcpResult != nil {
				if mcpResult.IsError {
					// Handle tool execution error reported by IsError field
					// For now, just grab the first text content if available, even for errors.
					// A more robust handler would inspect error details.
					for _, contentItem := range mcpResult.Content {
						if textContent, ok := contentItem.(mcp.TextContent); ok {
							return textContent.Text, nil
						}
						// TODO: Handle other content types if necessary, or specific error content.
					}
					return "MCP tool '" + llmToolCall.ToolName + "' executed with an error, but no text content in result.", nil
				}

				for _, contentItem := range mcpResult.Content {
					// Check for TextContent
					// Assuming mcp.TextContent is a concrete type or mcp.Content has a Text field/method
					// This part is still a bit speculative based on `Content []Content`
					// If Content is an interface, we'd need a type assertion.
					// If Content is a struct with optional fields (e.g. Text, Image), check those.
					// Let's assume for now mcp.Content might be a struct like { Text string; Image string; ... }
					// Or more likely, mcp.TextContent is a specific type that implements mcp.Content.
					if textContent, ok := contentItem.(mcp.TextContent); ok {
						return textContent.Text, nil
					}
					// A simpler direct check if mcp.Content has a Text field (less likely for interface array)
					// if contentItem.Text != "" { return contentItem.Text, nil }
				}

				// Fallback if no direct text content found
				resultBytes, marshalErr := json.Marshal(mcpResult)
				if marshalErr != nil {
					zap.S().Errorw("Failed to marshal MCP result", "error", marshalErr)
					return "MCP tool executed, but result could not be formatted.", nil
				}
				return string(resultBytes), nil
			}
			return "MCP tool '" + llmToolCall.ToolName + "' executed, but returned no content.", nil
		}

		return "LLM suggested tool '" + llmToolCall.ToolName + "', but it was not found on any available MCP server or the call failed.", nil
	}
	return "No response content from LLM.", nil
}

// getTools function removed.
