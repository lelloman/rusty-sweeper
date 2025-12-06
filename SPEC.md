# Rusty Sweeper - Design Specification

A Linux disk usage management utility written in Rust.

---

# Part 1: Project Specification

## 1.1 Overview

Rusty Sweeper prevents disk space exhaustion through three core capabilities:

1. **Monitor** - Proactive disk usage alerts via desktop notifications
2. **Clean** - Automated discovery and cleanup of build artifacts
3. **TUI** - Interactive filesystem explorer for disk usage analysis

### Goals

- Prevent surprise disk-full situations
- Automate tedious cleanup of build caches across project types
- Provide visibility into what's consuming disk space
- Work seamlessly across desktop environments (i3, GNOME, KDE)

### Non-Goals

- Cloud storage integration
- File backup/restore
- Real-time inotify-based monitoring
- Windows/macOS support

---

## 1.2 Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLI (clap)                              │
│    ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│    │ monitor  │  │  clean   │  │   scan   │  │   tui    │      │
│    └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘      │
├─────────┼─────────────┼────────────┼─────────────┼──────────────┤
│         │             │            │             │              │
│    ┌────▼─────┐  ┌────▼─────┐  ┌───▼────────────▼────┐         │
│    │ Notifier │  │ Cleaner  │  │    Disk Scanner     │         │
│    └────┬─────┘  └────┬─────┘  └──────────┬──────────┘         │
│         │             │                   │                     │
├─────────┴─────────────┴───────────────────┴─────────────────────┤
│                        Core Library                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │ Project Detector│  │  Size Calculator │  │  Config Manager │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 1.3 CLI Interface

```
rusty-sweeper <COMMAND>

Commands:
  monitor   Start disk usage monitoring (daemon or one-shot)
  clean     Discover and clean build artifacts
  scan      Analyze disk usage of a directory
  tui       Launch interactive disk explorer

Global Options:
  -c, --config <PATH>   Config file path
  -v, --verbose         Increase verbosity
  -q, --quiet           Suppress non-essential output
  -h, --help            Print help
  -V, --version         Print version
```

---

## 1.4 Monitor Service

Periodically checks disk usage and sends desktop notifications when thresholds are exceeded.

### Command

```
rusty-sweeper monitor [OPTIONS]

Options:
  -d, --daemon              Run as background daemon
  -i, --interval <SECS>     Check interval [default: 300]
  -w, --warn <PERCENT>      Warning threshold [default: 80]
  -c, --critical <PERCENT>  Critical threshold [default: 90]
  -m, --mount <PATH>        Mount point to monitor [default: /]
      --once                Check once and exit
```

### Notification Backends

| Backend | Detection | Method |
|---------|-----------|--------|
| D-Bus (primary) | Always attempted first | `notify-rust` crate |
| notify-send | Fallback | Shell out to binary |
| i3-nagbar | `$I3SOCK` present | For critical alerts |
| stderr | Always available | Last resort fallback |

### Notification Levels

| Level | Trigger | Urgency |
|-------|---------|---------|
| Warning | usage >= 80% | Normal |
| Critical | usage >= 90% | Critical |
| Emergency | usage >= 95% | Critical + persistent |

### Daemon Mode

- Daemonize via `fork()` or run under systemd
- PID file: `$XDG_RUNTIME_DIR/rusty-sweeper.pid`
- Log file: `$XDG_STATE_HOME/rusty-sweeper/monitor.log`
- Signal handling: SIGHUP (reload config), SIGTERM (shutdown)

### Systemd Integration

```ini
[Unit]
Description=Rusty Sweeper Disk Monitor
After=graphical-session.target

[Service]
Type=simple
ExecStart=/usr/bin/rusty-sweeper monitor
Restart=on-failure

[Install]
WantedBy=default.target
```

---

## 1.5 Cleaner Engine

Discovers coding projects and executes appropriate clean commands.

### Command

```
rusty-sweeper clean [OPTIONS] [PATH]

Arguments:
  [PATH]  Root directory to scan [default: .]

Options:
  -n, --dry-run           Show what would be cleaned
  -d, --max-depth <N>     Maximum recursion depth [default: 10]
  -t, --types <TYPES>     Project types to clean (comma-separated)
  -e, --exclude <PATHS>   Exclude patterns (glob)
  -a, --age <DAYS>        Only clean if not modified in N days
  -f, --force             Skip confirmation prompts
  -j, --jobs <N>          Parallel clean jobs [default: 4]
      --size-only         Report sizes without cleaning
```

### Supported Project Types

| Type | Detection Files | Clean Command | Artifacts |
|------|-----------------|---------------|-----------|
| Cargo | `Cargo.toml` | `cargo clean` | `target/` |
| Gradle | `gradlew`, `build.gradle` | `./gradlew clean` | `build/`, `.gradle/` |
| Maven | `pom.xml` | `mvn clean` | `target/` |
| npm | `package.json` | `rm -rf node_modules` | `node_modules/` |
| Go | `go.mod` | `go clean -cache` | module cache |
| CMake | `CMakeLists.txt` + `build/` | `rm -rf build/` | `build/` |
| Python | `venv/`, `.venv/` | `rm -rf venv/` | `venv/`, `__pycache__/` |
| Bazel | `WORKSPACE` | `bazel clean` | `bazel-*` |
| .NET | `*.csproj`, `*.sln` | `dotnet clean` | `bin/`, `obj/` |

### Detection Algorithm

```
detect_projects(path, depth):
    if depth > max_depth: return []

    for detector in detectors:
        if detector.matches(path):
            return [Project(path, detector)]  # Don't recurse into projects

    projects = []
    for subdir in path.subdirs():
        if not excluded(subdir):
            projects += detect_projects(subdir, depth + 1)
    return projects
```

### Confirmation UI

```
Found 5 projects with cleanable artifacts:

  TYPE      PATH                              SIZE
  ──────────────────────────────────────────────────
  cargo     ~/projects/rusty-sweeper          1.2 GB
  gradle    ~/projects/android-app            3.4 GB
  npm       ~/projects/web-frontend           512 MB

  Total: 5.1 GB

Proceed with cleanup? [y/N/s(elect)]
```

### Safety Measures

1. Only delete known artifact directories (never source files)
2. Prefer native clean commands over `rm -rf`
3. Warn on uncommitted git changes (optional)
4. Age verification with `--age` flag
5. Dry-run mode for preview

---

## 1.6 Disk Scanner

Parallel directory traversal with size calculation.

### Command

```
rusty-sweeper scan [OPTIONS] [PATH]

Arguments:
  [PATH]  Directory to analyze [default: .]

Options:
  -d, --max-depth <N>     Display depth [default: 3]
  -n, --top <N>           Show top N entries [default: 20]
  -a, --all               Include hidden files
  -x, --one-file-system   Don't cross mount boundaries
  -j, --jobs <N>          Parallel threads [default: num_cpus]
      --json              JSON output
      --sort <BY>         Sort: size|name|mtime [default: size]
```

### Data Model

```rust
struct DirEntry {
    path: PathBuf,
    size: u64,              // Apparent size
    disk_usage: u64,        // Actual blocks used
    file_count: u64,
    dir_count: u64,
    mtime: SystemTime,
    children: Vec<DirEntry>,
}
```

### Performance

- Parallel traversal using `rayon`
- Work-stealing for balanced load
- Optional caching to SQLite (`$XDG_CACHE_HOME/rusty-sweeper/cache.db`)

---

## 1.7 TUI Interface

Interactive terminal UI for exploring and managing disk usage.

### Command

```
rusty-sweeper tui [OPTIONS] [PATH]

Arguments:
  [PATH]  Starting directory [default: /]

Options:
  -x, --one-file-system   Don't cross mount boundaries
      --no-color          Disable colors
```

### Layout

```
┌─ Rusty Sweeper ──────────────────────────────── 85% used ─┐
│ /home/user                                        42.5 GB │
├───────────────────────────────────────────────────────────┤
│ ▼ projects/                              [████████░░] 28G │
│   ▼ android-app/                         [██████░░░░] 18G │
│     ► build/                             [█████░░░░░] 15G │
│       .gradle/                           [██░░░░░░░░]  3G │
│   ► rusty-sweeper/                       [███░░░░░░░]  5G │
│   ► web-frontend/                        [██░░░░░░░░]  3G │
│ ► .cache/                                [███░░░░░░░]  8G │
│ ► .local/                                [██░░░░░░░░]  4G │
├───────────────────────────────────────────────────────────┤
│ [↑↓] Navigate  [←→] Expand  [d] Delete  [c] Clean  [q] Quit│
└───────────────────────────────────────────────────────────┘
```

### Keybindings

| Key | Action |
|-----|--------|
| ↑/↓, j/k | Navigate |
| →/←, l/h, Enter | Expand/collapse |
| d | Delete (with confirmation) |
| c | Clean project artifacts |
| / | Search/filter |
| s | Cycle sort order |
| r | Refresh/rescan |
| Space | Mark for batch operation |
| ? | Help |
| q, Esc | Quit |

### Visual Indicators

| Symbol | Meaning |
|--------|---------|
| ▼ | Expanded directory |
| ► | Collapsed directory |
| [P] | Detected project (cleanable) |
| [!] | Large directory (>1GB) |
| [X] | Permission denied |

---

## 1.8 Configuration

### Location

- User: `$XDG_CONFIG_HOME/rusty-sweeper/config.toml`
- System: `/etc/rusty-sweeper/config.toml`

### Format

```toml
[monitor]
interval = 300
warn_threshold = 80
critical_threshold = 90
mount_points = ["/", "/home"]
notification_backend = "auto"  # auto|dbus|notify-send|stderr

[cleaner]
project_types = ["cargo", "gradle", "npm", "maven"]
exclude_patterns = ["**/.git", "**/vendor"]
min_age_days = 7
max_depth = 10
parallel_jobs = 4

[scanner]
parallel_threads = 0  # 0 = auto
cross_filesystems = false
use_cache = true
cache_ttl = 3600

[tui]
color_scheme = "auto"  # auto|dark|light|none
show_hidden = false
default_sort = "size"
large_dir_threshold = 1073741824  # 1 GB

# Custom project types
[project_types.cargo]
enabled = true
detection_files = ["Cargo.toml"]
clean_command = "cargo clean"
artifact_dirs = ["target"]
```

---

## 1.9 Error Handling

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Permission denied |
| 4 | Interrupted (SIGINT) |
| 5 | Partial failure |

### Error Strategy

| Error Type | Handling |
|------------|----------|
| Permission denied | Log, skip, continue |
| Disk I/O error | Retry with backoff, then fail gracefully |
| Invalid config | Use defaults, warn user |
| Clean command fails | Log, report, continue to next project |

---

## 1.10 Dependencies

### Rust Crates

| Crate | Purpose |
|-------|---------|
| `clap` | CLI parsing |
| `ratatui` + `crossterm` | TUI |
| `tokio` | Async runtime |
| `rayon` | Parallelism |
| `walkdir` | Directory traversal |
| `notify-rust` | Desktop notifications |
| `serde` + `toml` | Configuration |
| `tracing` | Logging |
| `anyhow` + `thiserror` | Error handling |
| `dirs` | XDG paths |
| `humansize` | Size formatting |
| `indicatif` | Progress bars |

### System Requirements

- Linux kernel 4.x+
- D-Bus (for notifications)

---

## 1.11 Security

1. Run as regular user, respect permissions
2. No arbitrary command execution (predefined clean commands only)
3. Path traversal protection (no symlink escape)
4. Audit logging for delete operations
5. Dry-run default in CI environments

---

# Part 2: Implementation Phases

## Phase 1: Foundation

**Goal**: Project scaffold and core infrastructure.

### Deliverables

1. Cargo project setup with workspace structure
2. CLI skeleton with `clap` (all subcommands defined, no implementation)
3. Configuration module (load/parse TOML, XDG paths)
4. Error types with `thiserror`
5. Logging setup with `tracing`

### Structure

```
rusty-sweeper/
├── Cargo.toml
├── src/
│   ├── main.rs           # Entry point, CLI dispatch
│   ├── cli.rs            # clap definitions
│   ├── config.rs         # Configuration loading
│   ├── error.rs          # Error types
│   └── lib.rs            # Library root
```

---

## Phase 2: Disk Scanner

**Goal**: Core scanning functionality used by all other features.

### Deliverables

1. Directory traversal with `walkdir`
2. Parallel size calculation with `rayon`
3. `DirEntry` data structure with tree building
4. `scan` subcommand implementation
5. JSON output option
6. Unit tests for size calculations

### Key Functions

```rust
pub fn scan_directory(path: &Path, opts: &ScanOptions) -> Result<DirEntry>;
pub fn format_tree(entry: &DirEntry, depth: usize) -> String;
```

---

## Phase 3: Project Detection & Cleaner

**Goal**: Detect projects and clean build artifacts.

### Deliverables

1. Project detector trait and implementations
2. Built-in detectors for all 9 project types
3. Artifact size calculation
4. Clean command execution
5. `clean` subcommand with dry-run, confirmation, parallel execution
6. Integration tests with temp directories

### Key Types

```rust
pub trait ProjectDetector {
    fn name(&self) -> &str;
    fn detect(&self, path: &Path) -> bool;
    fn artifact_dirs(&self) -> &[&str];
    fn clean_command(&self) -> &str;
}

pub struct DetectedProject {
    pub path: PathBuf,
    pub project_type: String,
    pub artifact_size: u64,
}
```

---

## Phase 4: TUI

**Goal**: Interactive disk explorer.

### Deliverables

1. Basic `ratatui` app structure
2. Tree view widget with expand/collapse
3. Navigation (vim keys + arrows)
4. Size bars and formatting
5. Delete with confirmation dialog
6. Clean integration (detect + execute)
7. Search/filter
8. Help overlay

### Components

```rust
struct App {
    tree: TreeState,
    entries: Vec<DirEntry>,
    selected: usize,
    mode: Mode,  // Normal, Search, Confirm, Help
}
```

---

## Phase 5: Monitor Service

**Goal**: Background disk monitoring with notifications.

### Deliverables

1. Disk usage checking (statvfs)
2. Notification backend abstraction
3. D-Bus notification implementation
4. notify-send fallback
5. Daemon mode (fork, PID file, signal handling)
6. `monitor` subcommand (one-shot and daemon)
7. Systemd service file

### Key Types

```rust
pub trait Notifier {
    fn send(&self, level: AlertLevel, message: &str) -> Result<()>;
}

pub struct DiskStatus {
    pub mount_point: PathBuf,
    pub total: u64,
    pub used: u64,
    pub percent: f32,
}
```

---

## Phase 6: Polish & Release

**Goal**: Production readiness.

### Deliverables

1. Comprehensive error messages
2. Man page generation
3. Shell completions (bash, zsh, fish)
4. README with examples
5. CI/CD pipeline (GitHub Actions)
6. Release binaries
7. AUR PKGBUILD

---

## Phase Summary

| Phase | Focus | Key Crates |
|-------|-------|------------|
| 1 | Foundation | clap, serde, toml, tracing, thiserror |
| 2 | Scanner | walkdir, rayon |
| 3 | Cleaner | (uses scanner) |
| 4 | TUI | ratatui, crossterm |
| 5 | Monitor | notify-rust, nix (for daemon) |
| 6 | Polish | clap_complete, clap_mangen |

---

## Testing Strategy

### Per-Phase Testing

- **Phase 1**: Config parsing tests
- **Phase 2**: Size calculation, tree building tests
- **Phase 3**: Project detection, mock clean execution
- **Phase 4**: Snapshot tests with `insta`
- **Phase 5**: Notification mocking, threshold tests

### Integration Tests

```bash
# Test fixture script
./scripts/create-test-fixtures.sh /tmp/test-projects
cargo test --test integration
```

---

## Success Criteria

The project is complete when:

1. `rusty-sweeper scan ~` shows disk usage tree
2. `rusty-sweeper clean ~/projects` finds and cleans build artifacts
3. `rusty-sweeper tui` provides interactive exploration
4. `rusty-sweeper monitor --daemon` runs in background and sends notifications
5. All commands respect configuration file
6. Clean exits on Ctrl+C
7. Handles permission errors gracefully
