.PHONY: build run test lint clean check fmt

# Build the Rust binary
build:
	cargo build --release

# Run the application in development mode
run:
	cargo run

# Run all tests
test:
	cargo test

# Run linting and formatting
lint:
	cargo clippy -- -D warnings
	cargo fmt --check

# Format code
fmt:
	cargo fmt

# Check compilation without building
check:
	cargo check

# Clean build artifacts
clean:
	cargo clean

# Development build (debug mode)
build-dev:
	cargo build

# Run with specific log level
run-debug:
	RUST_LOG=debug cargo run

# Run tests with output
test-verbose:
	cargo test -- --nocapture

# Install development dependencies
install-deps:
	rustup component add clippy rustfmt