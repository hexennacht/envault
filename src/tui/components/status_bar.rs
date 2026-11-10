use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::app::{App, FocusZone, Screen, StatusBar, StatusLevel};

pub const DEFAULT_LIST_SHORTCUTS: &str =
    "a=add  e=edit  d=delete  Enter=view  A=audit  /=search  q=quit";
pub const SEARCH_SHORTCUTS: &str = "Type=filter  Backspace=delete  Esc=clear/exit  Enter=keep";
pub const AUDIT_SHORTCUTS: &str = "j/k=scroll  g=top  G=bottom  Esc/q=back";
pub const ADD_FORM_SHORTCUTS: &str = "Tab=switch field  Enter=save  Esc=cancel";
pub const EDIT_FORM_SHORTCUTS: &str = "Enter=save  Esc=cancel";

/// Parse a shortcuts string into styled Spans with keys highlighted.
pub fn format_shortcuts(shortcuts: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let tokens: Vec<&str> = shortcuts.split_whitespace().collect();

    for (i, token) in tokens.iter().enumerate() {
        if let Some((key, action)) = token.split_once('=') {
            spans.push(Span::styled(
                key.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!("={action}"),
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            spans.push(Span::styled(
                (*token).to_string(),
                Style::default().fg(Color::DarkGray),
            ));
        }

        if i + 1 < tokens.len() {
            spans.push(Span::raw("  "));
        }
    }

    spans
}

/// Render the status bar displaying shortcuts and any active status message.
pub fn render_status_bar(frame: &mut Frame, status: &StatusBar, shortcuts: &str, area: Rect) {
    let block = if area.height >= 3 {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray))
    } else {
        Block::default()
    };

    let inner_width = if area.height >= 3 {
        area.width.saturating_sub(2) as usize
    } else {
        area.width as usize
    };

    let status_style = match status.level {
        StatusLevel::Info => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        StatusLevel::Error => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    };

    let mut line_spans = Vec::new();
    let shortcut_spans = format_shortcuts(shortcuts);

    if status.text.trim().is_empty() {
        line_spans = shortcut_spans;
    } else {
        let status_text = format!("● {}", status.text.trim());
        let shortcuts_len: usize = shortcut_spans.iter().map(|s| s.content.chars().count()).sum();
        let status_len = status_text.chars().count();

        if shortcuts_len + status_len + 2 <= inner_width {
            let padding_len = inner_width.saturating_sub(shortcuts_len + status_len);
            line_spans = shortcut_spans;
            line_spans.push(Span::raw(" ".repeat(padding_len)));
            line_spans.push(Span::styled(status_text, status_style));
        } else {
            // Tight space: prioritize status text
            line_spans.push(Span::styled(status_text, status_style));
        }
    }

    let paragraph = Paragraph::new(Line::from(line_spans)).block(block);
    frame.render_widget(paragraph, area);
}

/// Render status bar using shortcuts determined by the current screen and focus of App.
pub fn render_app_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let shortcuts = match &app.screen {
        Screen::SecretList => match app.focus {
            FocusZone::List => DEFAULT_LIST_SHORTCUTS,
            FocusZone::SearchBar => SEARCH_SHORTCUTS,
        },
        Screen::AddSecret(_) => ADD_FORM_SHORTCUTS,
        Screen::EditSecret(_) => EDIT_FORM_SHORTCUTS,
        Screen::AuditLog(_) => AUDIT_SHORTCUTS,
    };

    render_status_bar(frame, &app.status, shortcuts, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_format_shortcuts() {
        let spans = format_shortcuts("a=add  q=quit");
        assert!(!spans.is_empty());
        let full_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(full_text.contains("a=add"));
        assert!(full_text.contains("q=quit"));
    }

    #[test]
    fn test_render_status_bar_shortcuts_only() {
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let status = StatusBar::default();

        terminal
            .draw(|frame| {
                render_status_bar(frame, &status, DEFAULT_LIST_SHORTCUTS, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("a=add"));
        assert!(content.contains("q=quit"));
    }

    #[test]
    fn test_render_status_bar_with_info() {
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let status = StatusBar::info("Copied to clipboard");

        terminal
            .draw(|frame| {
                render_status_bar(frame, &status, DEFAULT_LIST_SHORTCUTS, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("Copied to clipboard"));
    }

    #[test]
    fn test_render_status_bar_with_error() {
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let status = StatusBar::error("Decryption failed");

        terminal
            .draw(|frame| {
                render_status_bar(frame, &status, DEFAULT_LIST_SHORTCUTS, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("Decryption failed"));
    }

    #[test]
    fn test_render_app_status_bar() {
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new("test".into(), "test".into(), vec![], &[]);

        terminal
            .draw(|frame| {
                render_app_status_bar(frame, &app, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("a=add"));
    }
}
