use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::tui::app::{App, AuditLogState};
use crate::tui::components::status_bar::format_shortcuts;
use crate::vault::audit::AuditAction;

/// Render the AuditLog screen per SPEC §8.8.
pub fn render_audit_log(frame: &mut Frame, app: &App, state: &AuditLogState, area: Rect) {
    let title = format!(" Audit Log ─── project: {} ", app.project);
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
        Constraint::Min(0),    // Table of audit entries
        Constraint::Length(1), // Bottom help line
    ])
    .split(inner);

    // 1. Audit entries table
    let header = Row::new(vec![
        Cell::from(Span::styled(
            "Time",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Scope",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Action",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Key",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
    ])
    .bottom_margin(0);

    let table_height = chunks[0].height.saturating_sub(1) as usize;
    let mut offset = state.scroll_offset;
    if table_height > 0 {
        if state.selected < offset {
            offset = state.selected;
        } else if state.selected >= offset + table_height {
            offset = state.selected + 1 - table_height;
        }
    }

    let rows: Vec<Row> = if state.entries.is_empty() {
        vec![Row::new(vec![
            Cell::from(Span::styled(
                "(no audit entries)",
                Style::default().fg(Color::DarkGray),
            )),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
        ])]
    } else {
        state
            .entries
            .iter()
            .enumerate()
            .skip(offset)
            .take(table_height)
            .map(|(idx, entry)| {
                let is_selected = idx == state.selected;
                let time_str = chrono::DateTime::from_timestamp(entry.at, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| entry.at.to_string());

                let action_style = match entry.action {
                    AuditAction::Read => Style::default().fg(Color::Cyan),
                    AuditAction::Write => Style::default().fg(Color::Yellow),
                    AuditAction::Delete => Style::default().fg(Color::Red),
                };

                let row_style = if is_selected {
                    Style::default().bg(Color::Rgb(40, 44, 52)).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(Span::styled(
                        time_str,
                        if is_selected {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Gray)
                        },
                    )),
                    Cell::from(Span::styled(
                        &entry.scope,
                        if is_selected {
                            Style::default().fg(Color::White)
                        } else {
                            Style::default().fg(Color::Gray)
                        },
                    )),
                    Cell::from(Span::styled(
                        entry.action.as_str(),
                        action_style.add_modifier(if is_selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                    )),
                    Cell::from(Span::styled(
                        &entry.key,
                        if is_selected {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    )),
                ])
                .style(row_style)
            })
            .collect()
    };

    let widths = [
        Constraint::Length(22),
        Constraint::Length(16),
        Constraint::Length(10),
        Constraint::Min(16),
    ];

    let table = Table::new(rows, widths).header(header);
    frame.render_widget(table, chunks[0]);

    // 2. Bottom help line
    let help_spans = format_shortcuts("j/k=scroll  g=top  G=bottom  Esc/q=back");
    let help_paragraph = Paragraph::new(Line::from(help_spans));
    frame.render_widget(help_paragraph, chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::audit::AuditEntry;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_render_audit_log_empty() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new("test-project".into(), "dev".into(), vec![], &[]);
        let state = AuditLogState::new(vec![]);

        terminal
            .draw(|frame| {
                render_audit_log(frame, &app, &state, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("Audit Log ─── project: test-project"));
        assert!(content.contains("Time"));
        assert!(content.contains("Scope"));
        assert!(content.contains("Action"));
        assert!(content.contains("Key"));
        assert!(content.contains("(no audit entries)"));
        assert!(content.contains("j/k=scroll"));
        assert!(content.contains("Esc/q=back"));
    }

    #[test]
    fn test_render_audit_log_populated() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new("my-project".into(), "staging".into(), vec![], &[]);
        let entries = vec![
            AuditEntry {
                id: 1,
                project: "my-project".into(),
                scope: "staging".into(),
                key: "DATABASE_URL".into(),
                action: AuditAction::Read,
                at: 1736937121, // 2025-01-15 10:32:01 UTC
            },
            AuditEntry {
                id: 2,
                project: "my-project".into(),
                scope: "staging".into(),
                key: "STRIPE_SECRET_KEY".into(),
                action: AuditAction::Write,
                at: 1736937045, // 2025-01-15 10:30:45 UTC
            },
            AuditEntry {
                id: 3,
                project: "my-project".into(),
                scope: "staging".into(),
                key: "OLD_KEY".into(),
                action: AuditAction::Delete,
                at: 1736932320, // 2025-01-15 09:12:00 UTC
            },
        ];
        let state = AuditLogState::new(entries);

        terminal
            .draw(|frame| {
                render_audit_log(frame, &app, &state, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        assert!(content.contains("Audit Log ─── project: my-project"));
        assert!(content.contains("DATABASE_URL"));
        assert!(content.contains("STRIPE_SECRET_KEY"));
        assert!(content.contains("OLD_KEY"));
        assert!(content.contains("READ"));
        assert!(content.contains("WRITE"));
        assert!(content.contains("DELETE"));
        assert!(content.contains("2025-01-15"));
    }

    #[test]
    fn test_render_audit_log_scrolling() {
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new("my-project".into(), "staging".into(), vec![], &[]);
        let entries: Vec<AuditEntry> = (0..20)
            .map(|i| AuditEntry {
                id: i,
                project: "my-project".into(),
                scope: "staging".into(),
                key: format!("KEY_{i:02}"),
                action: AuditAction::Write,
                at: 1736937000 + i,
            })
            .collect();
        let mut state = AuditLogState::new(entries);
        state.selected = 15;

        terminal
            .draw(|frame| {
                render_audit_log(frame, &app, &state, frame.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = format!("{buffer:?}");
        // Selected item KEY_15 must be visible
        assert!(content.contains("KEY_15"));
    }

    #[test]
    fn test_render_audit_log_tiny_area() {
        let sizes = [(0, 0), (2, 2), (5, 5), (20, 5)];
        for (w, h) in sizes {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();
            let app = App::new("my-project".into(), "staging".into(), vec![], &[]);
            let state = AuditLogState::new(vec![]);

            let res = terminal.draw(|frame| {
                render_audit_log(frame, &app, &state, frame.area());
            });
            assert!(res.is_ok());
        }
    }
}
