package agent

import (
	"context"
	"strings"

	"github.com/jarvis-g2o/internal/config"
	"github.com/jarvis-g2o/pkg/llm"
	// TODO: Add import for mcpclient "github.com/mark3labs/mcp-go"
	"github.com/sashabaranov/go-openai"
	"go.uber.org/zap"
)

// Agent is the main agent struct
type Agent struct {
	llmClient  llm.Client
	cfg        config.LLMConfig
	mcpServers []string
	// mcpClient  *mcpclient.Client // TODO: Uncomment when mcpclient is integrated
}

// New creates a new agent
func New(llmClient llm.Client, cfg config.Config) *Agent {
	// TODO: Initialize mcpClient
	// mcpClient := mcpclient.NewClient() or similar
	return &Agent{
		llmClient:  llmClient,
		cfg:        cfg.LLM,
		mcpServers: cfg.MCPServers,
		// mcpClient: mcpClient, // TODO: Assign initialized mcpClient
	}
}

// Process processes a request and returns a response
func (a *Agent) Process(ctx context.Context, request string) (string, error) {
	// First, we send the request to the LLM to understand the user's intent.
	// The LLM's role might shift to helping decide which MCP service to call,
	// or to format the request for an MCP service.
	// For now, we'll keep a simple LLM call. The `Tools` parameter is removed.
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
			// Tools: a.getTools(), // Removed
		},
	)

	zap.S().Infow("LLM response", "resp", resp)
	if err != nil {
		return "", err
	}

	// TODO: Implement MCP interaction logic here.
	// This is a conceptual outline and will depend on the mcp-go library's API.
	// 1. Analyze LLM response: Does it suggest an action or service?
	//    Or is the LLM's response the final answer?
	// 2. Identify target MCP server(s) from `a.mcpServers`.
	//    This might involve a discovery mechanism or simple routing.
	// 3. If an MCP action is needed:
	//    a. Connect to the chosen MCP server(s) using `a.mcpClient`.
	//       (e.g., `connection, err := a.mcpClient.Connect(serverAddress)`)
	//    b. Prepare the request for the MCP service. This might involve transforming
	//       the LLM output or user request into an MCP-compatible format.
	//       (e.g., `mcpRequest := mcpclient.NewRequest("serviceName", "methodName", params)`)
	//    c. Send the request via the MCP client.
	//       (e.g., `mcpResponse, err := connection.Send(mcpRequest)`)
	//    d. Handle the MCP response. This might involve sending it back to the LLM
	//       for summarization or directly to the user.
	//
	// For now, we will assume the LLM's first response is the final response
	// as the MCP interaction details are not yet defined.
	if len(resp.Choices) > 0 && resp.Choices[0].Message.Content != "" {
		llmResponseContent := resp.Choices[0].Message.Content
		zap.S().Infow("LLM provided content directly", "content", llmResponseContent)

		// Placeholder: If LLM mentions a "service" and "action", log it and try to "call" it.
		// This is highly speculative and needs to be replaced with actual MCP logic.
		if strings.Contains(llmResponseContent, "service:") && strings.Contains(llmResponseContent, "action:") {
			zap.S().Infow("LLM response suggests a potential MCP call. This part needs real implementation.", "response", llmResponseContent)
			// Example of how one might interact with MCP servers:
			// for _, serverAddr := range a.mcpServers {
			//    zap.S().Infow("Attempting to interact with MCP server (conceptual)", "server", serverAddr)
			//    // Hypothetical mcpClient usage:
			//    // err := a.mcpClient.Connect(serverAddr)
			//    // if err != nil { continue }
			//    // mcpResult, err := a.mcpClient.Call(ctx, "some_service", "some_action", llmResponseContent)
			//    // if err == nil { return mcpResult, nil }
			// }
			return "MCP interaction placeholder: LLM suggested an action. Actual call logic needs to be implemented using the mcp-go library and details from " + strings.Join(a.mcpServers, ", "), nil
		}
		return llmResponseContent, nil
	}

	// Fallback if no direct content from LLM.
	return "No response from LLM.", nil
}

// getTools function is no longer needed and has been removed.
