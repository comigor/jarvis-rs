
.PHONY: build run

build:
	go build -o bin/jarvis ./cmd/jarvis

run:
	go run ./cmd/jarvis

test:
	go test ./...

lint:
	go vet ./...
	gofmt -w .
