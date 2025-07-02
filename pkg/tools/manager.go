
package tools

import "fmt"

// ToolManager manages the available tools
type ToolManager struct {
	tools map[string]Tool
}

// NewToolManager creates a new ToolManager
func NewToolManager() *ToolManager {
	return &ToolManager{
		tools: make(map[string]Tool),
	}
}

// RegisterTool registers a new tool
func (m *ToolManager) RegisterTool(tool Tool) {
	m.tools[tool.Name()] = tool
}

// GetTool retrieves a tool by name
func (m *ToolManager) GetTool(name string) (Tool, error) {
	tool, ok := m.tools[name]
	if !ok {
		return nil, fmt.Errorf("tool not found: %s", name)
	}
	return tool, nil
}
