//! UI rendering for the TUI.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::app::{App, ConfirmAction, Mode};

/// Render the entire UI.
pub fn render(app: &App, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(1),    // Tree view
            Constraint::Length(2), // Footer
        ])
        .split(frame.area());

    render_header(app, frame, chunks[0]);
    render_tree_area(app, frame, chunks[1]);
    render_footer(app, frame, chunks[2]);

    // Render overlays based on mode
    match app.mode {
        Mode::Search => render_search_overlay(app, frame),
        Mode::Confirm(action) => render_confirm_dialog(app, frame, action),
        Mode::Help => render_help_overlay(frame),
        Mode::Normal => {}
    }
}

fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let path_display = app.root.display().to_string();
    let size_display = app
        .tree
        .as_ref()
        .map(|t| humansize::format_size(t.size, humansize::BINARY))
        .unwrap_or_else(|| "...".to_string());

    let header_text = format!(" {}  {}", path_display, size_display);

    let title = " Rusty Sweeper ";

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(header_text)
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(paragraph, area);
}

fn render_tree_area(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    if app.visible_entries.is_empty() {
        let message = if app.scanning {
            "Scanning..."
        } else if !app.search_query.is_empty() {
            "No matches found"
        } else if app.tree.is_none() {
            "No data"
        } else {
            "Empty directory"
        };

        let paragraph = Paragraph::new(message)
            .block(block)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));

        frame.render_widget(paragraph, area);
        return;
    }

    // Render tree entries
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let max_size = app.tree.as_ref().map(|t| t.size).unwrap_or(1);
    let visible_height = inner_area.height as usize;

    // Calculate scroll offset to keep selection visible
    let scroll_offset = calculate_scroll_offset(app.selected, visible_height, app.visible_entries.len());

    for (i, visible_entry) in app
        .visible_entries
        .iter()
        .skip(scroll_offset)
        .take(visible_height)
        .enumerate()
    {
        let y = inner_area.y + i as u16;
        let is_selected = scroll_offset + i == app.selected;

        render_entry(frame, visible_entry, inner_area.x, y, inner_area.width, max_size, is_selected);
    }
}

fn calculate_scroll_offset(selected: usize, visible_height: usize, total: usize) -> usize {
    if total <= visible_height {
        return 0;
    }

    let padding = 3.min(visible_height / 4);

    if selected < padding {
        0
    } else if selected >= total - padding {
        total.saturating_sub(visible_height)
    } else {
        selected.saturating_sub(padding)
    }
}

fn render_entry(
    frame: &mut Frame,
    entry: &super::app::VisibleEntry,
    x: u16,
    y: u16,
    width: u16,
    max_size: u64,
    is_selected: bool,
) {
    let indent = "  ".repeat(entry.depth);

    // Expand/collapse icon
    let icon = if entry.entry.is_dir {
        if entry.is_expanded {
            "▼ "
        } else {
            "► "
        }
    } else {
        "  "
    };

    // Size bar (10 chars wide)
    let bar_width = 10;
    let fill = if max_size > 0 {
        ((entry.entry.size as f64 / max_size as f64) * bar_width as f64) as usize
    } else {
        0
    };
    let bar = format!("[{}{}]", "█".repeat(fill), "░".repeat(bar_width - fill));

    // Size text
    let size_str = humansize::format_size(entry.entry.size, humansize::BINARY);

    // Project type indicator (e.g., "[Rust]")
    let project_indicator = entry
        .project_type
        .as_ref()
        .map(|t| format!(" [{}]", t))
        .unwrap_or_default();

    // Calculate available width for name
    let prefix_len = indent.len() + icon.len();
    let suffix_len = bar.len() + size_str.len() + 2 + project_indicator.len();
    let name_width = (width as usize).saturating_sub(prefix_len + suffix_len);

    // Truncate name if needed
    let name = &entry.entry.name;
    let display_name = if name.len() > name_width && name_width > 1 {
        format!("{}…", &name[..name_width.saturating_sub(1)])
    } else {
        name.clone()
    };

    // Build the line
    let padding = " ".repeat(name_width.saturating_sub(display_name.len()));
    let line = format!("{}{}{}{}{} {} {}", indent, icon, display_name, project_indicator, padding, bar, size_str);

    // Truncate to fit width
    let line: String = line.chars().take(width as usize).collect();

    // Style
    let mut style = if entry.entry.is_dir {
        Style::default().fg(Color::Blue).bold()
    } else {
        Style::default().fg(Color::White)
    };

    if is_selected {
        style = style.bg(Color::DarkGray);
    }

    let span = Span::styled(line, style);
    let area = Rect::new(x, y, width, 1);
    frame.render_widget(Paragraph::new(span), area);
}

fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let hints = match app.mode {
        Mode::Normal => {
            "[↑↓] Navigate  [←→] Expand  [d] Delete  [c] Clean  [/] Search  [?] Help  [q] Quit"
        }
        Mode::Search => "[Enter] Confirm  [Esc] Cancel",
        Mode::Confirm(_) => "[y] Yes  [n] No",
        Mode::Help => "[Esc] Close",
    };

    // Show status message if present, otherwise hints
    let text = app.status_message.as_deref().unwrap_or(hints);

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}

fn render_search_overlay(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Position at bottom, above footer
    let search_area = Rect {
        x: 2,
        y: area.height.saturating_sub(6),
        width: area.width.saturating_sub(4).min(60),
        height: 3,
    };

    // Clear background
    frame.render_widget(Clear, search_area);

    let block = Block::default()
        .title(" Search ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let input = format!("/{}", app.search_query);

    let paragraph = Paragraph::new(input)
        .block(block)
        .style(Style::default().fg(Color::White));

    frame.render_widget(paragraph, search_area);

    // Show cursor position
    frame.set_cursor_position(Position::new(
        search_area.x + app.search_query.len() as u16 + 2, // +2 for border and /
        search_area.y + 1,
    ));
}

fn render_confirm_dialog(app: &App, frame: &mut Frame, action: ConfirmAction) {
    let area = frame.area();

    // Center the dialog
    let dialog_width = 50u16.min(area.width.saturating_sub(4));
    let dialog_height = 7u16;
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
            let path = app
                .selected_entry()
                .map(|e| {
                    let p = e.entry.path.display().to_string();
                    if p.len() > 35 {
                        format!("...{}", &p[p.len() - 32..])
                    } else {
                        p
                    }
                })
                .unwrap_or_default();
            let size = app
                .selected_entry()
                .map(|e| humansize::format_size(e.entry.size, humansize::BINARY))
                .unwrap_or_default();
            (
                " Delete ",
                format!("Delete '{}'?\n\nSize: {}\n\n[y]es  [n]o", path, size),
            )
        }
        ConfirmAction::Clean => {
            let path = app
                .selected_entry()
                .map(|e| {
                    let p = e.entry.path.display().to_string();
                    if p.len() > 35 {
                        format!("...{}", &p[p.len() - 32..])
                    } else {
                        p
                    }
                })
                .unwrap_or_default();
            (
                " Clean Project ",
                format!("Clean build artifacts in\n'{}'?\n\n[y]es  [n]o", path),
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

fn render_help_overlay(frame: &mut Frame) {
    let area = frame.area();

    // Near full-screen overlay
    let help_width = 60u16.min(area.width.saturating_sub(8));
    let help_height = 22u16.min(area.height.saturating_sub(4));
    let help_area = Rect {
        x: (area.width.saturating_sub(help_width)) / 2,
        y: (area.height.saturating_sub(help_height)) / 2,
        width: help_width,
        height: help_height,
    };

    frame.render_widget(Clear, help_area);

    let help_text = r#"
 NAVIGATION
 ─────────────────────────────────
 ↑/k        Move up
 ↓/j        Move down
 →/l/Enter  Expand directory
 ←/h/Bksp   Collapse / Go to parent
 Space      Toggle expand/collapse
 g          Go to top
 G          Go to bottom

 ACTIONS
 ─────────────────────────────────
 d          Delete selected
 c          Clean project artifacts
 r          Refresh / Rescan

 VIEW
 ─────────────────────────────────
 /          Search / Filter
 s          Cycle sort order
 .          Toggle hidden files
 ?          Toggle this help
 q/Esc      Quit
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    #[test]
    fn test_render_does_not_panic() {
        let app = App::new(PathBuf::from("/"));
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
    }

    #[test]
    fn test_render_with_tree() {
        use crate::scanner::DirEntry;

        let mut app = App::new(PathBuf::from("/test"));
        app.tree = Some(DirEntry::new_dir(PathBuf::from("/test"), None));
        app.rebuild_visible_entries();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
    }

    #[test]
    fn test_calculate_scroll_offset() {
        // No scroll needed when list fits in view
        assert_eq!(calculate_scroll_offset(0, 20, 10), 0);
        assert_eq!(calculate_scroll_offset(5, 20, 10), 0);

        // At the top
        assert_eq!(calculate_scroll_offset(0, 10, 100), 0);
        assert_eq!(calculate_scroll_offset(2, 10, 100), 0);

        // In the middle - selected item minus padding
        let offset = calculate_scroll_offset(50, 10, 100);
        // Should keep selected visible with some padding
        assert!(offset <= 50);
        assert!(offset + 10 > 50);

        // At the bottom
        let offset = calculate_scroll_offset(99, 10, 100);
        assert_eq!(offset, 90); // 100 - 10 = 90
    }

    #[test]
    fn test_render_search_overlay() {
        let mut app = App::new(PathBuf::from("/"));
        app.mode = Mode::Search;
        app.search_query = "test".to_string();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
    }

    #[test]
    fn test_render_confirm_dialog() {
        use crate::scanner::DirEntry;

        let mut app = App::new(PathBuf::from("/test"));
        app.tree = Some(DirEntry::new_dir(PathBuf::from("/test"), None));
        app.rebuild_visible_entries();
        app.mode = Mode::Confirm(ConfirmAction::Delete);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
    }

    #[test]
    fn test_render_help_overlay() {
        let mut app = App::new(PathBuf::from("/"));
        app.mode = Mode::Help;

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(&app, frame)).unwrap();
    }
}
