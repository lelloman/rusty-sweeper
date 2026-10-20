# Rusty Sweeper

A fast, interactive disk usage analyzer and cleaner for Linux.

## Current Status

- `rusty-sweeper [PATH]` launches the interactive TUI.
- `rusty-sweeper --cli [PATH]` prints a disk-usage report and exits.
- Add `--json` for machine-readable output or `--depth N` to limit text display depth.
- `rusty-sweeper --cli --clean [PATH]` previews project cleanup and asks for confirmation.
- The monitor binary and service have been removed.

## Features

- **Interactive TUI**: Browse disk usage and clean directly from the terminal
- **CLI reports**: Print disk usage as a tree or JSON for scripts

## Installation

### From crates.io

```bash
cargo install rusty-sweeper
```

### From Source

```bash
git clone https://github.com/lelloman/rusty-sweeper
cd rusty-sweeper
cargo install --path .
```

## Usage

### Interactive TUI

```bash
# Launch TUI in the current working directory
rusty-sweeper

# Scan a specific directory instead of the current working directory
rusty-sweeper /path/to/scan
```

### Non-interactive CLI

```bash
# Print a full disk-usage tree (defaults to the current directory)
rusty-sweeper --cli /path/to/scan

# Show the root and its immediate children; totals still include all descendants
rusty-sweeper --cli --depth 1 /path/to/scan

# Export the full scan as JSON
rusty-sweeper --cli --json /path/to/scan > usage.json
```

Reports include hidden entries and use the scanner configuration below. Text output
marks entries with scan errors as `[!]`; JSON includes their error details.
Diagnostics go to stderr, leaving stdout suitable for piping. `--json` and `--depth`
require `--cli` and cannot be combined.

### CLI project cleanup

```bash
# Discover projects, report artifacts and sizes, then ask for confirmation
rusty-sweeper --cli --clean /path/to/projects

# Print the same report and clean automatically
rusty-sweeper --cli --clean -y /path/to/projects
```

The scan includes the root project and nested projects, using `[cleaner]` settings
for project types, exclusions, maximum depth, minimum age, and parallel jobs.
Cleanup deletes the listed local artifact directories directly. It does not run
project scripts or clean global caches. Enter `y` or `yes` to confirm; any other
response, an empty line, or end-of-input cancels. `-y` / `--yes` skips confirmation.
The final report includes successes, failures, and space freed; any cleanup failure
returns a nonzero exit status. `--clean` cannot be combined with `--json` or `--depth`.

## Configuration

Configuration file: `~/.config/rusty-sweeper/config.toml`

Scanner and TUI cleaner behavior is driven by the corresponding values.

Scanner parallelism, filesystem-boundary behavior, and the persistent metadata cache are active. Cached files are validated by device/inode, size, allocated blocks, and nanosecond modification/change times before their accounting is reused.

```toml
[scanner]
parallel_threads = 0  # 0 = auto
cross_filesystems = false
use_cache = true
cache_ttl = 3600

[cleaner]
project_types = ["cargo", "gradle", "maven", "npm", "go", "cmake", "python", "bazel", "dotnet"]
exclude_patterns = ["**/.git", "**/vendor"]
min_age_days = 0
max_depth = 10
parallel_jobs = 4

[tui]
color_scheme = "auto"
show_hidden = false
default_sort = "size"
large_dir_threshold = 1073741824
```

## TUI Keybindings

Scanning and cleanup run in background workers. The UI processes queued scan
results in batches and redraws changed frames at most 60 times per second;
keyboard input and terminal resizing are handled independently. Deleting an
entry updates the in-memory tree without rescanning the root. Press `r` to
refresh changes made outside the application.

If a deletion fails with a permission error, the TUI offers to retry that path
with sudo. Accepting temporarily opens sudo's terminal password prompt, then
returns to the TUI. Press `n` or `Esc` to leave the remaining files in place.

| Key | Action |
|-----|--------|
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `Enter` / `l` / `Right` | Expand/enter directory |
| `h` / `Left` / `Backspace` | Collapse/go up |
| `d` / `Delete` | Delete selected |
| `c` | Clean project |
| `C` | Clean all eligible projects under selected directory |
| `/` | Search |
| `s` | Cycle sort order |
| `.` | Hide/show hidden files (shown by default) |
| `r` | Rescan |
| `?` | Help |
| `Esc` | Cancel active work, otherwise quit |
| `q` | Quit |

## Cleanup Support

The TUI can identify and clean common local build artifacts for Cargo, npm, Python, CMake, Gradle, Maven, and .NET projects. Docker system resources are also surfaced when available.

## License

MIT
