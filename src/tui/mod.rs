pub mod app;
pub mod components;
pub mod screens;

pub use app::*;
pub use screens::render_screen;

use std::io::Write;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::Terminal;

use crate::schema::types::SchemaEntry;
use crate::vault::VaultClient;

/// Restores the terminal to its original state (disables raw mode, leaves alternate screen, shows cursor).
pub fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        std::io::stdout(),
        LeaveAlternateScreen,
        crossterm::cursor::Show
    );
}

/// Copies `text` to system clipboard, silently ignoring errors if no clipboard tool exists.
fn copy_to_clipboard(text: &str) {
    #[cfg(target_os = "macos")]
    {
        if let Ok(mut child) = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(mut child) = std::process::Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
            return;
        }
        if let Ok(mut child) = std::process::Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(mut child) = std::process::Command::new("clip")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }
    let _ = text;
}

/// Dispatch key event to active popup overlay.
pub fn handle_popup_event(app: &mut App, client: &mut VaultClient, key_event: KeyEvent) {
    let popup = match &app.popup {
        Some(p) => p.clone(),
        None => return,
    };

    match popup {
        PopupKind::ConfirmDelete { key } => match key_event.code {
            KeyCode::Enter => match client.delete(&key) {
                Ok(()) => match client.list() {
                    Ok(stored) => {
                        app.reload_secrets(&stored);
                        app.set_status_info(format!("Deleted secret '{}'", key));
                        app.popup = None;
                    }
                    Err(e) => {
                        app.show_error("Failed to Reload Secrets", format!("{e:#}"));
                    }
                },
                Err(e) => {
                    app.show_error("Failed to Delete Secret", format!("{e:#}"));
                }
            },
            KeyCode::Esc => {
                app.popup = None;
            }
            _ => {}
        },
        PopupKind::ShowValue { value, .. } => match key_event.code {
            KeyCode::Esc | KeyCode::Enter => {
                app.close_popup();
            }
            KeyCode::Char('c') => {
                copy_to_clipboard(&value);
                app.set_status_info("Copied to clipboard");
            }
            _ => {}
        },
        PopupKind::Error { .. } => match key_event.code {
            KeyCode::Esc | KeyCode::Enter => {
                app.popup = None;
            }
            _ => {}
        },
    }
}

/// Dispatch key event for the SecretList screen.
pub fn handle_list_event(app: &mut App, client: &mut VaultClient, key_event: KeyEvent) {
    match app.focus {
        FocusZone::SearchBar => match key_event.code {
            KeyCode::Char(c) => {
                app.search_query.push(c);
                app.clamp_selection();
            }
            KeyCode::Backspace => {
                app.search_query.pop();
                app.clamp_selection();
            }
            KeyCode::Esc => {
                app.clear_search();
                app.focus = FocusZone::List;
            }
            KeyCode::Enter => {
                app.focus = FocusZone::List;
            }
            _ => {}
        },
        FocusZone::List => match key_event.code {
            KeyCode::Char('q') => {
                app.mode = AppMode::Quit;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                app.move_selection_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.move_selection_up();
            }
            KeyCode::Char('/') => {
                app.focus = FocusZone::SearchBar;
            }
            KeyCode::Char('a') => {
                app.open_add_secret();
            }
            KeyCode::Char('e') => {
                app.open_edit_selected();
            }
            KeyCode::Char('d') => {
                app.open_confirm_delete_selected();
            }
            KeyCode::Enter => {
                if let Some(secret) = app.selected_secret().cloned() {
                    match client.get(&secret.key) {
                        Ok(Some(val)) => {
                            app.open_show_value(secret.key, val);
                        }
                        Ok(None) => {
                            app.show_error(
                                "Secret Not Found",
                                format!("Secret '{}' is not set in vault", secret.key),
                            );
                        }
                        Err(e) => {
                            app.show_error("Decryption Failed", format!("{e:#}"));
                        }
                    }
                }
            }
            KeyCode::Char('A') => match client.get_audit_log() {
                Ok(entries) => {
                    app.open_audit_log(entries);
                }
                Err(e) => {
                    app.show_error("Audit Log Failed", format!("{e:#}"));
                }
            },
            _ => {}
        },
    }
}

/// Dispatch key event for the AddSecret screen.
pub fn handle_add_event(app: &mut App, client: &mut VaultClient, key_event: KeyEvent) {
    match key_event.code {
        KeyCode::Esc => {
            app.return_to_list();
        }
        KeyCode::Enter => {
            let (key, value) = if let Screen::AddSecret(ref form) = app.screen {
                (form.key_input.trim().to_string(), form.value_input.clone())
            } else {
                return;
            };

            if key.is_empty() {
                app.set_status_error("Key cannot be empty");
            } else {
                match client.set(&key, &value) {
                    Ok(()) => match client.list() {
                        Ok(stored) => {
                            app.reload_secrets(&stored);
                            app.set_status_info(format!("Saved secret '{}'", key));
                            app.return_to_list();
                        }
                        Err(e) => {
                            app.show_error("Failed to Reload Secrets", format!("{e:#}"));
                        }
                    },
                    Err(e) => {
                        app.show_error("Failed to Save Secret", format!("{e:#}"));
                    }
                }
            }
        }
        _ => {
            if let Screen::AddSecret(ref mut form) = app.screen {
                match key_event.code {
                    KeyCode::Tab => {
                        form.toggle_focus();
                    }
                    KeyCode::Char(c) => match form.focused_field {
                        FormField::Key => {
                            if c.is_ascii_alphanumeric() || c == '_' {
                                if form.cursor_pos >= form.key_input.len() {
                                    form.key_input.push(c);
                                    form.cursor_pos = form.key_input.len();
                                } else {
                                    form.key_input.insert(form.cursor_pos, c);
                                    form.cursor_pos += 1;
                                }
                            }
                        }
                        FormField::Value => {
                            if form.cursor_pos >= form.value_input.len() {
                                form.value_input.push(c);
                                form.cursor_pos = form.value_input.len();
                            } else {
                                form.value_input.insert(form.cursor_pos, c);
                                form.cursor_pos += 1;
                            }
                        }
                    },
                    KeyCode::Backspace => match form.focused_field {
                        FormField::Key => {
                            if form.cursor_pos > 0 && form.cursor_pos <= form.key_input.len() {
                                form.cursor_pos -= 1;
                                form.key_input.remove(form.cursor_pos);
                            }
                        }
                        FormField::Value => {
                            if form.cursor_pos > 0 && form.cursor_pos <= form.value_input.len() {
                                form.cursor_pos -= 1;
                                form.value_input.remove(form.cursor_pos);
                            }
                        }
                    },
                    KeyCode::Left => {
                        form.cursor_pos = form.cursor_pos.saturating_sub(1);
                    }
                    KeyCode::Right => {
                        let max_len = match form.focused_field {
                            FormField::Key => form.key_input.len(),
                            FormField::Value => form.value_input.len(),
                        };
                        if form.cursor_pos < max_len {
                            form.cursor_pos += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Dispatch key event for the EditSecret screen.
pub fn handle_edit_event(app: &mut App, client: &mut VaultClient, key_event: KeyEvent) {
    match key_event.code {
        KeyCode::Esc => {
            app.return_to_list();
        }
        KeyCode::Enter => {
            let (key, value) = if let Screen::EditSecret(ref form) = app.screen {
                (form.key.clone(), form.value_input.clone())
            } else {
                return;
            };

            match client.set(&key, &value) {
                Ok(()) => match client.list() {
                    Ok(stored) => {
                        app.reload_secrets(&stored);
                        app.set_status_info(format!("Updated secret '{}'", key));
                        app.return_to_list();
                    }
                    Err(e) => {
                        app.show_error("Failed to Reload Secrets", format!("{e:#}"));
                    }
                },
                Err(e) => {
                    app.show_error("Failed to Save Secret", format!("{e:#}"));
                }
            }
        }
        _ => {
            if let Screen::EditSecret(ref mut form) = app.screen {
                match key_event.code {
                    KeyCode::Char(c) => {
                        if form.cursor_pos >= form.value_input.len() {
                            form.value_input.push(c);
                            form.cursor_pos = form.value_input.len();
                        } else {
                            form.value_input.insert(form.cursor_pos, c);
                            form.cursor_pos += 1;
                        }
                    }
                    KeyCode::Backspace => {
                        if form.cursor_pos > 0 && form.cursor_pos <= form.value_input.len() {
                            form.cursor_pos -= 1;
                            form.value_input.remove(form.cursor_pos);
                        }
                    }
                    KeyCode::Left => {
                        form.cursor_pos = form.cursor_pos.saturating_sub(1);
                    }
                    KeyCode::Right if form.cursor_pos < form.value_input.len() => {
                        form.cursor_pos += 1;
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Dispatch key event for the AuditLog screen.
pub fn handle_audit_event(app: &mut App, key_event: KeyEvent) {
    match key_event.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.return_to_list();
        }
        _ => {
            if let Screen::AuditLog(ref mut state) = app.screen {
                match key_event.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        state.scroll_down(20);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        state.scroll_up();
                    }
                    KeyCode::Char('g') => {
                        state.scroll_to_top();
                    }
                    KeyCode::Char('G') => {
                        state.scroll_to_bottom(20);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Dispatch key event to active popup overlay or active screen.
pub fn handle_key_event(app: &mut App, client: &mut VaultClient, key_event: KeyEvent) {
    if key_event.kind == KeyEventKind::Release {
        return;
    }

    if app.popup.is_some() {
        handle_popup_event(app, client, key_event);
        return;
    }

    match &app.screen {
        Screen::SecretList => handle_list_event(app, client, key_event),
        Screen::AddSecret(_) => handle_add_event(app, client, key_event),
        Screen::EditSecret(_) => handle_edit_event(app, client, key_event),
        Screen::AuditLog(_) => handle_audit_event(app, key_event),
    }
}

/// Run the TUI event loop until `app.mode == AppMode::Quit`.
pub fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    client: &mut VaultClient,
) -> anyhow::Result<()> {
    while app.mode != AppMode::Quit {
        terminal.draw(|frame| {
            screens::render_screen(frame, app, frame.area());
        })?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            handle_key_event(app, client, key);
        }
    }
    Ok(())
}

/// Initialize terminal, set panic hook, execute run_app, and restore terminal on exit.
pub fn run(
    mut client: VaultClient,
    project: String,
    scope: String,
    schema: Vec<SchemaEntry>,
) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        default_hook(panic_info);
    }));

    let stored_keys = client.list().unwrap_or_default();
    let mut app = App::new(project, scope, schema, &stored_keys);

    let res = run_app(&mut terminal, &mut app, &mut client);

    restore_terminal();
    let _ = terminal.show_cursor();

    res
}
