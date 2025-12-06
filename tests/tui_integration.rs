//! Integration tests for the TUI module.
//!
//! These tests exercise the TUI with simulated input, verifying correct
//! behavior without requiring an actual terminal.

use std::fs;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rusty_sweeper::tui::app::{ConfirmAction, Mode};
use rusty_sweeper::tui::event::handle_key_event;
use rusty_sweeper::tui::App;
use tempfile::tempdir;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn key_char(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

#[test]
fn test_tui_initial_scan() {
    let temp = tempdir().unwrap();

    fs::create_dir(temp.path().join("dir1")).unwrap();
    fs::create_dir(temp.path().join("dir2")).unwrap();
    fs::write(temp.path().join("file1.txt"), "content").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Root should be expanded after initial scan
    assert!(app.expanded.contains(&temp.path().to_path_buf()));

    // Should have visible entries (root + children)
    assert!(app.visible_entries.len() >= 4); // root + dir1 + dir2 + file1
}

#[test]
fn test_tui_navigation_down() {
    let temp = tempdir().unwrap();

    fs::create_dir(temp.path().join("dir1")).unwrap();
    fs::create_dir(temp.path().join("dir2")).unwrap();
    fs::write(temp.path().join("file1.txt"), "content").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    assert_eq!(app.selected, 0);

    // Navigate down
    handle_key_event(&mut app, key(KeyCode::Down));
    assert_eq!(app.selected, 1);

    // Navigate down again
    handle_key_event(&mut app, key(KeyCode::Down));
    assert_eq!(app.selected, 2);
}

#[test]
fn test_tui_navigation_up() {
    let temp = tempdir().unwrap();

    fs::create_dir(temp.path().join("dir1")).unwrap();
    fs::write(temp.path().join("file1.txt"), "content").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();
    app.selected = 2;

    // Navigate up
    handle_key_event(&mut app, key(KeyCode::Up));
    assert_eq!(app.selected, 1);

    // Navigate up again
    handle_key_event(&mut app, key(KeyCode::Up));
    assert_eq!(app.selected, 0);
}

#[test]
fn test_tui_navigation_vim_keys() {
    let temp = tempdir().unwrap();

    fs::create_dir(temp.path().join("dir1")).unwrap();
    fs::write(temp.path().join("file1.txt"), "content").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Navigate with j (down)
    handle_key_event(&mut app, key_char('j'));
    assert_eq!(app.selected, 1);

    // Navigate with k (up)
    handle_key_event(&mut app, key_char('k'));
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

    // Navigate to subdir (index 1 after root is expanded)
    handle_key_event(&mut app, key(KeyCode::Down));

    let initial_count = app.visible_entries.len();

    // Expand directory with Right arrow
    handle_key_event(&mut app, key(KeyCode::Right));

    // Should now have more entries (subdir's children are visible)
    assert!(app.visible_entries.len() > initial_count);

    // Collapse with Left arrow
    handle_key_event(&mut app, key(KeyCode::Left));

    assert_eq!(app.visible_entries.len(), initial_count);
}

#[test]
fn test_tui_toggle_expand() {
    let temp = tempdir().unwrap();

    let subdir = temp.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("file.txt"), "content").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Navigate to subdir
    handle_key_event(&mut app, key(KeyCode::Down));

    let initial_count = app.visible_entries.len();

    // Toggle expand with Space
    handle_key_event(&mut app, key(KeyCode::Char(' ')));
    assert!(app.visible_entries.len() > initial_count);

    // Toggle collapse with Space
    handle_key_event(&mut app, key(KeyCode::Char(' ')));
    assert_eq!(app.visible_entries.len(), initial_count);
}

#[test]
fn test_tui_search_mode() {
    let temp = tempdir().unwrap();

    fs::write(temp.path().join("apple.txt"), "").unwrap();
    fs::write(temp.path().join("banana.txt"), "").unwrap();
    fs::write(temp.path().join("cherry.txt"), "").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Enter search mode with /
    handle_key_event(&mut app, key_char('/'));
    assert_eq!(app.mode, Mode::Search);

    // Type search query
    handle_key_event(&mut app, key_char('a'));
    handle_key_event(&mut app, key_char('p'));
    handle_key_event(&mut app, key_char('p'));
    assert_eq!(app.search_query, "app");

    // Only entries containing 'app' should be visible
    let visible_names: Vec<_> = app
        .visible_entries
        .iter()
        .filter(|e| e.depth > 0) // Exclude root
        .map(|e| e.entry.name.clone())
        .collect();

    assert!(visible_names.contains(&"apple.txt".to_string()));
    assert!(!visible_names.contains(&"banana.txt".to_string()));
    assert!(!visible_names.contains(&"cherry.txt".to_string()));
}

#[test]
fn test_tui_search_cancel() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("test.txt"), "").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Enter search mode
    handle_key_event(&mut app, key_char('/'));
    handle_key_event(&mut app, key_char('t'));
    assert!(!app.search_query.is_empty());

    // Cancel with Escape
    handle_key_event(&mut app, key(KeyCode::Esc));
    assert_eq!(app.mode, Mode::Normal);
    assert!(app.search_query.is_empty());
}

#[test]
fn test_tui_search_confirm() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("test.txt"), "").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Enter search mode and type
    handle_key_event(&mut app, key_char('/'));
    handle_key_event(&mut app, key_char('t'));

    // Confirm with Enter
    handle_key_event(&mut app, key(KeyCode::Enter));
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.search_query, "t"); // Query preserved
}

#[test]
fn test_tui_delete_confirmation() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("test.txt");
    fs::write(&file_path, "content").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Navigate to file (after root)
    handle_key_event(&mut app, key(KeyCode::Down));

    // Press delete
    handle_key_event(&mut app, key_char('d'));

    assert!(matches!(app.mode, Mode::Confirm(ConfirmAction::Delete)));

    // Confirm with y
    handle_key_event(&mut app, key_char('y'));

    // File should be deleted
    assert!(!file_path.exists());
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn test_tui_delete_cancel() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("test.txt");
    fs::write(&file_path, "content").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Navigate to file
    handle_key_event(&mut app, key(KeyCode::Down));

    // Press delete
    handle_key_event(&mut app, key_char('d'));

    // Cancel with n
    handle_key_event(&mut app, key_char('n'));

    // File should still exist
    assert!(file_path.exists());
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn test_tui_help_mode() {
    let mut app = App::new(PathBuf::from("/"));

    // Enter help mode with ?
    handle_key_event(&mut app, key_char('?'));
    assert_eq!(app.mode, Mode::Help);

    // Exit help mode with Escape
    handle_key_event(&mut app, key(KeyCode::Esc));
    assert_eq!(app.mode, Mode::Normal);
}

#[test]
fn test_tui_quit() {
    let mut app = App::new(PathBuf::from("/"));

    assert!(!app.should_quit);

    // Quit with q
    handle_key_event(&mut app, key_char('q'));
    assert!(app.should_quit);
}

#[test]
fn test_tui_quit_ctrl_c() {
    let mut app = App::new(PathBuf::from("/"));

    assert!(!app.should_quit);

    // Quit with Ctrl+C
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    handle_key_event(&mut app, ctrl_c);
    assert!(app.should_quit);
}

#[test]
fn test_tui_sort_order_cycling() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("test.txt"), "").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Default is Size
    assert_eq!(
        app.sort_order,
        rusty_sweeper::tui::app::SortOrder::Size
    );

    // Cycle with s
    handle_key_event(&mut app, key_char('s'));
    assert_eq!(
        app.sort_order,
        rusty_sweeper::tui::app::SortOrder::Name
    );

    handle_key_event(&mut app, key_char('s'));
    assert_eq!(
        app.sort_order,
        rusty_sweeper::tui::app::SortOrder::Mtime
    );

    handle_key_event(&mut app, key_char('s'));
    assert_eq!(
        app.sort_order,
        rusty_sweeper::tui::app::SortOrder::Size
    );
}

#[test]
fn test_tui_toggle_hidden() {
    let temp = tempdir().unwrap();
    let hidden_path = temp.path().join(".hidden");
    fs::create_dir(&hidden_path).unwrap();
    fs::create_dir(temp.path().join("visible")).unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Initially hidden files are not shown
    assert!(!app.show_hidden);
    let has_hidden = app
        .visible_entries
        .iter()
        .any(|e| e.entry.path == hidden_path);
    assert!(!has_hidden, "Hidden dir should not be visible initially");

    // Toggle with .
    handle_key_event(&mut app, key_char('.'));
    assert!(app.show_hidden);

    // Need to verify status message was set
    assert!(app.status_message.is_some());
    assert!(app
        .status_message
        .as_ref()
        .unwrap()
        .contains("Showing hidden"));

    // After toggling, hidden should be visible
    let has_hidden = app
        .visible_entries
        .iter()
        .any(|e| e.entry.path == hidden_path);
    assert!(has_hidden, "Hidden dir should be visible after toggle. Entries: {:?}",
        app.visible_entries.iter().map(|e| &e.entry.path).collect::<Vec<_>>());

    // Toggle again
    handle_key_event(&mut app, key_char('.'));
    assert!(!app.show_hidden);
}

#[test]
fn test_tui_go_to_top_bottom() {
    let temp = tempdir().unwrap();
    fs::create_dir(temp.path().join("dir1")).unwrap();
    fs::create_dir(temp.path().join("dir2")).unwrap();
    fs::create_dir(temp.path().join("dir3")).unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Go to bottom with G
    handle_key_event(&mut app, key_char('G'));
    assert_eq!(app.selected, app.visible_entries.len() - 1);

    // Go to top with g
    handle_key_event(&mut app, key_char('g'));
    assert_eq!(app.selected, 0);
}

#[test]
fn test_tui_rescan() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("test.txt");
    fs::write(&file_path, "content").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    let initial_count = app.visible_entries.len();

    // Create a new file outside TUI
    fs::write(temp.path().join("new_file.txt"), "content").unwrap();

    // Press r to start background rescan
    handle_key_event(&mut app, key_char('r'));

    // Wait for background scan to complete (with timeout)
    let start = std::time::Instant::now();
    while app.scanning && start.elapsed() < std::time::Duration::from_secs(5) {
        app.poll_scan_result();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Should now see the new file
    assert!(app.visible_entries.len() > initial_count);
}
