use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::app::FocusZone;

/// Render the search bar widget styled according to the active FocusZone.
pub fn render_search_bar(frame: &mut Frame, search_query: &str, focus: FocusZone, area: Rect) {
    let is_focused = focus == FocusZone::SearchBar;
    render_search_bar_focused(frame, search_query, is_focused, area);
}

/// Render the search bar widget with explicit focus styling.
pub fn render_search_bar_focused(
    frame: &mut Frame,
    search_query: &str,
    is_focused: bool,
    area: Rect,
) {
    let prefix_style = if is_focused {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let text_style = if is_focused {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::Gray)
    };

    let cursor_style = if is_focused {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mut spans = vec![
        Span::styled("[/] Search: ", prefix_style),
        Span::styled(search_query, text_style),
    ];

    if is_focused {
        spans.push(Span::styled("_", cursor_style));
    } else if search_query.is_empty() {
        spans.push(Span::styled("_", Style::default().fg(Color::DarkGray)));
    }

    let block = if area.height >= 3 {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if is_focused {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            })
            .title(Span::styled(
                " Search ",
                if is_focused {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ))
    } else {
        Block::default()
    };

    let paragraph = Paragraph::new(Line::from(spans)).block(block);
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_render_search_bar_focused() {
        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_search_bar(frame, "my_query", FocusZone::SearchBar, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("Search"));
        assert!(content.contains("my_query"));
    }

    #[test]
    fn test_render_search_bar_unfocused() {
        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_search_bar(frame, "", FocusZone::List, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("Search"));
    }

    #[test]
    fn test_render_search_bar_single_line() {
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_search_bar(frame, "filter", FocusZone::SearchBar, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("filter"));
    }
}
