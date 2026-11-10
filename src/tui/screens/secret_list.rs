use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::tui::app::{App, FocusZone};
use crate::tui::components::search_bar::render_search_bar;
use crate::tui::components::status_bar::{
    render_status_bar, DEFAULT_LIST_SHORTCUTS, SEARCH_SHORTCUTS,
};

/// Render the main SecretList screen per SPEC §8.2.
pub fn render_secret_list(frame: &mut Frame, app: &App, area: Rect) {
    let title = format!(" envault ─── project: {} ── scope: {} ", app.project, app.scope);
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            title,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));

    if area.width < 2 || area.height < 2 {
        frame.render_widget(outer_block, area);
        return;
    }

    let inner = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(1), // Top search bar
        Constraint::Min(0),    // Middle list of secrets
        Constraint::Length(1), // Bottom status bar
    ])
    .split(inner);

    // 1. Search bar
    render_search_bar(frame, &app.search_query, app.focus, chunks[0]);

    // 2. Middle list of secrets
    let filtered = app.filtered_secrets();
    let list_height = chunks[1].height as usize;

    if filtered.is_empty() {
        let empty_msg = if app.search_query.is_empty() {
            "  (no secrets)"
        } else {
            "  (no matching secrets)"
        };
        let empty_line = Line::from(Span::styled(empty_msg, Style::default().fg(Color::DarkGray)));
        let empty_paragraph = Paragraph::new(empty_line);
        frame.render_widget(empty_paragraph, chunks[1]);
    } else {
        let scroll_offset = if list_height == 0 {
            0
        } else if app.selected >= list_height {
            app.selected + 1 - list_height
        } else {
            0
        };

        let rows: Vec<Row> = filtered
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(list_height)
            .map(|(idx, secret)| {
                let is_selected = idx == app.selected;
                let indicator = if is_selected { "▶ " } else { "  " };

                let key_spans = vec![
                    Span::styled(
                        indicator,
                        if is_selected {
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                    Span::styled(
                        &secret.key,
                        if is_selected {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                ];

                let status_span = if secret.has_value {
                    Span::styled(
                        "● set",
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(
                        "○ missing",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )
                };

                let required_span = if secret.required {
                    Span::styled("required", Style::default().fg(Color::Cyan))
                } else {
                    Span::raw("")
                };

                let sensitive_span = if secret.sensitive {
                    Span::styled("sensitive", Style::default().fg(Color::Magenta))
                } else {
                    Span::raw("")
                };

                let row_style = if is_selected {
                    Style::default().bg(Color::Rgb(40, 44, 52))
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(Line::from(key_spans)),
                    Cell::from(status_span),
                    Cell::from(required_span),
                    Cell::from(sensitive_span),
                ])
                .style(row_style)
            })
            .collect();

        let widths = [
            Constraint::Percentage(50),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
        ];

        let table = Table::new(rows, widths);
        frame.render_widget(table, chunks[1]);
    }

    // 3. Bottom status bar
    let shortcuts = match app.focus {
        FocusZone::List => DEFAULT_LIST_SHORTCUTS,
        FocusZone::SearchBar => SEARCH_SHORTCUTS,
    };
    render_status_bar(frame, &app.status, shortcuts, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::types::{SchemaEntry, SchemaType};
    use crate::tui::app::SecretMeta;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_render_secret_list_empty() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new("test-project".into(), "development".into(), vec![], &[]);

        terminal
            .draw(|frame| {
                render_secret_list(frame, &app, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("test-project"));
        assert!(content.contains("development"));
        assert!(content.contains("Search"));
        assert!(content.contains("(no secrets)"));
    }

    #[test]
    fn test_render_secret_list_populated() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        let schema = vec![
            SchemaEntry {
                key: "DATABASE_URL".into(),
                schema_type: SchemaType::String,
                required: true,
                sensitive: true,
                description: None,
                default_value: None,
            },
            SchemaEntry {
                key: "STRIPE_KEY".into(),
                schema_type: SchemaType::String,
                required: true,
                sensitive: true,
                description: None,
                default_value: None,
            },
            SchemaEntry {
                key: "PORT".into(),
                schema_type: SchemaType::Integer,
                required: false,
                sensitive: false,
                description: None,
                default_value: None,
            },
        ];
        let stored_keys = vec!["DATABASE_URL".to_string(), "PORT".to_string()];
        let app = App::new(
            "my-app".into(),
            "staging".into(),
            schema,
            &stored_keys,
        );

        terminal
            .draw(|frame| {
                render_secret_list(frame, &app, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("DATABASE_URL"));
        assert!(content.contains("STRIPE_KEY"));
        assert!(content.contains("PORT"));
        assert!(content.contains("▶"));
        assert!(content.contains("● set"));
        assert!(content.contains("○ missing"));
        assert!(content.contains("required"));
        assert!(content.contains("sensitive"));
    }

    #[test]
    fn test_render_secret_list_with_search_filter() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut app = App::new("proj".into(), "dev".into(), vec![], &[]);
        app.secrets = vec![
            SecretMeta::new("DATABASE_URL", true, true, true),
            SecretMeta::new("REDIS_URL", false, false, true),
            SecretMeta::new("API_KEY", true, true, false),
        ];
        app.set_search_query("REDIS");

        terminal
            .draw(|frame| {
                render_secret_list(frame, &app, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("REDIS_URL"));
        assert!(!content.contains("DATABASE_URL"));
    }

    #[test]
    fn test_render_secret_list_tiny_areas() {
        let sizes = [(0, 0), (1, 1), (2, 2), (5, 3), (20, 5)];
        for (w, h) in sizes {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();
            let app = App::new("proj".into(), "dev".into(), vec![], &[]);

            let res = terminal.draw(|frame| {
                render_secret_list(frame, &app, frame.area());
            });
            assert!(res.is_ok());
        }
    }
}
