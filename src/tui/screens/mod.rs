pub mod add_secret;
pub mod audit_log;
pub mod edit_secret;
pub mod secret_list;

pub use add_secret::render_add_secret;
pub use audit_log::render_audit_log;
pub use edit_secret::render_edit_secret;
pub use secret_list::render_secret_list;

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::tui::app::{App, Screen};

/// Dispatch rendering to the appropriate screen according to `app.screen`.
/// Also renders active popup overlay on top if present.
pub fn render_screen(frame: &mut Frame, app: &App, area: Rect) {
    match &app.screen {
        Screen::SecretList => render_secret_list(frame, app, area),
        Screen::AddSecret(form) => render_add_secret(frame, app, form, area),
        Screen::EditSecret(form) => render_edit_secret(frame, app, form, area),
        Screen::AuditLog(state) => render_audit_log(frame, app, state, area),
    }

    if let Some(popup) = &app.popup {
        crate::tui::components::popup::render_popup(frame, popup, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::{AddSecretForm, EditSecretForm, PopupKind};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_render_screen_dispatches_correctly() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new("test-proj".into(), "dev".into(), vec![], &[]);

        // 1. SecretList
        terminal
            .draw(|frame| {
                render_screen(frame, &app, frame.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert!(format!("{buffer:?}").contains("test-proj"));

        // 2. AddSecret
        app.screen = Screen::AddSecret(AddSecretForm::new());
        terminal
            .draw(|frame| {
                render_screen(frame, &app, frame.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert!(format!("{buffer:?}").contains("Add Secret"));

        // 3. EditSecret
        app.screen = Screen::EditSecret(EditSecretForm::new("KEY_NAME"));
        terminal
            .draw(|frame| {
                render_screen(frame, &app, frame.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert!(format!("{buffer:?}").contains("Edit Secret: KEY_NAME"));

        // 4. Popup overlay
        app.popup = Some(PopupKind::ConfirmDelete {
            key: "KEY_NAME".into(),
        });
        terminal
            .draw(|frame| {
                render_screen(frame, &app, frame.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert!(format!("{buffer:?}").contains("Confirm Delete"));
    }
}
