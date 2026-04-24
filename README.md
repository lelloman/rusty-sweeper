# Rusty Sweeper

A fast, interactive disk usage analyzer and cleaner for Linux.

## Current Status

Rusty Sweeper is implemented and test-covered across its main command surface:

- `scan` is implemented and supports tree output, JSON output, hidden files, depth limiting, and size/name sorting.
- `clean` is implemented for local build artifact cleanup and Docker system resources.
- `rusty-sweeper` launches the TUI by default, including navigation, search, delete, clean, and background scanning.
- `rusty-sweeper-monitor` is a dedicated monitor binary with one-shot checks, daemon mode, PID/log management, and multiple notification backends.

Current limitations:

- `scan --sort mtime` is accepted by the CLI, but currently falls back to size sorting.
- The configuration file is loaded and validated, but most command behavior is still driven directly by CLI flags rather than config values.
- Go and Bazel detectors exist in the codebase, but command-only cleanup for those project types is not currently surfaced by project scanning.

## Features

- **Disk Scanner**: Analyze disk usage with parallel traversal
- **Project Cleaner**: Detect and clean build artifacts (Cargo, npm, Python, CMake, Gradle, Maven, .NET) plus Docker system resources
- **Interactive TUI**: Navigate and clean directly from the terminal
- **Monitor Service**: Background daemon with desktop notifications when disk is full

## Installation

### From Source

```bash
git clone https://github.com/user/rusty-sweeper
cd rusty-sweeper
cargo install --path .
```

### Shell Completions

```bash
# Bash
rusty-sweeper completions bash > ~/.local/share/bash-completion/completions/rusty-sweeper

# Zsh
rusty-sweeper completions zsh > ~/.local/share/zsh/site-functions/_rusty-sweeper

# Fish
rusty-sweeper completions fish > ~/.config/fish/completions/rusty-sweeper.fish
```

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
# Find cleanable projects and show sizes
rusty-sweeper clean --size-only ~/projects

# Clean with confirmation
rusty-sweeper clean ~/projects

# Dry run (show what would be cleaned)
rusty-sweeper clean -n ~/projects

# Clean specific project types
rusty-sweeper clean --types cargo,npm ~/projects

# Clean projects not modified in 30+ days
rusty-sweeper clean --age 30 ~/projects

# Include Docker system resources in the cleanup report
rusty-sweeper clean --types docker ~/projects
```

### Interactive TUI

```bash
# Launch TUI at root
rusty-sweeper

# Start at specific directory
rusty-sweeper tui /home/user/projects
```

### Monitor Disk Usage

```bash
# Check once and exit
rusty-sweeper-monitor --once

# Start as daemon
rusty-sweeper-monitor --daemon

# Custom thresholds (warn at 70%, critical at 85%)
rusty-sweeper-monitor --warn 70 --critical 85

# Check daemon status
rusty-sweeper-monitor --status

# Stop daemon
rusty-sweeper-monitor --stop
```

## Configuration

Configuration file: `~/.config/rusty-sweeper/config.toml`

The file is currently loaded and validated on startup, but command behavior is not yet fully driven by these values. Treat it as partially implemented.

```toml
[scanner]
parallel_threads = 0  # 0 = auto
cross_filesystems = false
use_cache = true
cache_ttl = 3600

[cleaner]
project_types = ["cargo", "gradle", "npm", "maven"]
exclude_patterns = ["**/.git", "**/vendor"]
min_age_days = 7
max_depth = 10
parallel_jobs = 4

[monitor]
interval = 300  # seconds
warn_threshold = 80
critical_threshold = 90
mount_points = []
notification_backend = "auto"

[tui]
color_scheme = "auto"
show_hidden = false
default_sort = "size"
large_dir_threshold = 1073741824
```

## TUI Keybindings

| Key | Action |
|-----|--------|
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `Enter` / `l` / `Right` | Expand/enter directory |
| `h` / `Left` / `Backspace` | Collapse/go up |
| `d` / `Delete` | Delete selected |
| `c` | Clean project |
| `/` | Search |
| `s` | Cycle sort order |
| `.` | Toggle hidden files |
| `r` | Rescan |
| `?` | Help |
| `q` / `Esc` | Quit |

## Supported Project Types

These are the project types currently detected by `clean` when local artifacts are present:

| Type | Detection | Cleaned |
|------|-----------|---------|
| Cargo (Rust) | `Cargo.toml` | `target/` or `cargo clean` |
| npm/Node.js | `package.json` | `node_modules/` |
| Python | `venv/` or `.venv/` | `venv/`, `.venv/`, `__pycache__/` |
| CMake | `CMakeLists.txt` and `build/` | `build/` |
| Gradle | `build.gradle`, `build.gradle.kts`, or `gradlew` | `build/`, `.gradle/`, `app/build/` or `./gradlew clean` |
| Maven | `pom.xml` | `target/` or `mvn clean` |
| .NET | `*.csproj` or `*.sln` | `bin/`, `obj/` or `dotnet clean` |

Additional status:

- Docker system cleanup is available via the `docker` cleaner and reports reclaimable Docker images and build cache.
- Go and Bazel detectors exist in the codebase, but they are not currently reported by project scanning because they rely on command-only cleanup without local artifact directories.

## Systemd Service

To run the monitor as a systemd user service:

```bash
# Install the service
./dist/install-service.sh

# Enable and start
systemctl --user enable rusty-sweeper-monitor
systemctl --user start rusty-sweeper-monitor

# Check status
systemctl --user status rusty-sweeper-monitor

# View logs
journalctl --user -u rusty-sweeper-monitor -f
```

## License

MIT
