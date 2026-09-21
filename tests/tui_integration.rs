//! Integration tests for the TUI module.
//!
//! These tests exercise the TUI with simulated input, verifying correct
//! behavior without requiring an actual terminal.

use std::fs;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rusty_sweeper::config::CleanerConfig;
use rusty_sweeper::scanner::ScanOptions;
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

fn wait_for_background_work(app: &mut App) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while (app.operation_kind().is_some() || app.scanning || app.recursive_preview_running)
        && std::time::Instant::now() < deadline
    {
        app.poll_background_work();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        app.operation_kind().is_none(),
        "background operation timed out"
    );
    assert!(!app.scanning, "background rescan timed out");
}

fn app_for_cleaning(root: PathBuf, project_types: &[&str]) -> App {
    let mut cleaner = CleanerConfig::default();
    cleaner.min_age_days = 0;
    cleaner.project_types = project_types
        .iter()
        .map(|kind| (*kind).to_string())
        .collect();
    App::with_options(
        root,
        ScanOptions::default().with_one_file_system(true),
        cleaner,
    )
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
    wait_for_background_work(&mut app);

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
fn test_tui_delete_confirmation_pins_original_target() {
    let temp = tempdir().unwrap();
    let first_path = temp.path().join("first.txt");
    let second_path = temp.path().join("second.txt");
    fs::write(&first_path, "first file is larger").unwrap();
    fs::write(&second_path, "x").unwrap();

    let mut app = App::new(temp.path().to_path_buf());
    app.initial_scan();

    // Size sorting puts first.txt before second.txt.
    app.selected = 1;
    handle_key_event(&mut app, key_char('d'));
    assert!(matches!(app.mode, Mode::Confirm(ConfirmAction::Delete)));

    // Simulate a progressive scan rebuild changing which row the numeric
    // selection points at while the dialog remains open.
    app.selected = 2;
    handle_key_event(&mut app, key_char('y'));
    wait_for_background_work(&mut app);

    assert!(
        !first_path.exists(),
        "the originally confirmed file must be deleted"
    );
    assert!(
        second_path.exists(),
        "a newly selected file must not be deleted"
    );
}

#[test]
fn test_tui_clean_confirmation_pins_original_target() {
    let temp = tempdir().unwrap();
    let first_project = temp.path().join("first-project");
    let second_project = temp.path().join("second-project");
    fs::create_dir_all(first_project.join("target")).unwrap();
    fs::create_dir_all(second_project.join("target")).unwrap();
    fs::write(first_project.join("Cargo.toml"), "[package]").unwrap();
    fs::write(second_project.join("Cargo.toml"), "[package]").unwrap();
    fs::write(first_project.join("target/artifact"), "x".repeat(20)).unwrap();
    fs::write(second_project.join("target/artifact"), "x").unwrap();

    let mut app = app_for_cleaning(temp.path().to_path_buf(), &["cargo"]);
    app.initial_scan();

    app.selected = 1;
    handle_key_event(&mut app, key_char('c'));
    assert!(matches!(app.mode, Mode::Confirm(ConfirmAction::Clean)));

    app.selected = 2;
    handle_key_event(&mut app, key_char('y'));
    wait_for_background_work(&mut app);

    assert!(!first_project.join("target").exists());
    assert!(second_project.join("target").exists());
}

#[test]
fn test_tui_partial_clean_reports_freed_space_and_refreshes_tree() {
    let temp = tempdir().unwrap();
    let build = temp.path().join("build");
    let invalid_artifact = temp.path().join(".gradle");
    fs::write(temp.path().join("build.gradle"), "").unwrap();
    fs::create_dir(&build).unwrap();
    fs::write(build.join("artifact.bin"), "x".repeat(128)).unwrap();
    // The detector currently accepts any existing artifact path. A regular
    // file forces remove_dir_all to fail after build/ has been removed.
    fs::write(&invalid_artifact, "not a directory").unwrap();

    let mut app = app_for_cleaning(temp.path().to_path_buf(), &["gradle"]);
    app.initial_scan();
    assert!(app
        .visible_entries
        .iter()
        .any(|entry| entry.entry.path == build));

    app.clean_selected();
    wait_for_background_work(&mut app);

    assert!(!build.exists());
    assert!(
        !app.visible_entries
            .iter()
            .any(|entry| entry.entry.path == build),
        "the tree must refresh after a partially successful cleanup"
    );
    let status = app.status_message.as_deref().unwrap_or_default();
    assert!(status.contains("partially cleaned"));
    assert!(status.contains("128 B"));
}

#[test]
fn test_tui_recursive_clean_selected_directory() {
    let temp = tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let first = workspace.join("first-app");
    let second = workspace.join("second-app");
    let outside = temp.path().join("outside-app");
    for project in [&first, &second, &outside] {
        fs::create_dir_all(project.join("node_modules")).unwrap();
        fs::write(project.join("package.json"), "{}").unwrap();
        fs::write(project.join("node_modules/dependency.js"), "x".repeat(32)).unwrap();
    }

    let mut app = app_for_cleaning(temp.path().to_path_buf(), &["npm"]);
    app.initial_scan();
    app.selected = app
        .visible_entries
        .iter()
        .position(|entry| entry.entry.path == workspace)
        .unwrap();

    handle_key_event(&mut app, key_char('C'));
    wait_for_background_work(&mut app);

    assert!(matches!(
        app.mode,
        Mode::Confirm(ConfirmAction::CleanRecursive)
    ));
    let preview = app.clean_preview.as_ref().unwrap();
    assert_eq!(preview.projects.len(), 2);
    assert_eq!(preview.scope_path, Some(workspace.clone()));

    handle_key_event(&mut app, key_char('y'));
    wait_for_background_work(&mut app);

    assert!(!first.join("node_modules").exists());
    assert!(!second.join("node_modules").exists());
    assert!(outside.join("node_modules").exists());
    assert!(app
        .status_message
        .as_deref()
        .unwrap_or_default()
        .contains("Cleaned 2 projects"));
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

#[cfg(unix)]
#[test]
fn test_tui_recursive_clean_continues_after_permission_failure() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().unwrap();
    let first = temp.path().join("first-app");
    let second = temp.path().join("second-app");
    for project in [&first, &second] {
        fs::create_dir_all(project.join("node_modules")).unwrap();
        fs::write(project.join("package.json"), "{}").unwrap();
        fs::write(project.join("node_modules/dependency.js"), "x".repeat(32)).unwrap();
    }
    let blocked = first.join("node_modules/blocked");
    fs::create_dir(&blocked).unwrap();
    fs::write(blocked.join("keep"), "protected").unwrap();
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o555)).unwrap();

    let mut app = app_for_cleaning(temp.path().to_path_buf(), &["npm"]);
    app.initial_scan();
    handle_key_event(&mut app, key_char('C'));
    wait_for_background_work(&mut app);
    assert!(matches!(
        app.mode,
        Mode::Confirm(ConfirmAction::CleanRecursive)
    ));
    handle_key_event(&mut app, key_char('y'));
    wait_for_background_work(&mut app);
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(blocked.join("keep").exists());
    assert!(!first.join("node_modules/dependency.js").exists());
    assert!(!second.join("node_modules").exists());
    let status = app.status_message.as_deref().unwrap_or_default();
    assert!(status.contains("Cleaned 1 projects; 1 failed"), "{status}");
    assert!(status.contains("64 B"), "{status}");
    assert_eq!(app.mode, Mode::Normal);
    assert!(!app.should_quit);
    handle_key_event(&mut app, key_char('?'));
    assert_eq!(app.mode, Mode::Help);
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
    assert_eq!(app.sort_order, rusty_sweeper::tui::app::SortOrder::Size);

    // Cycle with s
    handle_key_event(&mut app, key_char('s'));
    assert_eq!(app.sort_order, rusty_sweeper::tui::app::SortOrder::Name);

    handle_key_event(&mut app, key_char('s'));
    assert_eq!(app.sort_order, rusty_sweeper::tui::app::SortOrder::Mtime);

    handle_key_event(&mut app, key_char('s'));
    assert_eq!(app.sort_order, rusty_sweeper::tui::app::SortOrder::Size);
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
    assert!(
        has_hidden,
        "Hidden dir should be visible after toggle. Entries: {:?}",
        app.visible_entries
            .iter()
            .map(|e| &e.entry.path)
            .collect::<Vec<_>>()
    );

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
