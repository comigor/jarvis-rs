package history

import "time"

// Message represents a single conversational message.
// NOTE: Persistence to ObjectBox will be added in a follow-up change.
type Message struct {
    SessionID string    `json:"session_id"`
    Role      string    `json:"role"`
    Content   string    `json:"content"`
    CreatedAt time.Time `json:"created_at"`
}
