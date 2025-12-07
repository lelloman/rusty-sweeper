# Phase 6: Polish & Release - Implementation Plan

## Overview

Phase 6 focuses on production readiness: comprehensive documentation, shell completions, man pages, CI/CD pipeline, and release packaging.

**Prerequisites**: Phases 1-5 must be complete.

**Status Legend**:
- `[ ]` Not started
- `[~]` In progress
- `[x]` Completed

---

## Task 1: Fix Build Warnings

**Status**: `[x]`

### Description

Clean up existing compiler warnings in the codebase.

### Context

There are 2 warnings from Phase 2/3 code that should be fixed:
- `src/scanner/walker.rs:361` - unused `mut` on variable
- `src/scanner/size.rs:43` - unused `parse_size` function

### Implementation

1. Remove `mut` from `tree` variable in walker.rs
2. Either use `parse_size` or remove it (check if it was intended for future use)

### Acceptance Criteria

- [ ] `cargo build` produces no warnings
- [ ] `cargo clippy` produces no warnings

---

## Task 2: Add Shell Completions

**Status**: `[ ]`

### Description

Generate shell completion scripts for Bash, Zsh, and Fish using `clap_complete`.

### Context

Shell completions improve UX by allowing tab-completion of subcommands, options, and arguments.

### Implementation

**Add dependency to `Cargo.toml`:**

```toml
[build-dependencies]
clap_complete = "4"
```

**Create `build.rs`:**

```rust
use clap::CommandFactory;
use clap_complete::{generate_to, Shell};
use std::env;
use std::io::Error;

include!("src/cli.rs");

fn main() -> Result<(), Error> {
    let outdir = match env::var_os("OUT_DIR") {
        Some(outdir) => outdir,
        None => return Ok(()),
    };

    let mut cmd = Cli::command();

    generate_to(Shell::Bash, &mut cmd, "rusty-sweeper", &outdir)?;
    generate_to(Shell::Zsh, &mut cmd, "rusty-sweeper", &outdir)?;
    generate_to(Shell::Fish, &mut cmd, "rusty-sweeper", &outdir)?;

    Ok(())
}
```

**Alternative: Runtime generation subcommand:**

Add a `completions` subcommand that outputs completions to stdout:

```rust
#[derive(Subcommand)]
pub enum Command {
    // ... existing commands ...

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}
```

**Create `dist/completions/` directory with pre-generated files.**

### Acceptance Criteria

- [ ] Bash completions work (`complete -C rusty-sweeper rusty-sweeper`)
- [ ] Zsh completions work
- [ ] Fish completions work
- [ ] Completions files in `dist/completions/`

---

## Task 3: Generate Man Page

**Status**: `[ ]`

### Description

Generate a man page using `clap_mangen`.

### Context

Man pages are the standard Unix documentation format. Users expect `man rusty-sweeper` to work.

### Implementation

**Add dependency to `Cargo.toml`:**

```toml
[build-dependencies]
clap_mangen = "0.2"
```

**Update `build.rs`:**

```rust
use clap_mangen::Man;

fn main() -> Result<(), Error> {
    // ... completions code ...

    let man = Man::new(cmd.clone());
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;

    std::fs::write(
        std::path::Path::new(&outdir).join("rusty-sweeper.1"),
        buffer
    )?;

    Ok(())
}
```

**Create `dist/man/rusty-sweeper.1` with pre-generated man page.**

### Acceptance Criteria

- [ ] `man ./dist/man/rusty-sweeper.1` displays properly
- [ ] All subcommands documented
- [ ] Examples included

---

## Task 4: Write README

**Status**: `[ ]`

### Description

Create comprehensive README.md with installation instructions, usage examples, and screenshots.

### Context

The README is the first thing users see. It should clearly explain what the tool does and how to use it.

### Implementation

**File**: `README.md`

```markdown
# Rusty Sweeper

A fast, interactive disk usage analyzer and cleaner for Linux.

## Features

- **Disk Scanner**: Analyze disk usage with ncdu-like visualization
- **Project Cleaner**: Detect and clean build artifacts (Cargo, npm, etc.)
- **Interactive TUI**: Navigate and clean directly from the terminal
- **Monitor Service**: Background daemon with desktop notifications

## Installation

### From Source

```bash
cargo install --path .
```

### Arch Linux (AUR)

```bash
yay -S rusty-sweeper
```

### Pre-built Binaries

Download from [Releases](https://github.com/user/rusty-sweeper/releases).

## Usage

### Scan Directory

```bash
# Analyze current directory
rusty-sweeper scan

# Analyze specific path with depth limit
rusty-sweeper scan /home -d 5

# Output as JSON
rusty-sweeper scan --json /var
```

### Clean Build Artifacts

```bash
# Find cleanable projects
rusty-sweeper clean --size-only ~/projects

# Clean with confirmation
rusty-sweeper clean ~/projects

# Dry run (show what would be cleaned)
rusty-sweeper clean -n ~/projects

# Clean specific project types
rusty-sweeper clean --types cargo,npm ~/projects
```

### Interactive TUI

```bash
# Launch TUI at root
rusty-sweeper tui

# Start at specific directory
rusty-sweeper tui /home/user/projects
```

### Monitor Disk Usage

```bash
# Check once and exit
rusty-sweeper monitor --once

# Start as daemon
rusty-sweeper monitor --daemon

# Custom thresholds
rusty-sweeper monitor --warn 70 --critical 85

# Check daemon status
rusty-sweeper monitor --status

# Stop daemon
rusty-sweeper monitor --stop
```

## Configuration

Configuration file: `~/.config/rusty-sweeper/config.toml`

```toml
[scanner]
max_depth = 10
exclude = ["node_modules", ".git"]

[cleaner]
types = ["cargo", "npm", "go"]

[monitor]
interval = 300
warn_threshold = 80
critical_threshold = 90
```

## TUI Keybindings

| Key | Action |
|-----|--------|
| `j/↓` | Move down |
| `k/↑` | Move up |
| `Enter/l/→` | Expand/enter directory |
| `h/←/Backspace` | Collapse/go up |
| `d/Delete` | Delete selected |
| `c` | Clean project |
| `/` | Search |
| `.` | Toggle hidden files |
| `r` | Rescan |
| `?` | Help |
| `q/Esc` | Quit |

## License

MIT
```

### Acceptance Criteria

- [ ] Installation instructions for multiple methods
- [ ] Usage examples for all subcommands
- [ ] Configuration documented
- [ ] TUI keybindings table
- [ ] Screenshots/GIFs (optional but nice)

---

## Task 5: Create GitHub Actions CI

**Status**: `[ ]`

### Description

Set up CI/CD pipeline for automated testing and releases.

### Implementation

**File**: `.github/workflows/ci.yml`

```yaml
name: CI

on:
  push:
    branches: [main, master]
  pull_request:
    branches: [main, master]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Check formatting
        run: cargo fmt --check

      - name: Clippy
        run: cargo clippy -- -D warnings

      - name: Build
        run: cargo build --verbose

      - name: Run tests
        run: cargo test --verbose

  build:
    needs: test
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest]
        # Add more targets as needed:
        # os: [ubuntu-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Build release
        run: cargo build --release

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: rusty-sweeper-${{ matrix.os }}
          path: target/release/rusty-sweeper
```

**File**: `.github/workflows/release.yml`

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Build release
        run: cargo build --release

      - name: Create tarball
        run: |
          mkdir -p dist
          cp target/release/rusty-sweeper dist/
          cp README.md LICENSE dist/ 2>/dev/null || true
          cp -r dist/completions dist/pkg/ 2>/dev/null || true
          cp dist/man/*.1 dist/pkg/ 2>/dev/null || true
          tar -czvf rusty-sweeper-linux-x86_64.tar.gz -C dist .

      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: rusty-sweeper-linux-x86_64.tar.gz
          generate_release_notes: true
```

### Acceptance Criteria

- [ ] CI runs on every push/PR
- [ ] Tests must pass before merge
- [ ] Clippy warnings fail the build
- [ ] Release workflow creates GitHub releases
- [ ] Artifacts uploaded

---

## Task 6: Create AUR PKGBUILD

**Status**: `[ ]`

### Description

Create Arch Linux PKGBUILD for AUR submission.

### Implementation

**File**: `dist/PKGBUILD`

```bash
# Maintainer: Your Name <your.email@example.com>
pkgname=rusty-sweeper
pkgver=0.1.0
pkgrel=1
pkgdesc="A fast, interactive disk usage analyzer and cleaner"
arch=('x86_64')
url="https://github.com/user/rusty-sweeper"
license=('MIT')
depends=('gcc-libs')
makedepends=('rust' 'cargo')
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')

build() {
    cd "$pkgname-$pkgver"
    cargo build --release --locked
}

check() {
    cd "$pkgname-$pkgver"
    cargo test --release --locked
}

package() {
    cd "$pkgname-$pkgver"

    # Binary
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"

    # Man page
    install -Dm644 "dist/man/$pkgname.1" "$pkgdir/usr/share/man/man1/$pkgname.1"

    # Shell completions
    install -Dm644 "dist/completions/$pkgname.bash" \
        "$pkgdir/usr/share/bash-completion/completions/$pkgname"
    install -Dm644 "dist/completions/_$pkgname" \
        "$pkgdir/usr/share/zsh/site-functions/_$pkgname"
    install -Dm644 "dist/completions/$pkgname.fish" \
        "$pkgdir/usr/share/fish/vendor_completions.d/$pkgname.fish"

    # Systemd service
    install -Dm644 "dist/rusty-sweeper-monitor.service" \
        "$pkgdir/usr/lib/systemd/user/rusty-sweeper-monitor.service"

    # License
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
```

**File**: `dist/PKGBUILD-bin` (for pre-built binary)

```bash
# Maintainer: Your Name <your.email@example.com>
pkgname=rusty-sweeper-bin
pkgver=0.1.0
pkgrel=1
pkgdesc="A fast, interactive disk usage analyzer and cleaner (pre-built)"
arch=('x86_64')
url="https://github.com/user/rusty-sweeper"
license=('MIT')
depends=('gcc-libs')
provides=('rusty-sweeper')
conflicts=('rusty-sweeper')
source=("$url/releases/download/v$pkgver/rusty-sweeper-linux-x86_64.tar.gz")
sha256sums=('SKIP')

package() {
    install -Dm755 "rusty-sweeper" "$pkgdir/usr/bin/rusty-sweeper"
    # Add man page, completions, etc. from the tarball
}
```

### Acceptance Criteria

- [ ] PKGBUILD builds successfully with `makepkg`
- [ ] Package installs correctly
- [ ] Binary, man page, completions all installed
- [ ] `namcap` reports no errors

---

## Task 7: Add License File

**Status**: `[ ]`

### Description

Add MIT license file to the repository.

### Implementation

**File**: `LICENSE`

```
MIT License

Copyright (c) 2024 [Your Name]

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### Acceptance Criteria

- [ ] LICENSE file exists
- [ ] Cargo.toml has `license = "MIT"`

---

## Task 8: Improve Error Messages

**Status**: `[ ]`

### Description

Review and improve user-facing error messages throughout the codebase.

### Context

Good error messages should:
- Explain what went wrong
- Suggest how to fix it
- Include relevant context (file paths, values, etc.)

### Implementation

Review these areas:
1. Config file errors (invalid TOML, missing fields)
2. Permission denied errors
3. Path not found errors
4. Monitor threshold validation
5. Clean operation failures

Example improvements:

```rust
// Before
Err(SweeperError::PathNotFound(path))

// After
Err(SweeperError::PathNotFound {
    path: path.clone(),
    suggestion: if path.starts_with("~") {
        Some("Try using an absolute path instead of ~".to_string())
    } else {
        None
    },
})
```

### Acceptance Criteria

- [ ] All errors include actionable information
- [ ] No raw unwrap() panics in user-facing code
- [ ] Errors suggest fixes where possible

---

## Task 9: Add Integration Test Fixtures Script

**Status**: `[ ]`

### Description

Create a script to generate test fixtures for integration testing.

### Implementation

**File**: `scripts/create-test-fixtures.sh`

```bash
#!/bin/bash
# Create test fixture directories for integration testing

set -e

FIXTURE_DIR="${1:-/tmp/rusty-sweeper-fixtures}"

echo "Creating test fixtures in $FIXTURE_DIR"

rm -rf "$FIXTURE_DIR"
mkdir -p "$FIXTURE_DIR"

# Cargo project
mkdir -p "$FIXTURE_DIR/cargo-project/src"
echo 'fn main() { println!("Hello"); }' > "$FIXTURE_DIR/cargo-project/src/main.rs"
cat > "$FIXTURE_DIR/cargo-project/Cargo.toml" << 'EOF'
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
EOF
mkdir -p "$FIXTURE_DIR/cargo-project/target/debug"
dd if=/dev/zero of="$FIXTURE_DIR/cargo-project/target/debug/test" bs=1M count=10 2>/dev/null

# NPM project
mkdir -p "$FIXTURE_DIR/npm-project/node_modules/.bin"
echo '{"name": "test", "version": "1.0.0"}' > "$FIXTURE_DIR/npm-project/package.json"
dd if=/dev/zero of="$FIXTURE_DIR/npm-project/node_modules/.bin/fake" bs=1M count=5 2>/dev/null

# Go project
mkdir -p "$FIXTURE_DIR/go-project"
echo 'module example.com/test' > "$FIXTURE_DIR/go-project/go.mod"
echo 'package main; func main() {}' > "$FIXTURE_DIR/go-project/main.go"

# Large files for scanning
mkdir -p "$FIXTURE_DIR/large-files"
for i in {1..5}; do
    dd if=/dev/zero of="$FIXTURE_DIR/large-files/file$i.bin" bs=1M count=$((i * 2)) 2>/dev/null
done

# Deep directory structure
DEEP="$FIXTURE_DIR/deep"
for i in {1..10}; do
    DEEP="$DEEP/level$i"
done
mkdir -p "$DEEP"
echo "deep file" > "$DEEP/file.txt"

echo "Fixtures created:"
du -sh "$FIXTURE_DIR"/*
```

### Acceptance Criteria

- [ ] Script creates realistic test fixtures
- [ ] Includes Cargo, NPM, Go projects
- [ ] Includes various file sizes
- [ ] Integration tests can use these fixtures

---

## Task 10: Version Bump and Changelog

**Status**: `[ ]`

### Description

Prepare for release with version bump and changelog.

### Implementation

**Update `Cargo.toml`:**

```toml
[package]
name = "rusty-sweeper"
version = "0.1.0"  # Or appropriate version
```

**Create `CHANGELOG.md`:**

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2024-XX-XX

### Added
- Disk usage scanner with parallel traversal
- Project detection for Cargo, NPM, Go, Python, CMake, Gradle, Maven
- Interactive TUI with ncdu-like interface
- Clean command for removing build artifacts
- Background monitor daemon with desktop notifications
- Multiple notification backends (D-Bus, notify-send, i3-nagbar, stderr)
- Systemd user service for autostart
- Configuration file support

### Features
- `scan` - Analyze disk usage
- `clean` - Remove build artifacts
- `tui` - Interactive interface
- `monitor` - Background disk monitoring
```

### Acceptance Criteria

- [ ] Version in Cargo.toml is correct
- [ ] CHANGELOG.md documents all features
- [ ] Git tag created for release

---

## Summary

| Task | Description | Status |
|------|-------------|--------|
| 1 | Fix build warnings | `[ ]` |
| 2 | Add shell completions | `[ ]` |
| 3 | Generate man page | `[ ]` |
| 4 | Write README | `[ ]` |
| 5 | Create GitHub Actions CI | `[ ]` |
| 6 | Create AUR PKGBUILD | `[ ]` |
| 7 | Add license file | `[ ]` |
| 8 | Improve error messages | `[ ]` |
| 9 | Add test fixtures script | `[ ]` |
| 10 | Version bump and changelog | `[ ]` |

**Total: 10 tasks**

---

## Dependencies

Add to `Cargo.toml`:

```toml
[build-dependencies]
clap_complete = "4"
clap_mangen = "0.2"
```

---

## Definition of Done

Phase 6 is complete when:

1. `cargo build` produces no warnings
2. `cargo clippy` produces no warnings
3. Shell completions work for bash, zsh, fish
4. Man page is generated and correct
5. README has installation and usage docs
6. CI pipeline runs tests on every PR
7. Release workflow creates GitHub releases
8. AUR PKGBUILD builds successfully
9. All error messages are user-friendly
10. Version is tagged and changelog updated
