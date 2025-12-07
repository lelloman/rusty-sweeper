#!/bin/bash
# Create test fixture directories for integration testing
#
# Usage: ./scripts/create-test-fixtures.sh [OUTPUT_DIR]
#
# Creates a directory structure with:
# - Cargo project with target/ artifacts
# - NPM project with node_modules/
# - Go project
# - Python project with __pycache__/
# - Large files for scanning tests
# - Deep directory structure

set -e

FIXTURE_DIR="${1:-/tmp/rusty-sweeper-fixtures}"

echo "Creating test fixtures in $FIXTURE_DIR"
echo "========================================="

# Clean up any existing fixtures
rm -rf "$FIXTURE_DIR"
mkdir -p "$FIXTURE_DIR"

# Cargo project
echo "Creating Cargo project..."
mkdir -p "$FIXTURE_DIR/cargo-project/src"
cat > "$FIXTURE_DIR/cargo-project/src/main.rs" << 'EOF'
fn main() {
    println!("Hello, world!");
}
EOF
cat > "$FIXTURE_DIR/cargo-project/Cargo.toml" << 'EOF'
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
EOF
# Create fake target directory with some binaries
mkdir -p "$FIXTURE_DIR/cargo-project/target/debug/deps"
mkdir -p "$FIXTURE_DIR/cargo-project/target/release"
dd if=/dev/zero of="$FIXTURE_DIR/cargo-project/target/debug/test-project" bs=1M count=5 2>/dev/null
dd if=/dev/zero of="$FIXTURE_DIR/cargo-project/target/release/test-project" bs=1M count=2 2>/dev/null
dd if=/dev/zero of="$FIXTURE_DIR/cargo-project/target/debug/deps/libtest.rlib" bs=1M count=3 2>/dev/null

# NPM project
echo "Creating NPM project..."
mkdir -p "$FIXTURE_DIR/npm-project/node_modules/.bin"
mkdir -p "$FIXTURE_DIR/npm-project/node_modules/lodash"
mkdir -p "$FIXTURE_DIR/npm-project/node_modules/react"
cat > "$FIXTURE_DIR/npm-project/package.json" << 'EOF'
{
  "name": "test-npm-project",
  "version": "1.0.0",
  "dependencies": {
    "lodash": "^4.17.21",
    "react": "^18.2.0"
  }
}
EOF
dd if=/dev/zero of="$FIXTURE_DIR/npm-project/node_modules/lodash/lodash.js" bs=1M count=2 2>/dev/null
dd if=/dev/zero of="$FIXTURE_DIR/npm-project/node_modules/react/index.js" bs=1M count=1 2>/dev/null

# Go project
echo "Creating Go project..."
mkdir -p "$FIXTURE_DIR/go-project"
cat > "$FIXTURE_DIR/go-project/go.mod" << 'EOF'
module example.com/test

go 1.21
EOF
cat > "$FIXTURE_DIR/go-project/main.go" << 'EOF'
package main

func main() {
    println("Hello from Go!")
}
EOF

# Python project
echo "Creating Python project..."
mkdir -p "$FIXTURE_DIR/python-project/mypackage/__pycache__"
mkdir -p "$FIXTURE_DIR/python-project/.eggs"
cat > "$FIXTURE_DIR/python-project/setup.py" << 'EOF'
from setuptools import setup

setup(
    name="test-package",
    version="0.1.0",
)
EOF
cat > "$FIXTURE_DIR/python-project/mypackage/__init__.py" << 'EOF'
"""Test package"""
EOF
dd if=/dev/zero of="$FIXTURE_DIR/python-project/mypackage/__pycache__/module.cpython-311.pyc" bs=100K count=5 2>/dev/null
dd if=/dev/zero of="$FIXTURE_DIR/python-project/.eggs/test_egg.egg" bs=500K count=2 2>/dev/null

# Large files for scanning
echo "Creating large files..."
mkdir -p "$FIXTURE_DIR/large-files"
for i in 1 2 3 4 5; do
    size=$((i * 2))
    dd if=/dev/zero of="$FIXTURE_DIR/large-files/file${i}.bin" bs=1M count=$size 2>/dev/null
done

# Deep directory structure
echo "Creating deep directory structure..."
DEEP="$FIXTURE_DIR/deep"
for i in $(seq 1 10); do
    DEEP="$DEEP/level$i"
done
mkdir -p "$DEEP"
echo "deep file content" > "$DEEP/file.txt"

# Hidden files
echo "Creating hidden files..."
mkdir -p "$FIXTURE_DIR/hidden-test"
echo "visible" > "$FIXTURE_DIR/hidden-test/visible.txt"
echo "hidden" > "$FIXTURE_DIR/hidden-test/.hidden.txt"
mkdir -p "$FIXTURE_DIR/hidden-test/.hidden-dir"
echo "in hidden dir" > "$FIXTURE_DIR/hidden-test/.hidden-dir/file.txt"

# Empty directories
echo "Creating empty directories..."
mkdir -p "$FIXTURE_DIR/empty-dirs/empty1"
mkdir -p "$FIXTURE_DIR/empty-dirs/empty2"
mkdir -p "$FIXTURE_DIR/empty-dirs/not-empty"
echo "content" > "$FIXTURE_DIR/empty-dirs/not-empty/file.txt"

echo ""
echo "Fixtures created successfully!"
echo ""
echo "Summary:"
du -sh "$FIXTURE_DIR"/*
echo ""
echo "Total:"
du -sh "$FIXTURE_DIR"
