# Rusty Sweeper - Design Specification

A Linux disk usage management utility written in Rust.

This document reflects the current implementation status of the project, including shipped features and known gaps.

---

# Part 1: Project Specification

## 1.1 Overview

Rusty Sweeper currently exposes two user-facing capabilities:

1. **Monitor** - Proactive disk usage alerts via desktop notifications
2. **TUI** - Interactive filesystem explorer for disk usage analysis and cleanup

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

### Current Implementation Notes

- `rusty-sweeper` launches the TUI directly.
- `rusty-sweeper-monitor` is the dedicated monitor binary.
- The configuration file is loaded and validated, but most command behavior is still controlled directly by CLI flags.
- Scan and cleanup engines still exist in the codebase, but are no longer exposed as standalone subcommands on the main binary.

---

## 1.2 Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLI (clap)                              │
│    ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│    │ monitor* │  │  clean   │  │   scan   │  │   tui    │      │
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

Programs:
  rusty-sweeper            Launch the TUI
  rusty-sweeper-monitor    Start disk usage monitoring (daemon or one-shot)

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
rusty-sweeper-monitor [OPTIONS]

Options:
  -d, --daemon              Run as background daemon
  -i, --interval <SECS>     Check interval [default: 300]
  -w, --warn <PERCENT>      Warning threshold [default: 80]
  -C, --critical <PERCENT>  Critical threshold [default: 90]
  -m, --mount <PATH>        Mount point to monitor (repeatable)
      --notify <BACKEND>    Notification backend: auto|dbus|notify-send|stderr
      --once                Check once and exit
      --stop                Stop a running daemon
      --status              Show daemon status
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

Current status:

- Daemon mode, PID/log handling, stop/status commands, and notifier backend selection are implemented.
- SIGHUP sets a reload flag, but runtime config reload is not currently applied to the running monitor service.

### Systemd Integration

```ini
[Unit]
Description=Rusty Sweeper Disk Monitor
After=graphical-session.target

[Service]
Type=simple
ExecStart=/usr/bin/rusty-sweeper-monitor
Restart=on-failure

[Install]
WantedBy=default.target
```

---

## 1.5 Cleaner Engine

Discovers coding projects and executes appropriate clean operations from within the TUI.

### Supported Project Types

| Type | Detection Files | Clean Command | Artifacts |
|------|-----------------|---------------|-----------|
| Cargo | `Cargo.toml` | `cargo clean` | `target/` |
| Gradle | `gradlew`, `build.gradle`, `build.gradle.kts` | `./gradlew clean` | `build/`, `.gradle/`, `app/build/` |
| Maven | `pom.xml` | `mvn clean` | `target/` |
| npm | `package.json` | direct deletion | `node_modules/` |
| Go | `go.mod` | `go clean -cache` | global cache |
| CMake | `CMakeLists.txt` and `build/` | direct deletion | `build/` |
| Python | `venv/`, `.venv/` | direct deletion | `venv/`, `.venv/`, `__pycache__/` |
| Bazel | `WORKSPACE`, `WORKSPACE.bazel` | `bazel clean --expunge` | command-only |
| .NET | `*.csproj`, `*.sln` | `dotnet clean` | `bin/`, `obj/` |
| Docker | Docker daemon available | `docker builder prune` / `docker image prune -a` | build cache, reclaimable images |

Current behavior:

- The TUI only offers cleanup when local artifact directories are present.
- Because of that, Go and Bazel are currently not surfaced despite detector definitions.
- Docker is implemented as a system cleaner, not as a project detector.

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

Proceed with cleanup? [y/N]
```

### Safety Measures

1. Only delete known artifact directories for detector-based cleanup
2. Prefer native clean commands when available for a detector
3. Age verification with `--age` flag
4. Dry-run mode for preview
5. Confirmation prompt unless `--force` is used

Not currently implemented:

- Git dirty-tree warnings
- A standalone non-interactive cleanup CLI

---

## 1.6 Disk Scanner

Parallel directory traversal with size calculation. This powers the TUI and remains available as an internal engine.

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

Current status:

- Parallel scanning is implemented.
- Hidden file filtering and depth limiting are implemented in the engine.
- JSON output and standalone scan formatting still exist in code, but are no longer exposed on the main CLI.
- Persistent scan caching is not implemented.

---

## 1.7 TUI Interface

Interactive terminal UI for exploring and managing disk usage.

### Command

```
rusty-sweeper
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
| . | Toggle hidden files |
| Space | Toggle expand/collapse |
| ? | Help |
| q, Esc | Quit |

### Visual Indicators

| Symbol | Meaning |
|--------|---------|
| ▼ | Expanded directory |
| ► | Collapsed directory |
| `[Rust]`, `[npm]`, etc. | Detected project type |
| [X] | Permission denied |

Current status:

- Tree browsing, search, deletion, project cleaning, and progressive background scanning are implemented.
- The TUI also displays Docker system resources when available.
- Batch marking/queueing is not implemented.
- The interactive mode currently starts from `/` when launched without further arguments.

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

Current status:

- The config file is loaded from the documented locations and validated.
- The structured config fields above exist in code.
- Most command behavior is not yet driven from config values, and custom project type definitions are not implemented.

---

## 1.9 Error Handling

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid cleaner type selection |
| 5 | Partial failure during cleanup |

### Error Strategy

| Error Type | Handling |
|------------|----------|
| Permission denied | Log or render error entry, then continue where possible |
| Disk I/O error | Return an error or keep partial scan state, depending on command |
| Invalid explicit config path | Fail startup |
| Clean command fails | Warn and fall back to direct deletion when local artifacts exist |

---

## 1.10 Dependencies

### Rust Crates

| Crate | Purpose |
|-------|---------|
| `clap` | CLI parsing |
| `ratatui` + `crossterm` | TUI |
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
2. No arbitrary command execution beyond predefined cleaner commands
3. Scanner avoids following symlinks by default
4. Destructive delete/clean actions require confirmation unless explicitly forced
5. No trash or recovery layer is currently implemented

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
4. Scanner integration for the TUI and internal formatting paths
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
5. Cleaner integration for the TUI with confirmation and parallel execution
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
| 6 | Polish | clap_mangen |

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

1. `rusty-sweeper` provides interactive exploration
2. `rusty-sweeper-monitor --daemon` runs in background and sends notifications
5. All commands respect configuration file
6. Clean exits on Ctrl+C
7. Handles permission errors gracefully
