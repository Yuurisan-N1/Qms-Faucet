BINARY := qms-faucet-bot
RELEASE_DIR := target/release
DEBUG_DIR := target/debug

.PHONY: all build run release clean check fmt cuda package selftest

# Default
all: build

# Debug build
build:
	cargo build

# Run in debug mode
run:
	cargo run

# Optimized release build
release:
	cargo build --release

# Run release binary directly (faster startup)
start: release
	./$(RELEASE_DIR)/$(BINARY)

# Check errors without producing binary
check:
	cargo check

# Format code
fmt:
	cargo fmt

# Clean build artifacts
clean:
	cargo clean

# Build the optional gpu accelerator (needs the cuda toolkit)
cuda:
	./scripts/build_cuda.sh

# Run the built in checks
selftest: release
	./$(RELEASE_DIR)/$(BINARY) selftest

# Build a source archive for handoff
package:
	./scripts/package.sh

# Show binary size after release build
size: release
	@ls -lh $(RELEASE_DIR)/$(BINARY) | awk '{print "Binary size:", $$5}'