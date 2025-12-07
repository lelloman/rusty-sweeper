# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
- Multiple sort options (size, name, count)
- File/directory deletion with confirmation
- Dry-run preview mode
- Keyboard navigation (vim-style and arrow keys)
- Delete key support for triggering deletion

#### CLI Commands
- `scan`: Scan directories and report disk usage
- `tui`: Interactive terminal interface
- `clean`: Clean build artifacts (Cargo, npm, Python, etc.)
- `find-large`: Find files larger than specified size
- `monitor`: Background monitoring daemon with alerts
- `completions`: Generate shell completions

#### Build Artifact Detection
- Cargo projects (target/)
- Node.js projects (node_modules/)
- Python projects (__pycache__/, .eggs/, *.egg-info/)
- Go projects (vendor/)
- Generic patterns (dist/, build/, .cache/)

#### Monitor Service
- Background daemon for disk usage monitoring
- Configurable threshold alerts
- Desktop notifications via notify-rust
- PID file management for single instance
- Graceful shutdown handling

#### Configuration
- TOML configuration file support
- XDG-compliant config location (~/.config/rusty-sweeper/config.toml)
- Customizable thresholds and patterns
- Per-command configuration options

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

[0.1.0]: https://github.com/lelloman/rusty-sweeper/releases/tag/v0.1.0
