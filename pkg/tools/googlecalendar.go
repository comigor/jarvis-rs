package tools

import "fmt"

// GoogleCalendarTool is a tool for interacting with Google Calendar
type GoogleCalendarTool struct{}

// Name returns the name of the tool
func (t *GoogleCalendarTool) Name() string { return "google_calendar" }

// Description returns the description of the tool
func (t *GoogleCalendarTool) Description() string {
	return "Interacts with Google Calendar to manage events."
}

// Params schema (simple command string)
func (t *GoogleCalendarTool) Params() any {
	type args struct {
		Command string `json:"command"`
	}
	return &args{}
}

// Run runs the tool
func (t *GoogleCalendarTool) Run(args string) (string, error) {
	return fmt.Sprintf("Google Calendar tool called with args: %s", args), nil
}
