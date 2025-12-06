//! Rusty Sweeper - A Linux disk usage management utility
//!
//! This crate provides functionality for:
//! - Monitoring disk usage with desktop notifications
//! - Discovering and cleaning build artifacts
//! - Interactive TUI for disk exploration

pub mod cli;
pub mod config;
pub mod error;
pub mod scanner;

// Re-export commonly used types
pub use config::Config;
pub use error::{Result, SweeperError};
pub use scanner::{DirEntry, ScanOptions};
