use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::tui::app::{App, EditSecretForm};
use crate::tui::components::popup::centered_rect_exact;
use crate::tui::screens::secret_list::render_secret_list;

/// Render the EditSecret modal screen per SPEC §8.4.
pub fn render_edit_secret(frame: &mut Frame, app: &App, form: &EditSecretForm, area: Rect) {
    // Render the base secret list underneath
    render_secret_list(frame, app, area);

    // Compute centered dialog box
    let dialog_area = centered_rect_exact(64, 9, area);
    frame.render_widget(Clear, dialog_area);

    let title = format!(" Edit Secret: {} ", form.key);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            title,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));

    if dialog_area.width < 4 || dialog_area.height < 4 {
        frame.render_widget(block, dialog_area);
        return;
    }

    // 1. Key (read-only)
    let key_line = Line::from(vec![
        Span::styled("  Key:       ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            &form.key,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" (read-only)", Style::default().fg(Color::DarkGray)),
    ]);

    // 2. New Value field (masked with '█' characters and cursor)
    let mut val_spans = vec![Span::styled(
        "  New Value: ",
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )];
    val_spans.extend(crate::tui::screens::format_masked_input(
        &form.value_input,
        form.cursor_pos,
        true,
    ));
    let val_line = Line::from(val_spans);

    // 3. Helper text
    let helper_line = Line::from(vec![
        Span::raw("  "),
        Span::styled("[Enter]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(" save   ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Esc]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
    ]);

    let paragraph = Paragraph::new(vec![
        Line::from(""),
        key_line,
        Line::from(""),
        val_line,
        Line::from(""),
        helper_line,
        Line::from(""),
    ])
    .block(block);

    frame.render_widget(paragraph, dialog_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_render_edit_secret() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new("test-project".into(), "dev".into(), vec![], &[]);
        let mut form = EditSecretForm::new("DATABASE_URL");
        form.value_input = "new_secret_password".into();
        form.cursor_pos = 19;

        terminal
            .draw(|frame| {
                render_edit_secret(frame, &app, &form, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("Edit Secret: DATABASE_URL"));
        assert!(content.contains("Key:"));
        assert!(content.contains("DATABASE_URL (read-only)"));
        assert!(content.contains("New Value:"));
        assert!(content.contains('█'));
        assert!(!content.contains("new_secret_password"));
        assert!(content.contains("[Enter] save"));
        assert!(content.contains("[Esc] cancel"));
    }

    #[test]
    fn test_render_edit_secret_empty_value() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new("test-project".into(), "dev".into(), vec![], &[]);
        let form = EditSecretForm::new("API_TOKEN");

        terminal
            .draw(|frame| {
                render_edit_secret(frame, &app, &form, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("Edit Secret: API_TOKEN"));
        assert!(content.contains("API_TOKEN (read-only)"));
        assert!(content.contains('_'));
    }

    #[test]
    fn test_render_edit_secret_tiny_area() {
        let sizes = [(0, 0), (5, 5), (20, 5), (60, 8)];
        for (w, h) in sizes {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();
            let app = App::new("test-project".into(), "dev".into(), vec![], &[]);
            let form = EditSecretForm::new("KEY");

            let res = terminal.draw(|frame| {
                render_edit_secret(frame, &app, &form, frame.area());
            });
            assert!(res.is_ok());
        }
    }
}
