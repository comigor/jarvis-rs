package agent

import (
	"context"

	"encoding/json"
	"strings"

	"github.com/jarvis-g2o/internal/config"
	"github.com/jarvis-g2o/pkg/llm"
	"github.com/mark3labs/mcp-go/client"
	"github.com/mark3labs/mcp-go/client/transport" // For header options
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
	llmClient  llm.Client
	cfg        config.LLMConfig
	mcpClients []MCPClientInterface
}

// New creates a new agent
func New(llmClient llm.Client, appCfg config.Config) *Agent {
	initializedMcpClients := make([]MCPClientInterface, 0, len(appCfg.MCPServers))
	backgroundCtx := context.Background()

	for _, serverCfg := range appCfg.MCPServers {
		var mcpC *client.Client
		var err error

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

		initReq := mcp.InitializeRequest{
			Params: mcp.InitializeParams{
				Capabilities: mcp.ClientCapabilities{}, // TODO: Populate capabilities
			},
		}
		_, err = mcpC.Initialize(backgroundCtx, initReq)
		if err != nil {
			zap.S().Errorf("Failed to initialize MCP client for server %s: %v", serverCfg.URL, err)
			mcpC.Close()
			continue
		}
		initializedMcpClients = append(initializedMcpClients, mcpC)
	}

	if len(initializedMcpClients) == 0 && len(appCfg.MCPServers) > 0 {
		zap.S().Warnf("No MCP clients were successfully initialized despite %d servers configured.", len(appCfg.MCPServers))
	}

	return &Agent{
		llmClient:  llmClient,
		cfg:        appCfg.LLM,
		mcpClients: initializedMcpClients,
	}
}

// Process processes a request and returns a response
func (a *Agent) Process(ctx context.Context, request string) (string, error) {
	resp, err := a.llmClient.CreateChatCompletion(
		ctx,
		openai.ChatCompletionRequest{
			Model: a.cfg.Model,
			Messages: []openai.ChatCompletionMessage{
				{Role: openai.ChatMessageRoleUser, Content: request},
			},
		},
	)

	zap.S().Infow("LLM response", "resp", resp)
	if err != nil {
		return "", err
	}

	if len(resp.Choices) > 0 && resp.Choices[0].Message.Content != "" {
		llmPrimaryResponseContent := resp.Choices[0].Message.Content
		var jsonUnmarshalErr error

		var llmToolCall struct {
			ToolName  string         `json:"tool_name"`
			Arguments map[string]any `json:"arguments"`
		}
		jsonUnmarshalErr = json.Unmarshal([]byte(llmPrimaryResponseContent), &llmToolCall)
		if jsonUnmarshalErr != nil {
			zap.S().Debugw("LLM response is not a structured tool call JSON, returning as direct answer.", "response", llmPrimaryResponseContent, "parse_error", jsonUnmarshalErr)
			return llmPrimaryResponseContent, nil
		}

		if llmToolCall.ToolName == "" {
			zap.S().Debugw("LLM response parsed as JSON but no tool_name specified, returning as direct answer.", "response", llmPrimaryResponseContent)
			return llmPrimaryResponseContent, nil
		}

		zap.S().Infow("LLM suggested MCP tool call", "tool", llmToolCall.ToolName, "args", llmToolCall.Arguments)

		if len(a.mcpClients) == 0 {
			return "LLM suggested an MCP tool, but no MCP clients are available.", nil
		}

		for _, mcpClientInstance := range a.mcpClients {
			// Note: serverURL for logging is not directly available here anymore unless we store MCPServerConfig along with clients
			zap.S().Infow("Attempting with an MCP client")

			listToolsReq := mcp.ListToolsRequest{}
			serverTools, listErr := mcpClientInstance.ListTools(ctx, listToolsReq)
			if listErr != nil {
				zap.S().Warnw("Failed to list tools for MCP client", "error", listErr)
				continue
			}

			toolFoundOnServer := false
			for _, serverTool := range serverTools.Tools {
				if serverTool.Name == llmToolCall.ToolName {
					toolFoundOnServer = true
					break
				}
			}

			if !toolFoundOnServer {
				zap.S().Infow("Tool not found on this MCP server", "tool", llmToolCall.ToolName)
				continue
			}

			zap.S().Infow("Tool found on server, attempting call", "tool", llmToolCall.ToolName)
			callToolRequest := mcp.CallToolRequest{
				Params: mcp.CallToolParams{
					Name:      llmToolCall.ToolName,
					Arguments: llmToolCall.Arguments,
				},
			}

			mcpResult, callErr := mcpClientInstance.CallTool(ctx, callToolRequest)
			if callErr != nil {
				zap.S().Warnw("MCP CallTool failed for client", "tool", llmToolCall.ToolName, "error", callErr)
				continue
			}

			if mcpResult != nil {
				if mcpResult.IsError {
					for _, contentItem := range mcpResult.Content {
						if textContent, ok := contentItem.(mcp.TextContent); ok {
							return textContent.Text, nil
						}
					}
					return "MCP tool '" + llmToolCall.ToolName + "' executed with an error, but no text content in result.", nil
				}
				for _, contentItem := range mcpResult.Content {
					if textContent, ok := contentItem.(mcp.TextContent); ok {
						return textContent.Text, nil
					}
				}
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
