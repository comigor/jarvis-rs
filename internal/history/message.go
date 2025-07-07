package history

import "time"

//go:generate go run github.com/objectbox/objectbox-go/cmd/objectbox-gogen

// Message represents a single conversational message persisted in ObjectBox.
type Message struct {
    Id        uint64    `objectbox:"id" json:"id"`
    SessionID string    `json:"session_id"`
    Role      string    `json:"role"`
    Content   string    `json:"content"`
    CreatedAt time.Time `json:"created_at"`
}
