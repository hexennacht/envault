use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::tui::app::{AddSecretForm, App, FormField};
use crate::tui::components::popup::centered_rect_exact;
use crate::tui::screens::secret_list::render_secret_list;

/// Render the AddSecret modal screen per SPEC §8.3.
pub fn render_add_secret(frame: &mut Frame, app: &App, form: &AddSecretForm, area: Rect) {
    // Render the base secret list underneath
    render_secret_list(frame, app, area);

    // Compute centered dialog box
    let dialog_area = centered_rect_exact(64, 9, area);
    frame.render_widget(Clear, dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Add Secret ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));

    if dialog_area.width < 4 || dialog_area.height < 4 {
        frame.render_widget(block, dialog_area);
        return;
    }

    // 1. Key field
    let is_key_focused = form.focused_field == FormField::Key;
    let key_label_style = if is_key_focused {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let mut key_spans = vec![Span::styled("  Key:   ", key_label_style)];
    if is_key_focused {
        let text = &form.key_input;
        let pos = form.cursor_pos.min(text.len());
        let (before, after) = text.split_at(pos);
        key_spans.push(Span::styled(before, Style::default().fg(Color::White)));
        if let Some(ch) = after.chars().next() {
            let mut buf = [0u8; 4];
            let ch_str = ch.encode_utf8(&mut buf);
            key_spans.push(Span::styled(
                ch_str.to_string(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::REVERSED),
            ));
            let remainder = &after[ch.len_utf8()..];
            if !remainder.is_empty() {
                key_spans.push(Span::styled(remainder, Style::default().fg(Color::White)));
            }
        } else {
            key_spans.push(Span::styled(
                "_",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
        }
    } else {
        let text_style = if form.key_input.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };
        let display_text = if form.key_input.is_empty() {
            "(empty)"
        } else {
            &form.key_input
        };
        key_spans.push(Span::styled(display_text, text_style));
    }
    let key_line = Line::from(key_spans);

    // 2. Value field
    let is_val_focused = form.focused_field == FormField::Value;
    let val_label_style = if is_val_focused {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let mut val_spans = vec![Span::styled("  Value: ", val_label_style)];
    let val_len = form.value_input.chars().count();
    let masked_char = '█';

    if is_val_focused {
        let pos = form.cursor_pos.min(val_len);
        let before_str = "█".repeat(pos);
        val_spans.push(Span::styled(before_str, Style::default().fg(Color::White)));

        if pos < val_len {
            val_spans.push(Span::styled(
                masked_char.to_string(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::REVERSED),
            ));
            let after_str = "█".repeat(val_len - pos - 1);
            if !after_str.is_empty() {
                val_spans.push(Span::styled(after_str, Style::default().fg(Color::White)));
            }
        } else {
            val_spans.push(Span::styled(
                "_",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
        }
    } else if form.value_input.is_empty() {
        val_spans.push(Span::styled("(empty)", Style::default().fg(Color::DarkGray)));
    } else {
        let masked_str = "█".repeat(val_len);
        val_spans.push(Span::styled(masked_str, Style::default().fg(Color::White)));
    }
    let val_line = Line::from(val_spans);

    // 3. Helper text
    let helper_line = Line::from(vec![
        Span::raw("  "),
        Span::styled("[Tab]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(" switch field   ", Style::default().fg(Color::DarkGray)),
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
    fn test_render_add_secret_key_focused() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new("test-project".into(), "dev".into(), vec![], &[]);
        let mut form = AddSecretForm::new();
        form.key_input = "MY_NEW_SECRET".into();
        form.cursor_pos = 13;

        terminal
            .draw(|frame| {
                render_add_secret(frame, &app, &form, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("Add Secret"));
        assert!(content.contains("Key:"));
        assert!(content.contains("MY_NEW_SECRET"));
        assert!(content.contains("Value:"));
        assert!(content.contains("[Tab] switch field"));
        assert!(content.contains("[Enter] save"));
        assert!(content.contains("[Esc] cancel"));
    }

    #[test]
    fn test_render_add_secret_value_focused() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new("test-project".into(), "dev".into(), vec![], &[]);
        let mut form = AddSecretForm::new();
        form.key_input = "API_KEY".into();
        form.value_input = "supersecret".into();
        form.focused_field = FormField::Value;
        form.cursor_pos = 11;

        terminal
            .draw(|frame| {
                render_add_secret(frame, &app, &form, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("Add Secret"));
        assert!(content.contains("Key:"));
        assert!(content.contains("API_KEY"));
        assert!(content.contains("Value:"));
        // Value should be masked with '█' and NOT show "supersecret"
        assert!(content.contains('█'));
        assert!(!content.contains("supersecret"));
    }

    #[test]
    fn test_render_add_secret_tiny_area() {
        let sizes = [(0, 0), (5, 5), (20, 5), (60, 8)];
        for (w, h) in sizes {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();
            let app = App::new("test-project".into(), "dev".into(), vec![], &[]);
            let form = AddSecretForm::new();

            let res = terminal.draw(|frame| {
                render_add_secret(frame, &app, &form, frame.area());
            });
            assert!(res.is_ok());
        }
    }
}
