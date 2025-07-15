.PHONY: build run test lint

# Build the Rust binary
build:
	cargo build --release
	mkdir -p bin
	cp target/release/jarvis bin/jarvis

# Run the application in development mode
run:
	cargo run

# Run all tests
test:
	cargo test

# Run linting and formatting checks
lint:
	cargo clippy -- -D warnings
	cargo fmt --check