//! Snapshot tests for TUI rendering.
//!
//! These tests render the UI to a test buffer and compare against saved snapshots.
//! Any visual change requires explicit approval with `cargo insta review`.

use insta::assert_snapshot;
use ratatui::{backend::TestBackend, Terminal};
use rusty_sweeper::scanner::DirEntry;
use rusty_sweeper::tui::app::{ConfirmAction, Mode};
use rusty_sweeper::tui::ui::render;
use rusty_sweeper::tui::App;
use std::path::PathBuf;

/// Render the app to a string for snapshot comparison.
fn render_to_string(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|frame| render(app, frame)).unwrap();

    let buffer = terminal.backend().buffer();
    let mut output = String::new();

    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            output.push_str(cell.symbol());
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
fn test_scanning_state_snapshot() {
    let mut app = App::new(PathBuf::from("/home/user"));
    app.scanning = true;
    let output = render_to_string(&app, 80, 24);
    assert_snapshot!(output);
}

#[test]
fn test_with_tree_snapshot() {
    let mut app = App::new(PathBuf::from("/test"));

    // Create a simple tree
    let mut root = DirEntry::new_dir(PathBuf::from("/test"), None);
    root.children.push(DirEntry::new_file(
        PathBuf::from("/test/file1.txt"),
        1024,
        4096,
        None,
    ));
    root.children.push(DirEntry::new_file(
        PathBuf::from("/test/file2.txt"),
        2048,
        4096,
        None,
    ));
    root.recalculate_totals();

    app.tree = Some(root);
    app.expanded.insert(PathBuf::from("/test"));
    app.rebuild_visible_entries();

    let output = render_to_string(&app, 80, 24);
    assert_snapshot!(output);
}

#[test]
fn test_help_overlay_snapshot() {
    let mut app = App::new(PathBuf::from("/home/user"));
    app.mode = Mode::Help;
    let output = render_to_string(&app, 80, 30);
    assert_snapshot!(output);
}

#[test]
fn test_search_mode_snapshot() {
    let mut app = App::new(PathBuf::from("/home/user"));
    app.mode = Mode::Search;
    app.search_query = "test".to_string();
    let output = render_to_string(&app, 80, 24);
    assert_snapshot!(output);
}

#[test]
fn test_confirm_delete_snapshot() {
    let mut app = App::new(PathBuf::from("/test"));

    let mut root = DirEntry::new_dir(PathBuf::from("/test"), None);
    root.children.push(DirEntry::new_file(
        PathBuf::from("/test/important.txt"),
        1024000,
        4096,
        None,
    ));
    root.recalculate_totals();

    app.tree = Some(root);
    app.expanded.insert(PathBuf::from("/test"));
    app.rebuild_visible_entries();
    app.selected = 1; // Select the file
    app.mode = Mode::Confirm(ConfirmAction::Delete);

    let output = render_to_string(&app, 80, 24);
    assert_snapshot!(output);
}

#[test]
fn test_confirm_clean_snapshot() {
    let mut app = App::new(PathBuf::from("/project"));

    let mut root = DirEntry::new_dir(PathBuf::from("/project"), None);
    root.children.push(DirEntry::new_dir(
        PathBuf::from("/project/target"),
        None,
    ));
    root.recalculate_totals();

    app.tree = Some(root);
    app.expanded.insert(PathBuf::from("/project"));
    app.rebuild_visible_entries();
    app.mode = Mode::Confirm(ConfirmAction::Clean);

    let output = render_to_string(&app, 80, 24);
    assert_snapshot!(output);
}

#[test]
fn test_status_message_snapshot() {
    let mut app = App::new(PathBuf::from("/test"));
    app.status_message = Some("Operation completed successfully".to_string());
    let output = render_to_string(&app, 80, 24);
    assert_snapshot!(output);
}

#[test]
fn test_nested_tree_snapshot() {
    let mut app = App::new(PathBuf::from("/root"));

    let mut root = DirEntry::new_dir(PathBuf::from("/root"), None);

    let mut subdir = DirEntry::new_dir(PathBuf::from("/root/subdir"), None);
    subdir.children.push(DirEntry::new_file(
        PathBuf::from("/root/subdir/nested.txt"),
        512,
        4096,
        None,
    ));
    subdir.recalculate_totals();

    root.children.push(subdir);
    root.children.push(DirEntry::new_file(
        PathBuf::from("/root/top.txt"),
        256,
        4096,
        None,
    ));
    root.recalculate_totals();

    app.tree = Some(root);
    app.expanded.insert(PathBuf::from("/root"));
    app.expanded.insert(PathBuf::from("/root/subdir"));
    app.rebuild_visible_entries();

    let output = render_to_string(&app, 80, 24);
    assert_snapshot!(output);
}
