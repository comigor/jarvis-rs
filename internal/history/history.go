package history

import "sync"

var (
    mu       sync.Mutex
    messages []Message
)

// Save persists a message (in-memory). Replace with ObjectBox later.
func Save(msg Message) {
    mu.Lock()
    messages = append(messages, msg)
    mu.Unlock()
}
