
package tools

// Tool is the interface for all tools
type Tool interface {
	Name() string
	Description() string
	Run(args string) (string, error)
}
