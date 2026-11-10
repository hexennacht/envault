use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::app::PopupKind;

/// Compute a centered sub-rectangle using percentage constraints.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y.min(100)) / 2),
        Constraint::Percentage(percent_y.min(100)),
        Constraint::Percentage((100 - percent_y.min(100)) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x.min(100)) / 2),
        Constraint::Percentage(percent_x.min(100)),
        Constraint::Percentage((100 - percent_x.min(100)) / 2),
    ])
    .split(popup_layout[1])[1]
}

/// Compute a centered sub-rectangle using exact width and height dimensions, bounded by area.
pub fn centered_rect_exact(width: u16, height: u16, area: Rect) -> Rect {
    let actual_width = width.min(area.width);
    let actual_height = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(actual_width)) / 2;
    let y = area.y + (area.height.saturating_sub(actual_height)) / 2;
    Rect {
        x,
        y,
        width: actual_width,
        height: actual_height,
    }
}

/// Dispatcher to render the appropriate popup overlay based on `PopupKind`.
pub fn render_popup(frame: &mut Frame, popup: &PopupKind, area: Rect) {
    match popup {
        PopupKind::ConfirmDelete { key } => {
            let popup_area = centered_rect_exact(56, 8, area);
            render_confirm_delete(frame, key, popup_area);
        }
        PopupKind::ShowValue { key, value } => {
            let popup_area = centered_rect_exact(64, 8, area);
            render_show_value(frame, key, value.as_str(), popup_area);
        }
        PopupKind::Error { title, detail } => {
            let popup_area = centered_rect_exact(64, 9, area);
            render_error(frame, title, detail, popup_area);
        }
    }
}

/// Render the ConfirmDelete modal popup per SPEC §8.5.
pub fn render_confirm_delete(frame: &mut Frame, key: &str, area: Rect) {
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Delete secret: "),
            Span::styled(
                key,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("?"),
        ]),
        Line::from(vec![Span::styled(
            "  This action cannot be undone.",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "       [ Yes (Enter) ]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("     "),
            Span::styled("[ No (Esc) ]", Style::default().fg(Color::White)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red))
        .title(Span::styled(
            " Confirm Delete ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

/// Render the ShowValue modal popup per SPEC §8.6.
pub fn render_show_value(frame: &mut Frame, key: &str, value: &str, area: Rect) {
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                value,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  [c]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" copy to clipboard   "),
            Span::styled(
                "[Esc]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" close"),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            format!(" {} ", key),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Render the Error modal popup per SPEC §8.7.
pub fn render_error(frame: &mut Frame, title: &str, detail: &str, area: Rect) {
    frame.render_widget(Clear, area);

    let mut text = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                title,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    if !detail.is_empty() {
        text.push(Line::from(vec![
            Span::styled("  Detail: ", Style::default().fg(Color::DarkGray)),
            Span::styled(detail, Style::default().fg(Color::Red)),
        ]));
    }

    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled(
            "  [Esc]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" dismiss"),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red))
        .title(Span::styled(
            format!(" Error: {} ", title),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_centered_rect_exact() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = centered_rect_exact(40, 20, area);
        assert_eq!(centered.width, 40);
        assert_eq!(centered.height, 20);
        assert_eq!(centered.x, 30);
        assert_eq!(centered.y, 15);
    }

    #[test]
    fn test_centered_rect_exact_bounded() {
        let area = Rect::new(0, 0, 30, 10);
        let centered = centered_rect_exact(50, 20, area);
        assert_eq!(centered.width, 30);
        assert_eq!(centered.height, 10);
        assert_eq!(centered.x, 0);
        assert_eq!(centered.y, 0);
    }

    #[test]
    fn test_centered_rect_percentage() {
        let area = Rect::new(0, 0, 100, 100);
        let centered = centered_rect(50, 50, area);
        assert_eq!(centered.width, 50);
        assert_eq!(centered.height, 50);
        assert_eq!(centered.x, 25);
        assert_eq!(centered.y, 25);
    }

    #[test]
    fn test_render_confirm_delete_popup() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let popup = PopupKind::ConfirmDelete {
                    key: "DATABASE_URL".into(),
                };
                render_popup(frame, &popup, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("Confirm Delete"));
        assert!(content.contains("DATABASE_URL"));
        assert!(content.contains("Yes (Enter)"));
    }

    #[test]
    fn test_render_show_value_popup() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let popup = PopupKind::ShowValue {
                    key: "API_KEY".into(),
                    value: zeroize::Zeroizing::new("secret_12345".into()),
                };
                render_popup(frame, &popup, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("API_KEY"));
        assert!(content.contains("secret_12345"));
        assert!(content.contains("copy to clipboard"));
    }

    #[test]
    fn test_render_error_popup() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let popup = PopupKind::Error {
                    title: "Decryption Failed".into(),
                    detail: "Corrupted auth tag".into(),
                };
                render_popup(frame, &popup, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("Decryption Failed"));
        assert!(content.contains("Corrupted auth tag"));
        assert!(content.contains("dismiss"));
    }
}
