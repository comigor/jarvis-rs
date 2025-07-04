# ---- Build stage ----
FROM golang:1.24 AS builder
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -o jarvis ./cmd/jarvis

# ---- Runtime stage ----
FROM gcr.io/distroless/static
COPY --from=builder /app/jarvis /usr/local/bin/jarvis
ENTRYPOINT ["/usr/local/bin/jarvis"]
