//! UI rendering for the TUI.

use ratatui::{
    prelude::*,
    text::Line,
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

    // Get disk usage info
    let disk_info = app.get_disk_usage().map(|(total, used, avail)| {
        let total_str = humansize::format_size(total, humansize::BINARY);
        let used_str = humansize::format_size(used, humansize::BINARY);
        let avail_str = humansize::format_size(avail, humansize::BINARY);
        let percent = if total > 0 {
            (used as f64 / total as f64 * 100.0) as u32
        } else {
            0
        };
        format!("Disk: {} / {} ({}% used, {} free)", used_str, total_str, percent, avail_str)
    });

    let header_text = match disk_info {
        Some(disk) => format!(" {}  {}  │  {}", path_display, size_display, disk),
        None => format!(" {}  {}", path_display, size_display),
    };

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

/// Get color for size display based on relative size.
/// Green for small entries, red for large entries.
fn size_color(size: u64, max_size: u64) -> Color {
    if max_size == 0 {
        return Color::Gray;
    }

    let ratio = size as f64 / max_size as f64;

    if ratio < 0.25 {
        Color::Green
    } else if ratio < 0.50 {
        Color::Yellow
    } else if ratio < 0.75 {
        Color::Rgb(255, 165, 0) // Orange
    } else {
        Color::Red
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
    let bar_filled = "█".repeat(fill);
    let bar_empty = "░".repeat(bar_width - fill);

    // Size text - fixed width for alignment (max "1023.9 TiB" = 10 chars)
    let size_width = 10;
    let size_str = humansize::format_size(entry.entry.size, humansize::BINARY);
    let size_str_padded = format!("{:>width$}", size_str, width = size_width);

    // Get color for size display
    let size_style_color = size_color(entry.entry.size, max_size);

    // Project type indicator (e.g., "[Rust]")
    let project_indicator = entry
        .project_type
        .as_ref()
        .map(|t| format!(" [{}]", t))
        .unwrap_or_default();

    // Calculate available width for name + project indicator combined
    // Use chars().count() for display width, not byte length
    let prefix_len = indent.chars().count() + icon.chars().count();
    // bar is [████░░░░░░] = bar_width + 2 (for brackets), size is fixed width
    let suffix_len = (bar_width + 2) + size_width + 2;
    let name_area_width = (width as usize).saturating_sub(prefix_len + suffix_len);

    // Project indicator comes out of the name area
    let name_width = name_area_width.saturating_sub(project_indicator.chars().count());

    // Truncate name if needed - use char boundaries for proper Unicode handling
    let name = &entry.entry.name;
    let name_char_count = name.chars().count();
    let display_name = if name_char_count > name_width && name_width > 1 {
        let truncated: String = name.chars().take(name_width.saturating_sub(1)).collect();
        format!("{}…", truncated)
    } else {
        name.clone()
    };

    // Build the line using spans for mixed colors
    // Padding fills the remaining space in name_area after name + project indicator
    let used_in_name_area = display_name.chars().count() + project_indicator.chars().count();
    let padding = " ".repeat(name_area_width.saturating_sub(used_in_name_area));

    // Base style for name (blue for dirs, white for files)
    let name_style = if entry.entry.is_dir {
        Style::default().fg(Color::Blue).bold()
    } else {
        Style::default().fg(Color::White)
    };

    // Project indicator style (cyan)
    let project_style = Style::default().fg(Color::Cyan);

    // Size style (colored based on relative size)
    let size_style = Style::default().fg(size_style_color);

    // Build spans
    let mut spans = vec![
        Span::styled(format!("{}{}", indent, icon), name_style),
        Span::styled(display_name.clone(), name_style),
    ];

    if !project_indicator.is_empty() {
        spans.push(Span::styled(project_indicator.clone(), project_style));
    }

    spans.push(Span::raw(padding));
    spans.push(Span::raw(" "));
    spans.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
    spans.push(Span::styled(bar_filled, size_style));
    spans.push(Span::styled(bar_empty, Style::default().fg(Color::DarkGray)));
    spans.push(Span::styled("]", Style::default().fg(Color::DarkGray)));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(size_str_padded, size_style));

    let mut line = Line::from(spans);

    // Apply selection background
    if is_selected {
        line = line.style(Style::default().bg(Color::DarkGray));
    }

    let area = Rect::new(x, y, width, 1);
    frame.render_widget(Paragraph::new(line), area);
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

    #[test]
    fn test_size_color_gradient() {
        let max_size = 1000;

        // Small size -> Green
        assert_eq!(size_color(100, max_size), Color::Green);
        assert_eq!(size_color(249, max_size), Color::Green);

        // Medium-small -> Yellow
        assert_eq!(size_color(250, max_size), Color::Yellow);
        assert_eq!(size_color(499, max_size), Color::Yellow);

        // Medium-large -> Orange
        assert_eq!(size_color(500, max_size), Color::Rgb(255, 165, 0));
        assert_eq!(size_color(749, max_size), Color::Rgb(255, 165, 0));

        // Large -> Red
        assert_eq!(size_color(750, max_size), Color::Red);
        assert_eq!(size_color(1000, max_size), Color::Red);
    }

    #[test]
    fn test_size_color_zero_max() {
        // Zero max size should return Gray
        assert_eq!(size_color(100, 0), Color::Gray);
    }

    #[test]
    fn test_size_color_edge_cases() {
        // Same size as max
        assert_eq!(size_color(1000, 1000), Color::Red);

        // Zero size
        assert_eq!(size_color(0, 1000), Color::Green);

        // Very large sizes
        assert_eq!(size_color(1_000_000_000, 1_000_000_000), Color::Red);
    }
}
