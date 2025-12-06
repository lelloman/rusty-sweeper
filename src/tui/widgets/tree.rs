//! Tree widget for displaying directory structure.
//!
//! Note: Tree rendering is implemented directly in `ui.rs` for simplicity.
//! This module is kept for potential future extraction into a reusable widget.

// Tree rendering is handled in ui.rs:render_tree_area and ui.rs:render_entry
// The implementation includes:
// - Entry rendering with indentation, icons, size bars
// - Selection highlighting
// - Scroll offset calculation
