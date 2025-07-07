package history
// Package history provides SQLite-based persistence for chat messages.
// The database is opened lazily and created on first use.
// If opening the DB or executing queries fails, the package falls back to in-memory storage.


import (
    "database/sql"
    "sync"

    _ "github.com/glebarez/go-sqlite"

    "github.com/comigor/jarvis-go/internal/logger"
)

var (
    mu       sync.Mutex
    messages []Message // in-memory fallback

    dbOnce sync.Once
    db     *sql.DB
    initErr error
)

// initDB lazily opens the SQLite database and creates the messages table if it doesn't exist.
func initDB() {
    var err error
    db, err = sql.Open("sqlite", "file:history.db?_busy_timeout=10000&_fk=1")
    if err != nil {
        initErr = err
        logger.L.Warn("sqlite open failed; using in-memory history", "error", err)
        return
    }
    if _, err = db.Exec(`CREATE TABLE IF NOT EXISTS messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT,
        role TEXT,
        content TEXT,
        created_at DATETIME
    );`); err != nil {
        initErr = err
        logger.L.Warn("sqlite table creation failed; using in-memory history", "error", err)
        return
    }
    logger.L.Info("sqlite history DB initialized")
}

// Save persists a message to the SQLite database when available and always keeps
// an in-memory copy as fallback.
func Save(msg Message) {
    dbOnce.Do(initDB)

    if initErr == nil && db != nil {
        _, err := db.Exec(`INSERT INTO messages (session_id, role, content, created_at) VALUES (?,?,?,?);`, msg.SessionID, msg.Role, msg.Content, msg.CreatedAt)
        if err != nil {
            logger.L.Error("failed to store message in sqlite; falling back to memory", "error", err)
        }
    }

    mu.Lock()
    messages = append(messages, msg)
    mu.Unlock()
}
