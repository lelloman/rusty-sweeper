# Rusty Sweeper

A fast, interactive disk usage analyzer and cleaner for Linux.

## Current Status

Rusty Sweeper is implemented and test-covered across its main command surface:

- `rusty-sweeper` is the interactive product and launches the TUI directly.
- `rusty-sweeper-monitor` is a dedicated monitor binary with one-shot checks, daemon mode, PID/log management, and multiple notification backends.

Current limitations:

- The configuration file is loaded and validated, but most command behavior is still driven directly by CLI flags rather than config values.
- Cleanup and scan engines still exist in the codebase, but they are no longer exposed as standalone CLI subcommands.

## Features

- **Interactive TUI**: Browse disk usage and clean directly from the terminal
- **Monitor Service**: Background daemon with desktop notifications when disk is full

## Installation

### From Source

```bash
git clone https://github.com/user/rusty-sweeper
cd rusty-sweeper
cargo install --path .
```

## Usage

### Interactive TUI

```bash
# Launch TUI at root
rusty-sweeper

# `rusty-sweeper` is the only user-facing mode of the main binary
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

The file is currently loaded and validated on startup, but behavior is not yet fully driven by these values. Treat it as partially implemented.

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

## Cleanup Support

The TUI can identify and clean common local build artifacts for Cargo, npm, Python, CMake, Gradle, Maven, and .NET projects. Docker system resources are also surfaced when available.

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
