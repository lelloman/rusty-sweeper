# Phase 4: TUI Implementation Plan

## Overview

This document provides a detailed, step-by-step implementation plan for the TUI (Terminal User Interface) component of Rusty Sweeper. Each task is designed to be completed in a single focused session.

### Prerequisites

- Phase 2 (Disk Scanner) must be complete
- Phase 3 (Cleaner) must be complete
- Dependencies: `ratatui`, `crossterm`

### Status Legend

- `[ ]` - Not started
- `[~]` - In progress
- `[x]` - Complete

---

## Task 4.1: Add TUI Dependencies

**Status**: `[x]`

### Description

Add the required crates for building the TUI to `Cargo.toml`.

### Context

We use `ratatui` (the maintained fork of `tui-rs`) for rendering widgets and `crossterm` as the terminal backend. This combination is cross-platform and well-maintained.

### Implementation

Add to `Cargo.toml`:

```toml
[dependencies]
ratatui = "0.28"
crossterm = "0.28"
```

### Tests

- `cargo check` passes
- `cargo build` compiles without errors

---

## Task 4.2: Create TUI Module Structure

**Status**: `[x]`

### Description

Set up the module structure for the TUI code.

### Context

Organizing TUI code into separate modules improves maintainability. We separate concerns: app state, UI rendering, event handling, and widgets.

### Implementation

Create the following file structure:

```
src/
├── tui/
│   ├── mod.rs          # Module exports
│   ├── app.rs          # Application state
│   ├── ui.rs           # UI rendering
│   ├── event.rs        # Event handling
│   ├── widgets/
│   │   ├── mod.rs
│   │   └── tree.rs     # Tree widget
│   └── actions.rs      # User actions (delete, clean, etc.)
```

`src/tui/mod.rs`:
```rust
pub mod app;
pub mod ui;
pub mod event;
pub mod widgets;
pub mod actions;

pub use app::App;
```

### Tests

- Module compiles
- All submodules are accessible from `crate::tui`

---

## Task 4.3: Implement Terminal Setup/Teardown

**Status**: `[x]`

### Description

Create functions to initialize and restore the terminal state.

### Context

TUI applications must switch the terminal to raw mode and an alternate screen. On exit (including panics), the terminal must be restored to its original state to avoid leaving the user's terminal in a broken state.

### Implementation

`src/tui/mod.rs` (add to existing):

```rust
use std::io::{self, stdout, Stdout};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub fn init_terminal() -> io::Result<Tui> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    Terminal::new(backend)
}

pub fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

/// Install panic hook that restores terminal before printing panic
pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));
}
```

### Tests

```rust
#[test]
fn test_terminal_init_restore() {
    // This test should only run in interactive mode, skip in CI
    if std::env::var("CI").is_ok() {
        return;
    }
    let terminal = init_terminal();
    assert!(terminal.is_ok());
    assert!(restore_terminal().is_ok());
}
```

---

## Task 4.4: Define Application State

**Status**: `[x]`

### Description

Create the `App` struct that holds all TUI state.

### Context

The application state contains the directory tree, current selection, view mode, and any transient UI state (dialogs, search input, etc.).

### Implementation

`src/tui/app.rs`:

```rust
use std::path::PathBuf;
use crate::scanner::DirEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Search,
    Confirm(ConfirmAction),
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    Delete,
    Clean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Size,
    Name,
    Mtime,
}

pub struct App {
    /// Root directory being explored
    pub root: PathBuf,

    /// Scanned directory tree
    pub tree: Option<DirEntry>,

    /// Flattened visible entries (for rendering)
    pub visible_entries: Vec<VisibleEntry>,

    /// Currently selected index in visible_entries
    pub selected: usize,

    /// Set of expanded directory paths
    pub expanded: std::collections::HashSet<PathBuf>,

    /// Current UI mode
    pub mode: Mode,

    /// Search/filter input
    pub search_query: String,

    /// Current sort order
    pub sort_order: SortOrder,

    /// Whether to show hidden files
    pub show_hidden: bool,

    /// Application should quit
    pub should_quit: bool,

    /// Status message to display
    pub status_message: Option<String>,

    /// Is currently scanning
    pub scanning: bool,
}

#[derive(Debug, Clone)]
pub struct VisibleEntry {
    pub entry: DirEntry,
    pub depth: usize,
    pub is_expanded: bool,
    pub is_last_sibling: bool,
}

impl App {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            tree: None,
            visible_entries: Vec::new(),
            selected: 0,
            expanded: std::collections::HashSet::new(),
            mode: Mode::Normal,
            search_query: String::new(),
            sort_order: SortOrder::Size,
            show_hidden: false,
            should_quit: false,
            status_message: None,
            scanning: false,
        }
    }

    pub fn selected_entry(&self) -> Option<&VisibleEntry> {
        self.visible_entries.get(self.selected)
    }
}
```

### Tests

```rust
#[test]
fn test_app_new() {
    let app = App::new(PathBuf::from("/home/user"));
    assert_eq!(app.root, PathBuf::from("/home/user"));
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.selected, 0);
    assert!(!app.should_quit);
}

#[test]
fn test_app_selected_entry_empty() {
    let app = App::new(PathBuf::from("/"));
    assert!(app.selected_entry().is_none());
}
```

---

## Task 4.5: Implement Tree Flattening

**Status**: `[x]`

### Description

Implement logic to convert the hierarchical `DirEntry` tree into a flat list of visible entries based on expansion state.

### Context

The TUI displays a flat list but represents a tree. We need to "flatten" the tree, including only entries whose parents are all expanded. This list is rebuilt when expand/collapse state changes.

### Implementation

`src/tui/app.rs` (add to impl App):

```rust
impl App {
    /// Rebuild the visible_entries list from the tree based on expansion state
    pub fn rebuild_visible_entries(&mut self) {
        self.visible_entries.clear();

        if let Some(ref tree) = self.tree {
            self.flatten_tree(tree, 0);
        }

        // Ensure selection is valid
        if self.selected >= self.visible_entries.len() {
            self.selected = self.visible_entries.len().saturating_sub(1);
        }
    }

    fn flatten_tree(&mut self, entry: &DirEntry, depth: usize) {
        // Apply search filter
        if !self.search_query.is_empty() {
            let name = entry.path.file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if !name.contains(&self.search_query.to_lowercase()) {
                // Check if any children match (for directories)
                if !self.has_matching_descendants(entry) {
                    return;
                }
            }
        }

        // Apply hidden filter
        if !self.show_hidden {
            if let Some(name) = entry.path.file_name() {
                if name.to_string_lossy().starts_with('.') {
                    return;
                }
            }
        }

        let is_expanded = self.expanded.contains(&entry.path);

        self.visible_entries.push(VisibleEntry {
            entry: entry.clone(),
            depth,
            is_expanded,
            is_last_sibling: false, // Updated in post-processing
        });

        // If expanded and has children, recurse
        if is_expanded {
            let mut children: Vec<_> = entry.children.iter().collect();
            self.sort_entries(&mut children);

            for child in children {
                self.flatten_tree(child, depth + 1);
            }
        }
    }

    fn has_matching_descendants(&self, entry: &DirEntry) -> bool {
        for child in &entry.children {
            let name = child.path.file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if name.contains(&self.search_query.to_lowercase()) {
                return true;
            }
            if self.has_matching_descendants(child) {
                return true;
            }
        }
        false
    }

    fn sort_entries(&self, entries: &mut [&DirEntry]) {
        match self.sort_order {
            SortOrder::Size => entries.sort_by(|a, b| b.size.cmp(&a.size)),
            SortOrder::Name => entries.sort_by(|a, b| a.path.cmp(&b.path)),
            SortOrder::Mtime => entries.sort_by(|a, b| b.mtime.cmp(&a.mtime)),
        }
    }

    /// Toggle expansion state of selected entry
    pub fn toggle_selected(&mut self) {
        if let Some(entry) = self.selected_entry() {
            let path = entry.entry.path.clone();
            if entry.entry.is_dir() {
                if self.expanded.contains(&path) {
                    self.expanded.remove(&path);
                } else {
                    self.expanded.insert(path);
                }
                self.rebuild_visible_entries();
            }
        }
    }
}
```

### Tests

```rust
#[test]
fn test_flatten_empty_tree() {
    let mut app = App::new(PathBuf::from("/"));
    app.rebuild_visible_entries();
    assert!(app.visible_entries.is_empty());
}

#[test]
fn test_flatten_single_entry() {
    let mut app = App::new(PathBuf::from("/"));
    app.tree = Some(DirEntry {
        path: PathBuf::from("/home"),
        size: 1000,
        // ... other fields
    });
    app.rebuild_visible_entries();
    assert_eq!(app.visible_entries.len(), 1);
}

#[test]
fn test_toggle_expand() {
    let mut app = App::new(PathBuf::from("/"));
    // Setup tree with children
    // ...
    app.rebuild_visible_entries();
    let initial_count = app.visible_entries.len();

    app.toggle_selected();
    app.rebuild_visible_entries();

    // Should have more entries now (children visible)
    assert!(app.visible_entries.len() > initial_count);
}
```

---

## Task 4.6: Implement Event Handling

**Status**: `[x]`

### Description

Create the event loop that reads keyboard input and dispatches to handlers.

### Context

We use `crossterm` for reading terminal events. The event loop runs on each frame, checking for input with a small timeout to allow for UI updates.

### Implementation

`src/tui/event.rs`:

```rust
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

use super::app::{App, Mode, ConfirmAction};

pub fn handle_events(app: &mut App, timeout: Duration) -> std::io::Result<bool> {
    if event::poll(timeout)? {
        if let Event::Key(key) = event::read()? {
            handle_key_event(app, key);
            return Ok(true); // Event handled
        }
    }
    Ok(false) // No event
}

fn handle_key_event(app: &mut App, key: KeyEvent) {
    // Global keys (work in any mode)
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
            return;
        }
        _ => {}
    }

    // Mode-specific handling
    match app.mode {
        Mode::Normal => handle_normal_mode(app, key),
        Mode::Search => handle_search_mode(app, key),
        Mode::Confirm(action) => handle_confirm_mode(app, key, action),
        Mode::Help => handle_help_mode(app, key),
    }
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        // Quit
        KeyCode::Char('q') | KeyCode::Esc => {
            app.should_quit = true;
        }

        // Navigation
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_selection(-1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_selection(1);
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.selected = 0;
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.selected = app.visible_entries.len().saturating_sub(1);
        }
        KeyCode::PageUp => {
            app.move_selection(-20);
        }
        KeyCode::PageDown => {
            app.move_selection(20);
        }

        // Expand/Collapse
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
            app.expand_selected();
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
            app.collapse_selected();
        }
        KeyCode::Char(' ') => {
            app.toggle_selected();
        }

        // Actions
        KeyCode::Char('d') => {
            if app.selected_entry().is_some() {
                app.mode = Mode::Confirm(ConfirmAction::Delete);
            }
        }
        KeyCode::Char('c') => {
            if app.selected_entry().is_some() {
                app.mode = Mode::Confirm(ConfirmAction::Clean);
            }
        }
        KeyCode::Char('r') => {
            app.trigger_rescan();
        }

        // Search
        KeyCode::Char('/') => {
            app.mode = Mode::Search;
            app.search_query.clear();
        }

        // Sort
        KeyCode::Char('s') => {
            app.cycle_sort_order();
        }

        // Toggle hidden
        KeyCode::Char('.') => {
            app.show_hidden = !app.show_hidden;
            app.rebuild_visible_entries();
        }

        // Help
        KeyCode::Char('?') => {
            app.mode = Mode::Help;
        }

        _ => {}
    }
}

fn handle_search_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.search_query.clear();
            app.rebuild_visible_entries();
        }
        KeyCode::Enter => {
            app.mode = Mode::Normal;
            // Keep search query active
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.rebuild_visible_entries();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.rebuild_visible_entries();
        }
        _ => {}
    }
}

fn handle_confirm_mode(app: &mut App, key: KeyEvent, action: ConfirmAction) {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            match action {
                ConfirmAction::Delete => app.delete_selected(),
                ConfirmAction::Clean => app.clean_selected(),
            }
            app.mode = Mode::Normal;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        _ => {}
    }
}

fn handle_help_mode(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
            app.mode = Mode::Normal;
        }
        _ => {}
    }
}
```

### Tests

```rust
#[test]
fn test_quit_on_q() {
    let mut app = App::new(PathBuf::from("/"));
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    handle_key_event(&mut app, key);
    assert!(app.should_quit);
}

#[test]
fn test_navigation_down() {
    let mut app = App::new(PathBuf::from("/"));
    // Setup multiple visible entries
    app.selected = 0;
    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    handle_key_event(&mut app, key);
    assert_eq!(app.selected, 1);
}

#[test]
fn test_enter_search_mode() {
    let mut app = App::new(PathBuf::from("/"));
    let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    handle_key_event(&mut app, key);
    assert_eq!(app.mode, Mode::Search);
}
```

---

## Task 4.7: Implement Navigation Methods

**Status**: `[x]`

### Description

Add methods to `App` for cursor movement and tree navigation.

### Context

Navigation must handle edge cases: empty lists, bounds checking, and the expand/collapse semantics (expand on right, collapse on left or go to parent).

### Implementation

`src/tui/app.rs` (add to impl App):

```rust
impl App {
    /// Move selection by delta, clamping to valid range
    pub fn move_selection(&mut self, delta: i32) {
        if self.visible_entries.is_empty() {
            return;
        }

        let new_selected = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            self.selected.saturating_add(delta as usize)
        };

        self.selected = new_selected.min(self.visible_entries.len() - 1);
    }

    /// Expand selected directory (or enter it if already expanded)
    pub fn expand_selected(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            if entry.entry.is_dir() {
                if !entry.is_expanded {
                    self.expanded.insert(entry.entry.path.clone());
                    self.rebuild_visible_entries();
                } else {
                    // Already expanded, move to first child
                    if self.selected + 1 < self.visible_entries.len() {
                        self.selected += 1;
                    }
                }
            }
        }
    }

    /// Collapse selected directory (or go to parent if already collapsed/file)
    pub fn collapse_selected(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            if entry.is_expanded {
                // Collapse this directory
                self.expanded.remove(&entry.entry.path);
                self.rebuild_visible_entries();
            } else {
                // Go to parent
                self.go_to_parent();
            }
        }
    }

    /// Move selection to parent directory
    pub fn go_to_parent(&mut self) {
        if let Some(entry) = self.selected_entry() {
            if let Some(parent) = entry.entry.path.parent() {
                // Find parent in visible entries
                for (i, ve) in self.visible_entries.iter().enumerate() {
                    if ve.entry.path == parent {
                        self.selected = i;
                        break;
                    }
                }
            }
        }
    }

    /// Cycle through sort orders
    pub fn cycle_sort_order(&mut self) {
        self.sort_order = match self.sort_order {
            SortOrder::Size => SortOrder::Name,
            SortOrder::Name => SortOrder::Mtime,
            SortOrder::Mtime => SortOrder::Size,
        };
        self.rebuild_visible_entries();
        self.status_message = Some(format!("Sort: {:?}", self.sort_order));
    }
}
```

### Tests

```rust
#[test]
fn test_move_selection_clamps_lower() {
    let mut app = App::new(PathBuf::from("/"));
    setup_test_entries(&mut app, 5);
    app.selected = 0;
    app.move_selection(-10);
    assert_eq!(app.selected, 0);
}

#[test]
fn test_move_selection_clamps_upper() {
    let mut app = App::new(PathBuf::from("/"));
    setup_test_entries(&mut app, 5);
    app.selected = 4;
    app.move_selection(10);
    assert_eq!(app.selected, 4);
}

#[test]
fn test_cycle_sort_order() {
    let mut app = App::new(PathBuf::from("/"));
    assert_eq!(app.sort_order, SortOrder::Size);
    app.cycle_sort_order();
    assert_eq!(app.sort_order, SortOrder::Name);
    app.cycle_sort_order();
    assert_eq!(app.sort_order, SortOrder::Mtime);
    app.cycle_sort_order();
    assert_eq!(app.sort_order, SortOrder::Size);
}
```

---

## Task 4.8: Implement Basic UI Layout

**Status**: `[x]`

### Description

Create the main UI rendering function with header, tree view area, and footer.

### Context

We use `ratatui`'s constraint-based layout system. The UI is divided into three sections: a header showing current path and disk usage, the main tree view, and a footer with keybinding hints.

### Implementation

`src/tui/ui.rs`:

```rust
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Clear},
};

use super::app::{App, Mode};

pub fn render(app: &App, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(1),     // Tree view
            Constraint::Length(2),  // Footer
        ])
        .split(frame.area());

    render_header(app, frame, chunks[0]);
    render_tree(app, frame, chunks[1]);
    render_footer(app, frame, chunks[2]);

    // Render overlays based on mode
    match app.mode {
        Mode::Search => render_search_overlay(app, frame),
        Mode::Confirm(action) => render_confirm_dialog(app, frame, action),
        Mode::Help => render_help_overlay(app, frame),
        Mode::Normal => {}
    }
}

fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let disk_usage = app.get_disk_usage_percent();
    let usage_str = format!("{}% used", disk_usage as u32);

    let title = format!(
        " Rusty Sweeper ─{:─>width$} {} ",
        "─",
        usage_str,
        width = area.width.saturating_sub(usage_str.len() as u16 + 20) as usize
    );

    let path_display = app.root.display().to_string();
    let size_display = humansize::format_size(
        app.tree.as_ref().map(|t| t.size).unwrap_or(0),
        humansize::BINARY
    );

    let header_text = format!(" {}  {}", path_display, size_display);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(header_text)
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(paragraph, area);
}

fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let hints = match app.mode {
        Mode::Normal => {
            "[↑↓] Navigate  [←→] Expand  [d] Delete  [c] Clean  [/] Search  [?] Help  [q] Quit"
        }
        Mode::Search => {
            "[Enter] Confirm  [Esc] Cancel"
        }
        Mode::Confirm(_) => {
            "[y] Yes  [n] No"
        }
        Mode::Help => {
            "[Esc] Close"
        }
    };

    // Show status message if present, otherwise hints
    let text = app.status_message.as_deref().unwrap_or(hints);

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}
```

### Tests

```rust
// UI tests are typically done via snapshot testing
// See Task 4.16 for snapshot test setup

#[test]
fn test_render_does_not_panic() {
    let app = App::new(PathBuf::from("/"));
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(&app, frame)).unwrap();
}
```

---

## Task 4.9: Implement Tree Widget

**Status**: `[x]`

### Description

Create a custom widget for rendering the directory tree with indentation, icons, and size bars.

### Context

The tree widget is the core visual component. Each entry shows: expand/collapse icon, indentation based on depth, name, size bar, and size text.

### Implementation

`src/tui/widgets/tree.rs`:

```rust
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::tui::app::{App, VisibleEntry};

pub struct TreeWidget<'a> {
    app: &'a App,
    block: Option<Block<'a>>,
}

impl<'a> TreeWidget<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app, block: None }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    fn render_entry(&self, entry: &VisibleEntry, max_size: u64, width: u16) -> ListItem<'a> {
        let indent = "  ".repeat(entry.depth);

        // Expand/collapse icon
        let icon = if entry.entry.is_dir() {
            if entry.is_expanded { "▼ " } else { "► " }
        } else {
            "  "
        };

        // Entry name
        let name = entry.entry.path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| entry.entry.path.display().to_string());

        // Size bar (10 chars wide)
        let bar_width = 10;
        let fill = if max_size > 0 {
            ((entry.entry.size as f64 / max_size as f64) * bar_width as f64) as usize
        } else {
            0
        };
        let bar = format!(
            "[{}{}]",
            "█".repeat(fill),
            "░".repeat(bar_width - fill)
        );

        // Size text
        let size_str = humansize::format_size(entry.entry.size, humansize::BINARY);

        // Calculate available width for name
        let prefix_len = indent.len() + icon.len();
        let suffix_len = bar.len() + size_str.len() + 2; // +2 for spacing
        let name_width = (width as usize).saturating_sub(prefix_len + suffix_len);

        // Truncate name if needed
        let display_name = if name.len() > name_width {
            format!("{}…", &name[..name_width.saturating_sub(1)])
        } else {
            name
        };

        // Build the line with proper spacing
        let padding = " ".repeat(name_width.saturating_sub(display_name.len()));
        let line = format!(
            "{}{}{}{} {} {}",
            indent, icon, display_name, padding, bar, size_str
        );

        // Style based on entry type
        let style = if entry.entry.is_dir() {
            Style::default().fg(Color::Blue).bold()
        } else {
            Style::default().fg(Color::White)
        };

        ListItem::new(line).style(style)
    }
}

impl<'a> StatefulWidget for TreeWidget<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let max_size = self.app.tree.as_ref().map(|t| t.size).unwrap_or(1);
        let inner_width = area.width.saturating_sub(2); // Account for borders

        let items: Vec<ListItem> = self.app.visible_entries
            .iter()
            .map(|entry| self.render_entry(entry, max_size, inner_width))
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            )
            .highlight_symbol("→ ");

        let list = if let Some(block) = self.block {
            list.block(block)
        } else {
            list
        };

        state.select(Some(self.app.selected));
        StatefulWidget::render(list, area, buf, state);
    }
}
```

### Tests

```rust
#[test]
fn test_entry_rendering_truncation() {
    // Test that long names are properly truncated
    let entry = VisibleEntry {
        entry: DirEntry {
            path: PathBuf::from("/very/long/path/name/that/should/be/truncated"),
            size: 1000,
            // ...
        },
        depth: 0,
        is_expanded: false,
        is_last_sibling: false,
    };
    // Render to test backend and verify truncation
}

#[test]
fn test_size_bar_proportional() {
    // Test that size bars scale correctly
}
```

---

## Task 4.10: Integrate Tree Widget into UI

**Status**: `[x]`

### Description

Add the tree widget rendering to the main UI render function.

### Context

The tree widget needs to be rendered with a `ListState` to track selection. We store this state in a way that persists across frames.

### Implementation

`src/tui/ui.rs` (update render_tree function):

```rust
use super::widgets::tree::TreeWidget;
use ratatui::widgets::ListState;

fn render_tree(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    let tree_widget = TreeWidget::new(app).block(block);

    let mut state = ListState::default();
    state.select(Some(app.selected));

    frame.render_stateful_widget(tree_widget, area, &mut state);
}
```

### Tests

```rust
#[test]
fn test_tree_renders_entries() {
    let mut app = App::new(PathBuf::from("/"));
    setup_test_tree(&mut app);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|frame| render(&app, frame)).unwrap();

    let buffer = terminal.backend().buffer();
    // Verify expected content in buffer
}
```

---

## Task 4.11: Implement Search Overlay

**Status**: `[x]`

### Description

Create a search input overlay that appears when in search mode.

### Context

The search overlay appears at the bottom of the screen, showing the current search query. As the user types, the tree filters in real-time.

### Implementation

`src/tui/ui.rs` (add function):

```rust
fn render_search_overlay(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Position at bottom, above footer
    let search_area = Rect {
        x: 2,
        y: area.height.saturating_sub(5),
        width: area.width.saturating_sub(4),
        height: 3,
    };

    // Clear background
    frame.render_widget(Clear, search_area);

    let block = Block::default()
        .title(" Search ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let input = format!("/{}", app.search_query);
    let cursor_pos = input.len();

    let paragraph = Paragraph::new(input)
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(paragraph, search_area);

    // Show cursor
    frame.set_cursor_position(Position::new(
        search_area.x + cursor_pos as u16 + 1,
        search_area.y + 1,
    ));
}
```

### Tests

```rust
#[test]
fn test_search_overlay_shows_query() {
    let mut app = App::new(PathBuf::from("/"));
    app.mode = Mode::Search;
    app.search_query = "test".to_string();

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|frame| render(&app, frame)).unwrap();

    // Verify "/test" appears in buffer
}
```

---

## Task 4.12: Implement Confirmation Dialog

**Status**: `[x]`

### Description

Create a modal confirmation dialog for delete and clean actions.

### Context

Before destructive actions, we show a centered dialog asking for confirmation. This prevents accidental data loss.

### Implementation

`src/tui/ui.rs` (add function):

```rust
use super::app::ConfirmAction;

fn render_confirm_dialog(app: &App, frame: &mut Frame, action: ConfirmAction) {
    let area = frame.area();

    // Center the dialog
    let dialog_width = 50;
    let dialog_height = 7;
    let dialog_area = Rect {
        x: (area.width.saturating_sub(dialog_width)) / 2,
        y: (area.height.saturating_sub(dialog_height)) / 2,
        width: dialog_width,
        height: dialog_height,
    };

    // Clear background
    frame.render_widget(Clear, dialog_area);

    let (title, message) = match action {
        ConfirmAction::Delete => {
            let path = app.selected_entry()
                .map(|e| e.entry.path.display().to_string())
                .unwrap_or_default();
            let size = app.selected_entry()
                .map(|e| humansize::format_size(e.entry.size, humansize::BINARY))
                .unwrap_or_default();
            (
                " Delete ",
                format!("Delete '{}'?\nSize: {}\n\n[y]es  [n]o", path, size)
            )
        }
        ConfirmAction::Clean => {
            let path = app.selected_entry()
                .map(|e| e.entry.path.display().to_string())
                .unwrap_or_default();
            (
                " Clean Project ",
                format!("Clean build artifacts in '{}'?\n\n[y]es  [n]o", path)
            )
        }
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let paragraph = Paragraph::new(message)
        .block(block)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, dialog_area);
}
```

### Tests

```rust
#[test]
fn test_delete_dialog_shows_path() {
    let mut app = App::new(PathBuf::from("/"));
    setup_test_tree(&mut app);
    app.mode = Mode::Confirm(ConfirmAction::Delete);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|frame| render(&app, frame)).unwrap();

    // Verify dialog appears with path name
}
```

---

## Task 4.13: Implement Help Overlay

**Status**: `[x]`

### Description

Create a help overlay showing all keybindings.

### Context

Users can press `?` to see all available keybindings. This is a full-screen overlay that dismisses on any key.

### Implementation

`src/tui/ui.rs` (add function):

```rust
fn render_help_overlay(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Near full-screen overlay
    let help_area = Rect {
        x: 4,
        y: 2,
        width: area.width.saturating_sub(8),
        height: area.height.saturating_sub(4),
    };

    frame.render_widget(Clear, help_area);

    let help_text = r#"
  NAVIGATION
  ──────────────────────────────────
  ↑/k        Move up
  ↓/j        Move down
  →/l/Enter  Expand / Enter directory
  ←/h/Bksp   Collapse / Go to parent
  Space      Toggle expand/collapse
  g          Go to top
  G          Go to bottom
  PgUp/PgDn  Page up/down

  ACTIONS
  ──────────────────────────────────
  d          Delete selected
  c          Clean project artifacts
  r          Refresh / Rescan

  VIEW
  ──────────────────────────────────
  /          Search / Filter
  s          Cycle sort (size/name/mtime)
  .          Toggle hidden files

  OTHER
  ──────────────────────────────────
  ?          Toggle this help
  q/Esc      Quit

  Press any key to close
"#;

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(paragraph, help_area);
}
```

### Tests

```rust
#[test]
fn test_help_overlay_renders() {
    let mut app = App::new(PathBuf::from("/"));
    app.mode = Mode::Help;

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|frame| render(&app, frame)).unwrap();

    // Verify help text appears
}
```

---

## Task 4.14: Implement Delete Action

**Status**: `[x]`

### Description

Implement the actual file/directory deletion when confirmed.

### Context

After user confirms deletion, we need to actually remove the file or directory. Directories are removed recursively. We update the tree after deletion.

### Implementation

`src/tui/actions.rs`:

```rust
use std::fs;
use std::path::Path;

use super::app::App;

impl App {
    pub fn delete_selected(&mut self) {
        let path = match self.selected_entry() {
            Some(entry) => entry.entry.path.clone(),
            None => return,
        };

        match delete_path(&path) {
            Ok(()) => {
                self.status_message = Some(format!("Deleted: {}", path.display()));
                self.trigger_rescan();
            }
            Err(e) => {
                self.status_message = Some(format!("Error: {}", e));
            }
        }
    }
}

fn delete_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}
```

### Tests

```rust
#[test]
fn test_delete_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    fs::write(&file_path, "test").unwrap();

    assert!(file_path.exists());
    delete_path(&file_path).unwrap();
    assert!(!file_path.exists());
}

#[test]
fn test_delete_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path().join("subdir");
    fs::create_dir(&dir_path).unwrap();
    fs::write(dir_path.join("file.txt"), "test").unwrap();

    assert!(dir_path.exists());
    delete_path(&dir_path).unwrap();
    assert!(!dir_path.exists());
}
```

---

## Task 4.15: Implement Clean Action

**Status**: `[x]`

### Description

Integrate the cleaner engine to clean project artifacts from the TUI.

### Context

When a user presses 'c' on a detected project, we run the appropriate clean command. This requires integrating with the Phase 3 cleaner module.

### Implementation

`src/tui/actions.rs` (add to impl App):

```rust
use crate::cleaner::{detect_project, clean_project};

impl App {
    pub fn clean_selected(&mut self) {
        let path = match self.selected_entry() {
            Some(entry) => entry.entry.path.clone(),
            None => return,
        };

        // Check if it's a detected project
        match detect_project(&path) {
            Some(project) => {
                self.status_message = Some(format!("Cleaning {}...", project.project_type));

                match clean_project(&project) {
                    Ok(()) => {
                        self.status_message = Some(format!(
                            "Cleaned {} project: {}",
                            project.project_type,
                            path.display()
                        ));
                        self.trigger_rescan();
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Clean failed: {}", e));
                    }
                }
            }
            None => {
                self.status_message = Some("Not a recognized project".to_string());
            }
        }
    }

    /// Check if selected entry is a cleanable project
    pub fn selected_is_project(&self) -> bool {
        self.selected_entry()
            .map(|e| detect_project(&e.entry.path).is_some())
            .unwrap_or(false)
    }
}
```

### Tests

```rust
#[test]
fn test_clean_cargo_project() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create a fake Cargo project
    fs::write(temp_dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    fs::create_dir(temp_dir.path().join("target")).unwrap();
    fs::write(temp_dir.path().join("target/debug"), "").unwrap();

    let mut app = App::new(temp_dir.path().to_path_buf());
    // Setup and select the project

    app.clean_selected();

    // target/ should be gone (or clean command executed)
}
```

---

## Task 4.16: Implement Rescan Functionality

**Status**: `[ ]`

### Description

Add the ability to rescan the current directory to refresh the view.

### Context

After delete or clean operations, or when the user presses 'r', we need to rescan to show current state. This should preserve expansion state where possible.

### Implementation

`src/tui/app.rs` (add to impl App):

```rust
use crate::scanner::scan_directory;

impl App {
    pub fn trigger_rescan(&mut self) {
        self.scanning = true;
        // In a real implementation, this would be async
        // For now, we do a blocking scan

        match scan_directory(&self.root, &Default::default()) {
            Ok(tree) => {
                // Preserve expanded paths that still exist
                let old_expanded: Vec<_> = self.expanded.iter().cloned().collect();
                self.expanded.clear();

                for path in old_expanded {
                    if path.exists() {
                        self.expanded.insert(path);
                    }
                }

                self.tree = Some(tree);
                self.rebuild_visible_entries();
            }
            Err(e) => {
                self.status_message = Some(format!("Scan error: {}", e));
            }
        }

        self.scanning = false;
    }

    pub fn initial_scan(&mut self) {
        // Expand root by default
        self.expanded.insert(self.root.clone());
        self.trigger_rescan();
    }
}
```

### Tests

```rust
#[test]
fn test_rescan_preserves_expansion() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sub_dir = temp_dir.path().join("subdir");
    fs::create_dir(&sub_dir).unwrap();

    let mut app = App::new(temp_dir.path().to_path_buf());
    app.expanded.insert(sub_dir.clone());

    app.trigger_rescan();

    assert!(app.expanded.contains(&sub_dir));
}
```

---

## Task 4.17: Implement Main TUI Loop

**Status**: `[ ]`

### Description

Create the main application loop that ties everything together.

### Context

The main loop initializes the terminal, performs initial scan, then enters the event loop rendering and handling input until quit.

### Implementation

`src/tui/mod.rs` (add function):

```rust
use std::path::PathBuf;
use std::time::Duration;

use super::app::App;
use super::ui::render;
use super::event::handle_events;

pub fn run(root: PathBuf) -> anyhow::Result<()> {
    // Setup
    install_panic_hook();
    let mut terminal = init_terminal()?;

    // Initialize app
    let mut app = App::new(root);
    app.initial_scan();

    // Main loop
    while !app.should_quit {
        // Render
        terminal.draw(|frame| render(&app, frame))?;

        // Handle events (with 100ms timeout for responsive UI)
        handle_events(&mut app, Duration::from_millis(100))?;

        // Clear transient status messages after some time
        // (In a real impl, track message timestamp)
    }

    // Cleanup
    restore_terminal()?;

    Ok(())
}
```

### Tests

```rust
// Main loop is tested via integration tests
// See Task 4.19
```

---

## Task 4.18: Wire Up CLI Subcommand

**Status**: `[ ]`

### Description

Connect the TUI module to the `tui` CLI subcommand.

### Context

The CLI already has a `tui` subcommand defined. We need to implement it to call the TUI run function.

### Implementation

`src/cli.rs` (update tui subcommand handler):

```rust
pub fn handle_tui(args: TuiArgs) -> anyhow::Result<()> {
    let path = args.path.unwrap_or_else(|| PathBuf::from("."));
    let root = path.canonicalize()?;

    crate::tui::run(root)
}
```

`src/main.rs` (in match block):

```rust
Commands::Tui(args) => cli::handle_tui(args),
```

### Tests

```bash
# Manual test
cargo run -- tui /tmp
```

---

## Task 4.19: Add Integration Tests

**Status**: `[ ]`

### Description

Create integration tests that exercise the TUI with simulated input.

### Context

Integration tests verify the TUI works correctly end-to-end. We create temporary directory structures and simulate key presses.

### Implementation

`tests/tui_integration.rs`:

```rust
use std::fs;
use tempfile::tempdir;
use rusty_sweeper::tui::{App, Mode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn test_tui_navigation() {
    let temp = tempdir().unwrap();

    // Create test structure
    fs::create_dir(temp.path().join("dir1")).unwrap();
    fs::create_dir(temp.path().join("dir2")).unwrap();
    fs::write(temp.path().join("file1.txt"), "content").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    assert!(app.visible_entries.len() >= 3);
    assert_eq!(app.selected, 0);

    // Navigate down
    rusty_sweeper::tui::event::handle_key_event(&mut app, key(KeyCode::Down));
    assert_eq!(app.selected, 1);

    // Navigate up
    rusty_sweeper::tui::event::handle_key_event(&mut app, key(KeyCode::Up));
    assert_eq!(app.selected, 0);
}

#[test]
fn test_tui_expand_collapse() {
    let temp = tempdir().unwrap();

    let subdir = temp.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("file.txt"), "content").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    let initial_count = app.visible_entries.len();

    // Expand directory
    rusty_sweeper::tui::event::handle_key_event(&mut app, key(KeyCode::Right));

    assert!(app.visible_entries.len() > initial_count);
}

#[test]
fn test_tui_search_filter() {
    let temp = tempdir().unwrap();

    fs::write(temp.path().join("apple.txt"), "").unwrap();
    fs::write(temp.path().join("banana.txt"), "").unwrap();
    fs::write(temp.path().join("cherry.txt"), "").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Enter search mode
    rusty_sweeper::tui::event::handle_key_event(&mut app, key(KeyCode::Char('/')));
    assert_eq!(app.mode, Mode::Search);

    // Type search query
    rusty_sweeper::tui::event::handle_key_event(&mut app, key(KeyCode::Char('a')));

    // Should filter to only entries containing 'a'
    let visible_names: Vec<_> = app.visible_entries
        .iter()
        .map(|e| e.entry.path.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(visible_names.iter().all(|n| n.contains('a')));
}

#[test]
fn test_tui_delete_confirmation() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("test.txt");
    fs::write(&file_path, "content").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Press delete
    rusty_sweeper::tui::event::handle_key_event(&mut app, key(KeyCode::Char('d')));

    assert!(matches!(app.mode, Mode::Confirm(_)));

    // Confirm
    rusty_sweeper::tui::event::handle_key_event(&mut app, key(KeyCode::Char('y')));

    // File should be deleted
    assert!(!file_path.exists());
}
```

### Tests

```bash
cargo test --test tui_integration
```

---

## Task 4.20: Add Snapshot Tests

**Status**: `[ ]`

### Description

Set up snapshot testing with `insta` to catch UI regressions.

### Context

Snapshot tests render the UI to a buffer and compare against saved snapshots. Any visual change requires explicit approval.

### Implementation

Add to `Cargo.toml`:

```toml
[dev-dependencies]
insta = "1.40"
```

`tests/tui_snapshots.rs`:

```rust
use insta::assert_snapshot;
use ratatui::{backend::TestBackend, Terminal};
use rusty_sweeper::tui::{App, ui::render};
use std::path::PathBuf;

fn render_to_string(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|frame| render(app, frame)).unwrap();

    let buffer = terminal.backend().buffer();
    let mut output = String::new();

    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            output.push(buffer.get(x, y).symbol().chars().next().unwrap_or(' '));
        }
        output.push('\n');
    }

    output
}

#[test]
fn test_empty_app_snapshot() {
    let app = App::new(PathBuf::from("/home/user"));
    let output = render_to_string(&app, 80, 24);
    assert_snapshot!(output);
}

#[test]
fn test_help_overlay_snapshot() {
    let mut app = App::new(PathBuf::from("/home/user"));
    app.mode = rusty_sweeper::tui::app::Mode::Help;
    let output = render_to_string(&app, 80, 30);
    assert_snapshot!(output);
}

#[test]
fn test_search_mode_snapshot() {
    let mut app = App::new(PathBuf::from("/home/user"));
    app.mode = rusty_sweeper::tui::app::Mode::Search;
    app.search_query = "test".to_string();
    let output = render_to_string(&app, 80, 24);
    assert_snapshot!(output);
}
```

### Tests

```bash
cargo test --test tui_snapshots

# Review snapshots
cargo insta review
```

---

## Task 4.21: Add Project Indicator to Tree

**Status**: `[ ]`

### Description

Show a visual indicator `[P]` for directories that are detected as cleanable projects.

### Context

This helps users quickly identify which directories have build artifacts that can be cleaned.

### Implementation

`src/tui/widgets/tree.rs` (update render_entry):

```rust
fn render_entry(&self, entry: &VisibleEntry, max_size: u64, width: u16) -> ListItem<'a> {
    // ... existing code ...

    // Check if it's a project
    let project_indicator = if entry.entry.is_dir() {
        if crate::cleaner::detect_project(&entry.entry.path).is_some() {
            "[P] "
        } else {
            "    "
        }
    } else {
        "    "
    };

    // Include in line format
    let line = format!(
        "{}{}{}{}{} {} {}",
        indent, icon, project_indicator, display_name, padding, bar, size_str
    );

    // ... rest of function ...
}
```

### Tests

```rust
#[test]
fn test_project_indicator_shown() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("Cargo.toml"), "[package]").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Render and check for [P] indicator
}
```

---

## Task 4.22: Add Disk Usage to Header

**Status**: `[ ]`

### Description

Show the actual disk usage percentage for the mount point in the header.

### Context

The header should show real disk usage from `statvfs`, not just the scanned directory size.

### Implementation

`src/tui/app.rs` (add method):

```rust
use nix::sys::statvfs::statvfs;

impl App {
    pub fn get_disk_usage_percent(&self) -> f32 {
        match statvfs(&self.root) {
            Ok(stat) => {
                let total = stat.blocks() * stat.fragment_size();
                let available = stat.blocks_available() * stat.fragment_size();
                let used = total - available;

                if total > 0 {
                    (used as f64 / total as f64 * 100.0) as f32
                } else {
                    0.0
                }
            }
            Err(_) => 0.0,
        }
    }
}
```

Add to `Cargo.toml`:

```toml
[dependencies]
nix = { version = "0.29", features = ["fs"] }
```

### Tests

```rust
#[test]
fn test_disk_usage_percent() {
    let app = App::new(PathBuf::from("/"));
    let usage = app.get_disk_usage_percent();

    assert!(usage >= 0.0);
    assert!(usage <= 100.0);
}
```

---

## Task 4.23: Add Color Coding by Size

**Status**: `[ ]`

### Description

Color-code entries based on their size (green < yellow < orange < red).

### Context

Visual color coding helps users quickly spot large entries that may be candidates for cleanup.

### Implementation

`src/tui/widgets/tree.rs` (add function and update render_entry):

```rust
fn size_color(size: u64, threshold: u64) -> Color {
    let ratio = size as f64 / threshold.max(1) as f64;

    if ratio < 0.25 {
        Color::Green
    } else if ratio < 0.5 {
        Color::Yellow
    } else if ratio < 0.75 {
        Color::Rgb(255, 165, 0) // Orange
    } else {
        Color::Red
    }
}

fn render_entry(&self, entry: &VisibleEntry, max_size: u64, width: u16) -> ListItem<'a> {
    // ... existing code ...

    // Color the size text based on relative size
    let size_style = Style::default()
        .fg(size_color(entry.entry.size, max_size));

    // Use Spans for mixed styling
    let line = Line::from(vec![
        Span::styled(format!("{}{}{}{}", indent, icon, project_indicator, display_name), style),
        Span::raw(padding),
        Span::raw(" "),
        Span::styled(bar, size_style),
        Span::raw(" "),
        Span::styled(size_str, size_style),
    ]);

    ListItem::new(line)
}
```

### Tests

```rust
#[test]
fn test_size_color_gradient() {
    let threshold = 1000;

    assert_eq!(size_color(100, threshold), Color::Green);
    assert_eq!(size_color(400, threshold), Color::Yellow);
    assert_eq!(size_color(600, threshold), Color::Rgb(255, 165, 0));
    assert_eq!(size_color(900, threshold), Color::Red);
}
```

---

## Task 4.24: Polish and Edge Cases

**Status**: `[ ]`

### Description

Handle edge cases and add polish: empty directories, permission errors, very long paths, terminal resize.

### Context

A robust TUI must handle all edge cases gracefully without crashing or displaying garbage.

### Implementation

Various locations:

```rust
// Handle permission denied
impl App {
    fn flatten_tree(&mut self, entry: &DirEntry, depth: usize) {
        // Check for permission error indicator
        if entry.permission_denied {
            // Show with [X] indicator
        }
        // ... rest of function
    }
}

// Handle terminal resize
pub fn handle_events(app: &mut App, timeout: Duration) -> std::io::Result<bool> {
    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) => handle_key_event(app, key),
            Event::Resize(_, _) => {
                // Terminal resized, just redraw (handled automatically)
            }
            _ => {}
        }
        return Ok(true);
    }
    Ok(false)
}

// Handle empty state
fn render_tree(app: &App, frame: &mut Frame, area: Rect) {
    if app.visible_entries.is_empty() {
        let message = if app.scanning {
            "Scanning..."
        } else if !app.search_query.is_empty() {
            "No matches found"
        } else {
            "Empty directory"
        };

        let paragraph = Paragraph::new(message)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));

        frame.render_widget(paragraph, area);
        return;
    }

    // ... normal rendering
}
```

### Tests

```rust
#[test]
fn test_empty_directory() {
    let temp = tempdir().unwrap();
    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Should not panic, should show empty message
}

#[test]
fn test_permission_denied_handling() {
    // Platform-specific test
}

#[test]
fn test_very_long_path() {
    let temp = tempdir().unwrap();
    let long_name = "a".repeat(200);
    fs::write(temp.path().join(&long_name), "").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Should truncate without panic
}
```

---

## Summary

| Task | Description | Status |
|------|-------------|--------|
| 4.1 | Add TUI dependencies | `[x]` |
| 4.2 | Create module structure | `[x]` |
| 4.3 | Terminal setup/teardown | `[x]` |
| 4.4 | Define application state | `[x]` |
| 4.5 | Tree flattening logic | `[x]` |
| 4.6 | Event handling | `[x]` |
| 4.7 | Navigation methods | `[x]` |
| 4.8 | Basic UI layout | `[x]` |
| 4.9 | Tree widget | `[x]` |
| 4.10 | Integrate tree widget | `[x]` |
| 4.11 | Search overlay | `[x]` |
| 4.12 | Confirmation dialog | `[x]` |
| 4.13 | Help overlay | `[x]` |
| 4.14 | Delete action | `[x]` |
| 4.15 | Clean action | `[ ]` |
| 4.16 | Rescan functionality | `[ ]` |
| 4.17 | Main TUI loop | `[ ]` |
| 4.18 | Wire up CLI | `[ ]` |
| 4.19 | Integration tests | `[ ]` |
| 4.20 | Snapshot tests | `[ ]` |
| 4.21 | Project indicator | `[ ]` |
| 4.22 | Disk usage header | `[ ]` |
| 4.23 | Size color coding | `[ ]` |
| 4.24 | Polish and edge cases | `[ ]` |

**Total: 24 tasks**
