package logger

import (
	"log/slog"
	"os"
	"strings"
)

var levelVar = new(slog.LevelVar)

var L = slog.New(slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{Level: levelVar}))

// SetLevel configures the global log level (debug, info, warn, error).
func SetLevel(lvl string) {
	switch strings.ToLower(lvl) {
	case "debug":
		levelVar.Set(slog.LevelDebug)
	case "warn":
		levelVar.Set(slog.LevelWarn)
	case "error":
		levelVar.Set(slog.LevelError)
	default:
		levelVar.Set(slog.LevelInfo)
	}
}
