package history
// Package history provides persistence for chat messages using ObjectBox.
// Initialize() must be called once at startup to open the ObjectBox store.
// The package falls back to in-memory when ObjectBox init fails (e.g., during
// `go test` without generated files).


import (
    "sync"

    "github.com/objectbox/objectbox-go/objectbox"

    "github.com/comigor/jarvis-go/internal/logger"
)

var (
    mu       sync.Mutex
    messages []Message // holds messages if ObjectBox init fails or for quick access

    obxOnce  sync.Once
    obxStore *objectbox.ObjectBox
    msgBox   *MessageBox
    initErr  error
)

// initStore lazily opens the ObjectBox store. If initialization fails, the
// error is stored and in-memory fallback will be used.
func initStore() {
    obxStore, initErr = objectbox.NewBuilder().Model(ObjectBoxModel()).Build()
    if initErr != nil {
        logger.L.Warn("ObjectBox store initialization failed; using in-memory history", "error", initErr)
        return
    }
    msgBox = BoxForMessage(obxStore)
    logger.L.Info("ObjectBox store initialized for history persistence")
}

// Save persists a message to the ObjectBox database (when available) and always
// keeps an in-memory copy as fallback.
func Save(msg Message) {
    obxOnce.Do(initStore)

    if initErr == nil && msgBox != nil {
        if _, err := msgBox.Put(&msg); err != nil {
            logger.L.Error("failed to store message in ObjectBox; falling back to memory", "error", err)
        }
    }

    mu.Lock()
    messages = append(messages, msg)
    mu.Unlock()
}
