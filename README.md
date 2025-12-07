# Rusty Sweeper

A fast, interactive disk usage analyzer and cleaner for Linux.

## Features

- **Disk Scanner**: Analyze disk usage with parallel traversal
- **Project Cleaner**: Detect and clean build artifacts (Cargo, npm, Go, Python, CMake, Gradle, Maven)
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

# Custom thresholds (warn at 70%, critical at 85%)
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
parallel_threads = 0  # 0 = auto
cross_filesystems = false

[cleaner]
project_types = ["cargo", "npm", "go", "python"]
max_depth = 10

[monitor]
interval = 300  # seconds
warn_threshold = 80
critical_threshold = 90

[tui]
color_scheme = "auto"
show_hidden = false
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
| `.` | Toggle hidden files |
| `r` | Rescan |
| `?` | Help |
| `q` / `Esc` | Quit |

## Supported Project Types

| Type | Detection | Cleaned |
|------|-----------|---------|
| Cargo (Rust) | `Cargo.toml` | `target/` |
| npm/Node.js | `package.json` | `node_modules/` |
| Go | `go.mod` | `go build` cache |
| Python | `setup.py`, `pyproject.toml` | `__pycache__/`, `*.pyc`, `.eggs/`, `*.egg-info/` |
| CMake | `CMakeLists.txt` | `build/`, `cmake-build-*/` |
| Gradle | `build.gradle` | `build/`, `.gradle/` |
| Maven | `pom.xml` | `target/` |

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
