//! Rusty Sweeper - A Linux disk usage management utility
//!
//! This crate provides functionality for:
//! - Non-interactive disk usage reports
//! - Discovering and cleaning build artifacts
//! - Interactive TUI for disk exploration

pub mod cleaner;
pub mod cli;
pub mod config;
pub mod error;
pub mod scanner;
pub mod tui;

// Re-export commonly used types
pub use config::Config;
pub use error::{Result, SweeperError};
pub use scanner::{
    format_json, format_json_summary, format_size, format_table, format_tree, scan_directory,
    scan_directory_parallel, DirEntry, FormatOptions, ScanOptions, SummarizedEntry,
};
