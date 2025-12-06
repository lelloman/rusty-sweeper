//! Event handling for the TUI.

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use super::app::{App, ConfirmAction, Mode};

/// Poll for and handle events with a timeout.
///
/// Returns `Ok(true)` if an event was handled, `Ok(false)` if timeout expired.
pub fn handle_events(app: &mut App, timeout: Duration) -> std::io::Result<bool> {
    if event::poll(timeout)? {
        if let Event::Key(key) = event::read()? {
            handle_key_event(app, key);
            return Ok(true);
        }
    }
    Ok(false)
}

/// Handle a single key event.
pub fn handle_key_event(app: &mut App, key: KeyEvent) {
    // Global keys (work in any mode)
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
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
            if !app.visible_entries.is_empty() {
                app.selected = app.visible_entries.len() - 1;
            }
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
            app.status_message = Some(if app.show_hidden {
                "Showing hidden files".to_string()
            } else {
                "Hiding hidden files".to_string()
            });
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
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Enter => {
            app.mode = Mode::Normal;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_quit_on_q() {
        let mut app = App::new(PathBuf::from("/"));
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        handle_key_event(&mut app, key);
        assert!(app.should_quit);
    }

    #[test]
    fn test_quit_on_esc() {
        let mut app = App::new(PathBuf::from("/"));
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        handle_key_event(&mut app, key);
        assert!(app.should_quit);
    }

    #[test]
    fn test_quit_on_ctrl_c() {
        let mut app = App::new(PathBuf::from("/"));
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        handle_key_event(&mut app, key);
        assert!(app.should_quit);
    }

    #[test]
    fn test_enter_search_mode() {
        let mut app = App::new(PathBuf::from("/"));
        let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
        handle_key_event(&mut app, key);
        assert_eq!(app.mode, Mode::Search);
    }

    #[test]
    fn test_search_mode_typing() {
        let mut app = App::new(PathBuf::from("/"));
        app.mode = Mode::Search;

        handle_key_event(&mut app, KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        handle_key_event(&mut app, KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        handle_key_event(&mut app, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        handle_key_event(&mut app, KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));

        assert_eq!(app.search_query, "test");
    }

    #[test]
    fn test_search_mode_backspace() {
        let mut app = App::new(PathBuf::from("/"));
        app.mode = Mode::Search;
        app.search_query = "test".to_string();

        handle_key_event(&mut app, KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        assert_eq!(app.search_query, "tes");
    }

    #[test]
    fn test_search_mode_escape_clears() {
        let mut app = App::new(PathBuf::from("/"));
        app.mode = Mode::Search;
        app.search_query = "test".to_string();

        handle_key_event(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(app.mode, Mode::Normal);
        assert!(app.search_query.is_empty());
    }

    #[test]
    fn test_search_mode_enter_keeps_query() {
        let mut app = App::new(PathBuf::from("/"));
        app.mode = Mode::Search;
        app.search_query = "test".to_string();

        handle_key_event(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.search_query, "test");
    }

    #[test]
    fn test_enter_help_mode() {
        let mut app = App::new(PathBuf::from("/"));
        let key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
        handle_key_event(&mut app, key);
        assert_eq!(app.mode, Mode::Help);
    }

    #[test]
    fn test_exit_help_mode() {
        let mut app = App::new(PathBuf::from("/"));
        app.mode = Mode::Help;

        handle_key_event(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_toggle_hidden() {
        let mut app = App::new(PathBuf::from("/"));
        assert!(!app.show_hidden);

        let key = KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE);
        handle_key_event(&mut app, key);

        assert!(app.show_hidden);

        handle_key_event(&mut app, key);
        assert!(!app.show_hidden);
    }

    #[test]
    fn test_confirm_mode_yes() {
        let mut app = App::new(PathBuf::from("/"));
        app.mode = Mode::Confirm(ConfirmAction::Delete);

        handle_key_event(&mut app, KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_confirm_mode_no() {
        let mut app = App::new(PathBuf::from("/"));
        app.mode = Mode::Confirm(ConfirmAction::Delete);

        handle_key_event(&mut app, KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));

        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_ctrl_c_works_in_any_mode() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);

        // Normal mode
        let mut app = App::new(PathBuf::from("/"));
        handle_key_event(&mut app, ctrl_c);
        assert!(app.should_quit);

        // Search mode
        let mut app = App::new(PathBuf::from("/"));
        app.mode = Mode::Search;
        handle_key_event(&mut app, ctrl_c);
        assert!(app.should_quit);

        // Help mode
        let mut app = App::new(PathBuf::from("/"));
        app.mode = Mode::Help;
        handle_key_event(&mut app, ctrl_c);
        assert!(app.should_quit);

        // Confirm mode
        let mut app = App::new(PathBuf::from("/"));
        app.mode = Mode::Confirm(ConfirmAction::Delete);
        handle_key_event(&mut app, ctrl_c);
        assert!(app.should_quit);
    }
}
