# Makefile for rust-sort project
# Run 'make check' before committing to ensure code quality

.PHONY: all build release check test fmt clippy clean help

# Default target
all: check build

# Help target
help:
	@echo "Available targets:"
	@echo "  make build    - Build debug version"
	@echo "  make release  - Build release version"
	@echo "  make check    - Run all checks (fmt, clippy, test)"
	@echo "  make test     - Run all tests"
	@echo "  make fmt      - Format code"
	@echo "  make clippy   - Run clippy linter"
	@echo "  make clean    - Clean build artifacts"
	@echo "  make bench    - Run benchmarks"
	@echo "  make docs     - Generate documentation"
	@echo "  make pre-commit - Run all checks before commit (alias for check)"

# Build debug version
build:
	cargo build

# Build release version
release:
	cargo build --release

# Run all checks - use this before committing
check: fmt-check clippy test
	@echo "✅ All checks passed! Ready to commit."

# Alias for check
pre-commit: check

# Format code
fmt:
	cargo fmt --all

# Check formatting without modifying files
fmt-check:
	@echo "Checking formatting..."
	cargo fmt --all -- --check

# Run clippy with all targets and features
clippy:
	@echo "Running clippy..."
	cargo clippy --all-targets --all-features -- -D warnings

# Run all tests
test:
	@echo "Running tests..."
	cargo test --all-features

# Run tests with output
test-verbose:
	cargo test --all-features -- --nocapture

# Run benchmarks
bench:
	cargo bench

# Generate documentation
docs:
	cargo doc --no-deps --open

# Clean build artifacts
clean:
	cargo clean
	rm -rf target/

# Quick test of basic functionality
quick-test: build
	@echo "Testing basic sort..."
	@echo -e "3\n1\n2" | ./target/debug/sort
	@echo ""
	@echo "Testing numeric sort..."
	@echo -e "10\n2\n1" | ./target/debug/sort -n
	@echo ""
	@echo "Testing reverse sort..."
	@echo -e "a\nb\nc" | ./target/debug/sort -r

# Performance test with large file
perf-test: release
	@echo "Generating test file..."
	@seq 1 1000000 | sort -R > test_large.txt
	@echo "Testing performance..."
	@time ./target/release/sort test_large.txt > /dev/null
	@rm -f test_large.txt

# Install git pre-commit hook
install-hooks:
	@echo "#!/bin/sh" > .git/hooks/pre-commit
	@echo "make check" >> .git/hooks/pre-commit
	@chmod +x .git/hooks/pre-commit
	@echo "Git pre-commit hook installed!"

# CI/CD simulation - runs all checks that would run in CI
ci: fmt-check clippy test
	@echo "Building release version..."
	@cargo build --release
	@echo "✅ CI checks passed!"