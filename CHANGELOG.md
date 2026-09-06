# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added recursive subtree cleanup from the TUI with aggregate preview, configured project filtering, and batch results.

## [0.3.0] - 2026-08-31

### Added

- Added an optional starting path argument to the TUI, defaulting to the current working directory.
- Added a persistent scanner metadata cache that validates entries using filesystem identity, size, allocated blocks, and nanosecond timestamps.
- Added progressive scan deltas so the TUI can update incrementally without repeatedly cloning the full tree.

### Changed

- Routed TUI project discovery and cleanup through the shared `ProjectScanner` and `CleanOrchestrator` pipeline.
- Moved scans, project detection, system-resource detection, deletion, and cleanup work off the TUI thread.
- Refresh only cleaned artifact subtrees in place instead of rescanning the full root.
- Honor configured scanner parallelism and prune excluded subtrees before descending into them.
- Count hard-linked file storage only once per scan.
- Cache project detection results and avoid cloning descendant trees into visible TUI rows.

### Fixed

- Pinned delete and clean confirmations to the entry originally confirmed.
- Restored terminal state when the TUI exits through an error path.
- Prevented parallel scans from repeatedly following symbolic-link cycles.
- Preserved byte counts and refreshed the tree after partially successful cleanup.
- Measured bytes actually freed by native project cleaners.
- Hardened monitor PID validation before signaling a daemon process.
- Retried failed disk alerts and allowed notifier fallback backends to run.
- Corrected custom disk thresholds, escaped mount paths, virtual-filesystem matching, and invalid explicit mount handling.
- Safely truncated long Unicode paths in confirmation dialogs.
- Installed the monitor binary at the path expected by the packaged service.

## [0.2.0] - 2026-06-19

### Changed

- Made the main `rusty-sweeper` binary launch the TUI directly.
- Split monitor functionality into the dedicated `rusty-sweeper-monitor` binary.
- Synchronized `README.md`, `SPEC.md`, and the man page with the current implementation status.
- Documented that `scan --sort mtime` currently falls back to size sorting.
- Documented that configuration is loaded and validated, but is not yet fully wired into runtime behavior.
- Documented that Go and Bazel detectors exist in code but are not currently surfaced by project scanning.
- Corrected the supported cleaner list to include `.NET` and Docker system cleanup.

### Added

- Added Docker system resource cleanup in the TUI and monitor-aware CLI surface.

### Fixed

- Fixed Docker image pruning so unused images are removed.
- Fixed Ctrl+C handling for monitor mode.

### Removed

- Removed legacy `scan`, `clean`, `tui`, and `completions` subcommands from the main binary.
- Removed packaged shell completion files for the deleted completion command.

## [0.1.0] - 2024-12-07

### Added

#### Core Features
- **Directory Scanning**: Fast parallel directory scanning with real-time progress
- **Size Analysis**: Accurate disk usage calculation with human-readable formatting
- **Interactive TUI**: Terminal user interface for browsing and managing disk usage
- **CLI Commands**: Comprehensive command-line interface for all operations

#### Scanner
- Parallel scanning using rayon for improved performance
- Progressive scanning with real-time size updates
- Symlink detection and handling (skip by default)
- Hidden file support (configurable)
- Maximum depth limiting
- File filtering by pattern

#### TUI Features
- Tree-based navigation with expand/collapse
- Visual size bars showing relative sizes
- Multiple sort options (size, name, mtime in TUI)
- File/directory deletion with confirmation
- Keyboard navigation (vim-style and arrow keys)
- Delete key support for triggering deletion

#### CLI Commands
- `scan`: Scan directories and report disk usage
- `tui`: Interactive terminal interface
- `clean`: Clean build artifacts (Cargo, npm, Python, etc.)
- `monitor`: Background monitoring daemon with alerts
- `completions`: Generate shell completions

#### Build Artifact Detection
- Cargo projects (`target/`)
- Node.js projects (`node_modules/`)
- Python environments (`venv/`, `.venv/`, `__pycache__/`)
- CMake projects (`build/`)
- Gradle projects (`build/`, `.gradle/`)
- Maven projects (`target/`)
- .NET projects (`bin/`, `obj/`)
- Docker system resources (reclaimable images and build cache)

#### Monitor Service
- Background daemon for disk usage monitoring
- Configurable threshold alerts
- Desktop notifications via notify-rust
- PID file management for single instance
- Graceful shutdown handling

#### Configuration
- TOML configuration file support
- XDG-compliant config location (~/.config/rusty-sweeper/config.toml)
- Configuration parsing and validation
- Structured sections for monitor, cleaner, scanner, and TUI settings

#### Output Formats
- Human-readable (default)
- JSON for scripting
- Tree view for hierarchical display

#### Documentation
- Comprehensive man page
- Shell completions (bash, zsh, fish)
- README with examples and configuration guide

#### Packaging
- GitHub Actions CI/CD workflows
- AUR PKGBUILD for Arch Linux
- Release automation

### Technical Details
- Written in Rust with safety guarantees
- Uses ratatui for TUI rendering
- Uses crossterm for terminal handling
- Uses walkdir for directory traversal
- Uses rayon for parallel processing
- Uses clap for CLI parsing
- Uses tracing for logging

[Unreleased]: https://github.com/lelloman/rusty-sweeper/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/lelloman/rusty-sweeper/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/lelloman/rusty-sweeper/releases/tag/v0.2.0
[0.1.0]: https://github.com/lelloman/rusty-sweeper/releases/tag/v0.1.0
