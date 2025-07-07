package history
// Package history provides SQLite-based persistence for chat messages.
// The database is opened lazily and created on first use.
// If opening the DB or executing queries fails, the package falls back to in-memory storage.


import (
    "database/sql"
    "os"
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
    dbPath := os.Getenv("HISTORY_DB_PATH")
    if dbPath == "" {
        dbPath = "history.db"
    }
    db, err = sql.Open("sqlite", "file:"+dbPath+"?_busy_timeout=10000&_fk=1")
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

// List returns all messages of a session in chronological order.
func List(sessionID string) []Message {
    dbOnce.Do(initDB)
    var out []Message
    if initErr == nil && db != nil {
        rows, err := db.Query(`SELECT id, session_id, role, content, created_at FROM messages WHERE session_id = ? ORDER BY id ASC;`, sessionID)
        if err == nil {
            defer rows.Close()
            for rows.Next() {
                var m Message
                if err := rows.Scan(&m.ID, &m.SessionID, &m.Role, &m.Content, &m.CreatedAt); err == nil {
                    out = append(out, m)
                }
            }
            return out
        }
    }
    mu.Lock()
    for _, m := range messages {
        if m.SessionID == sessionID {
            out = append(out, m)
        }
    }
    mu.Unlock()
    return out
}

