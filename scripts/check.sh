#!/bin/bash
# Comprehensive check script for rust-sort project
# Run this before committing to ensure code quality

set -e  # Exit on first error

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Running comprehensive checks...${NC}"
echo ""

# Check 1: Formatting
echo -e "${YELLOW}[1/4] Checking code formatting...${NC}"
if cargo fmt --all -- --check; then
    echo -e "${GREEN}✓ Formatting check passed${NC}"
else
    echo -e "${RED}✗ Formatting issues found. Run 'cargo fmt --all' to fix.${NC}"
    exit 1
fi
echo ""

# Check 2: Clippy
echo -e "${YELLOW}[2/4] Running clippy linter...${NC}"
if cargo clippy --all-targets --all-features -- -D warnings; then
    echo -e "${GREEN}✓ Clippy check passed${NC}"
else
    echo -e "${RED}✗ Clippy found issues${NC}"
    exit 1
fi
echo ""

# Check 3: Build
echo -e "${YELLOW}[3/4] Building project...${NC}"
if cargo build --release; then
    echo -e "${GREEN}✓ Build successful${NC}"
else
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi
echo ""

# Check 4: Tests
echo -e "${YELLOW}[4/4] Running tests...${NC}"
if cargo test --all-features; then
    echo -e "${GREEN}✓ All tests passed${NC}"
else
    echo -e "${RED}✗ Tests failed${NC}"
    exit 1
fi
echo ""

echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}✅ All checks passed! Ready to commit.${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"