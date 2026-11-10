use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use envault::schema::types::{SchemaEntry, SchemaType};
use envault::tui::app::{App, AppMode, FocusZone, FormField, PopupKind, Screen};
use envault::tui::screens::render_screen;
use envault::tui::{handle_key_event, run_app};
use envault::vault::VaultClient;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn setup_test_env() -> (App, VaultClient) {
    let client = VaultClient::open_in_memory("test-proj", "default", "pass123")
        .expect("open_in_memory should succeed");

    client.set("ALPHA", "alpha_val").unwrap();
    client.set("BETA", "beta_val").unwrap();
    client.set("GAMMA", "gamma_val").unwrap();

    let stored = client.list().unwrap();
    let app = App::new("test-proj".into(), "default".into(), vec![], &stored);
    (app, client)
}

fn create_terminal() -> Terminal<TestBackend> {
    let backend = TestBackend::new(100, 30);
    Terminal::new(backend).expect("terminal creation should succeed")
}

#[test]
fn test_moving_selection() {
    let (mut app, mut client) = setup_test_env();
    let mut terminal = create_terminal();

    // Initial selection is 0 ("ALPHA")
    assert_eq!(app.selected, 0);
    assert_eq!(app.selected_secret().unwrap().key, "ALPHA");

    terminal
        .draw(|f| render_screen(f, &app, f.area()))
        .unwrap();
    let buffer = format!("{:?}", terminal.backend().buffer());
    assert!(buffer.contains("ALPHA"));
    assert!(buffer.contains("▶"));

    // Move down with 'j'
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('j')));
    assert_eq!(app.selected, 1);
    assert_eq!(app.selected_secret().unwrap().key, "BETA");

    // Move down with Down arrow
    handle_key_event(&mut app, &mut client, key(KeyCode::Down));
    assert_eq!(app.selected, 2);
    assert_eq!(app.selected_secret().unwrap().key, "GAMMA");

    // Move down at bottom boundary (stays at 2)
    handle_key_event(&mut app, &mut client, key(KeyCode::Down));
    assert_eq!(app.selected, 2);

    // Move up with 'k'
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('k')));
    assert_eq!(app.selected, 1);
    assert_eq!(app.selected_secret().unwrap().key, "BETA");

    // Move up with Up arrow
    handle_key_event(&mut app, &mut client, key(KeyCode::Up));
    assert_eq!(app.selected, 0);
    assert_eq!(app.selected_secret().unwrap().key, "ALPHA");

    // Move up at top boundary (stays at 0)
    handle_key_event(&mut app, &mut client, key(KeyCode::Up));
    assert_eq!(app.selected, 0);
}

#[test]
fn test_search_bar_focus_and_filter() {
    let (mut app, mut client) = setup_test_env();

    // Focus starts in List
    assert_eq!(app.focus, FocusZone::List);

    // Press '/' to switch to SearchBar
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('/')));
    assert_eq!(app.focus, FocusZone::SearchBar);

    // Type 'b' then 'e'
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('b')));
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('e')));
    assert_eq!(app.search_query, "be");

    let filtered = app.filtered_secrets();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].key, "BETA");

    // Backspace pops char
    handle_key_event(&mut app, &mut client, key(KeyCode::Backspace));
    assert_eq!(app.search_query, "b");

    // Esc clears search and returns focus to List
    handle_key_event(&mut app, &mut client, key(KeyCode::Esc));
    assert_eq!(app.search_query, "");
    assert_eq!(app.focus, FocusZone::List);
    assert_eq!(app.filtered_secrets().len(), 3);

    // Focus again and press Enter to keep query but return focus
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('/')));
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('g')));
    assert_eq!(app.focus, FocusZone::SearchBar);
    assert_eq!(app.search_query, "g");

    handle_key_event(&mut app, &mut client, key(KeyCode::Enter));
    assert_eq!(app.focus, FocusZone::List);
    assert_eq!(app.search_query, "g");
    assert_eq!(app.filtered_secrets().len(), 1);
}

#[test]
fn test_opening_add_form_and_canceling() {
    let (mut app, mut client) = setup_test_env();
    let mut terminal = create_terminal();

    // Press 'a' to open AddSecret
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('a')));
    assert!(matches!(app.screen, Screen::AddSecret(_)));

    terminal
        .draw(|f| render_screen(f, &app, f.area()))
        .unwrap();
    let buffer = format!("{:?}", terminal.backend().buffer());
    assert!(buffer.contains("Add Secret"));

    // Press Esc to cancel
    handle_key_event(&mut app, &mut client, key(KeyCode::Esc));
    assert_eq!(app.screen, Screen::SecretList);
}

#[test]
fn test_submitting_add_form_to_vault() {
    let (mut app, mut client) = setup_test_env();

    // Open add secret
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('a')));

    // Type Key: "NEW_KEY"
    for c in "NEW_KEY".chars() {
        handle_key_event(&mut app, &mut client, key(KeyCode::Char(c)));
    }

    // Try typing invalid characters: '$', '!', '-' (should be ignored for Key)
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('$')));
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('!')));
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('-')));

    if let Screen::AddSecret(ref form) = app.screen {
        assert_eq!(form.key_input, "NEW_KEY");
        assert_eq!(form.focused_field, FormField::Key);
    } else {
        panic!("expected AddSecret screen");
    }

    // Tab to switch field
    handle_key_event(&mut app, &mut client, key(KeyCode::Tab));
    if let Screen::AddSecret(ref form) = app.screen {
        assert_eq!(form.focused_field, FormField::Value);
    }

    // Type Value: "new_secret_val"
    for c in "new_secret_val".chars() {
        handle_key_event(&mut app, &mut client, key(KeyCode::Char(c)));
    }

    // Submit form with Enter
    handle_key_event(&mut app, &mut client, key(KeyCode::Enter));

    // Screen should return to SecretList
    assert_eq!(app.screen, Screen::SecretList);

    // Stored in vault client
    let retrieved = client.get("NEW_KEY").expect("get should succeed");
    assert_eq!(retrieved, Some("new_secret_val".to_string()));

    // Stored in app secrets
    let keys: Vec<String> = app.secrets.iter().map(|s| s.key.clone()).collect();
    assert!(keys.contains(&"NEW_KEY".to_string()));
    assert!(app.status.text.contains("Saved secret 'NEW_KEY'"));
}

#[test]
fn test_add_form_empty_key_does_not_submit() {
    let (mut app, mut client) = setup_test_env();

    // Open add secret
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('a')));

    // Submit while key is empty
    handle_key_event(&mut app, &mut client, key(KeyCode::Enter));

    // Must remain on AddSecret screen
    assert!(matches!(app.screen, Screen::AddSecret(_)));
    assert_eq!(app.status.text, "Key cannot be empty");
}

#[test]
fn test_viewing_secret_value_popup_and_clipboard() {
    let (mut app, mut client) = setup_test_env();
    let mut terminal = create_terminal();

    // Initial selected is ALPHA ("alpha_val")
    assert_eq!(app.selected_secret().unwrap().key, "ALPHA");

    // Press Enter to view secret
    handle_key_event(&mut app, &mut client, key(KeyCode::Enter));
    assert_eq!(
        app.popup,
        Some(PopupKind::ShowValue {
            key: "ALPHA".into(),
            value: zeroize::Zeroizing::new("alpha_val".into()),
        })
    );

    // Render to terminal
    terminal
        .draw(|f| render_screen(f, &app, f.area()))
        .unwrap();
    let buffer = format!("{:?}", terminal.backend().buffer());
    assert!(buffer.contains("alpha_val"));
    assert!(buffer.contains("copy to clipboard"));

    // Press 'c' to copy to clipboard
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('c')));
    assert_eq!(app.status.text, "Copied to clipboard");
    assert!(app.popup.is_some()); // Popup still open

    // Press Esc to close popup
    handle_key_event(&mut app, &mut client, key(KeyCode::Esc));
    assert_eq!(app.popup, None);

    // Open again and close with Enter
    handle_key_event(&mut app, &mut client, key(KeyCode::Enter));
    assert!(app.popup.is_some());
    handle_key_event(&mut app, &mut client, key(KeyCode::Enter));
    assert_eq!(app.popup, None);
}

#[test]
fn test_viewing_missing_secret_shows_error_popup() {
    let mut client = VaultClient::open_in_memory("test-proj", "default", "pass123").unwrap();
    let schema = vec![SchemaEntry {
        key: "UNSET_KEY".into(),
        required: true,
        sensitive: false,
        description: None,
        schema_type: SchemaType::String,
        default_value: None,
    }];
    let mut app = App::new("test-proj".into(), "default".into(), schema, &[]);
    let mut terminal = create_terminal();

    assert_eq!(app.selected_secret().unwrap().key, "UNSET_KEY");
    assert!(!app.selected_secret().unwrap().has_value);

    // Press Enter on missing secret
    handle_key_event(&mut app, &mut client, key(KeyCode::Enter));

    assert_eq!(
        app.popup,
        Some(PopupKind::Error {
            title: "Secret Not Found".into(),
            detail: "Secret 'UNSET_KEY' is not set in vault".into(),
        })
    );

    terminal
        .draw(|f| render_screen(f, &app, f.area()))
        .unwrap();
    let buffer = format!("{:?}", terminal.backend().buffer());
    assert!(buffer.contains("Secret Not Found"));

    // Close error popup with Esc
    handle_key_event(&mut app, &mut client, key(KeyCode::Esc));
    assert_eq!(app.popup, None);
}

#[test]
fn test_editing_secret_value_and_submitting() {
    let (mut app, mut client) = setup_test_env();
    let mut terminal = create_terminal();

    // Select BETA ("beta_val")
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('j')));
    assert_eq!(app.selected_secret().unwrap().key, "BETA");

    // Press 'e' to edit
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('e')));
    assert!(matches!(app.screen, Screen::EditSecret(_)));

    terminal
        .draw(|f| render_screen(f, &app, f.area()))
        .unwrap();
    let buffer = format!("{:?}", terminal.backend().buffer());
    assert!(buffer.contains("Edit Secret: BETA"));
    assert!(buffer.contains("(read-only)"));

    // Type new value "updated_beta"
    for c in "updated_beta".chars() {
        handle_key_event(&mut app, &mut client, key(KeyCode::Char(c)));
    }

    // Submit with Enter
    handle_key_event(&mut app, &mut client, key(KeyCode::Enter));

    // Screen should return to SecretList
    assert_eq!(app.screen, Screen::SecretList);

    // Vault should reflect updated value
    let val = client.get("BETA").unwrap();
    assert_eq!(val, Some("updated_beta".to_string()));
    assert!(app.status.text.contains("Updated secret 'BETA'"));
}

#[test]
fn test_editing_secret_cancel_with_esc() {
    let (mut app, mut client) = setup_test_env();

    // Press 'e'
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('e')));
    assert!(matches!(app.screen, Screen::EditSecret(_)));

    // Press Esc
    handle_key_event(&mut app, &mut client, key(KeyCode::Esc));
    assert_eq!(app.screen, Screen::SecretList);

    // Value unchanged
    assert_eq!(client.get("ALPHA").unwrap(), Some("alpha_val".to_string()));
}

#[test]
fn test_delete_secret_confirmation_and_cancel() {
    let (mut app, mut client) = setup_test_env();
    let mut terminal = create_terminal();

    // Select ALPHA
    assert_eq!(app.selected_secret().unwrap().key, "ALPHA");

    // Press 'd' to open ConfirmDelete
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('d')));
    assert_eq!(
        app.popup,
        Some(PopupKind::ConfirmDelete {
            key: "ALPHA".into()
        })
    );

    terminal
        .draw(|f| render_screen(f, &app, f.area()))
        .unwrap();
    let buffer = format!("{:?}", terminal.backend().buffer());
    assert!(buffer.contains("Confirm Delete"));
    assert!(buffer.contains("Delete secret: ALPHA?"));

    // Press Esc to cancel
    handle_key_event(&mut app, &mut client, key(KeyCode::Esc));
    assert_eq!(app.popup, None);

    // ALPHA still exists
    assert_eq!(client.get("ALPHA").unwrap(), Some("alpha_val".to_string()));
}

#[test]
fn test_delete_secret_confirmation_and_confirm() {
    let (mut app, mut client) = setup_test_env();

    // Select ALPHA
    assert_eq!(app.selected_secret().unwrap().key, "ALPHA");

    // Press 'd'
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('d')));
    assert!(app.popup.is_some());

    // Press Enter to confirm delete
    handle_key_event(&mut app, &mut client, key(KeyCode::Enter));

    // Popup closed, secret deleted
    assert_eq!(app.popup, None);
    assert_eq!(client.get("ALPHA").unwrap(), None);

    let remaining_keys: Vec<String> = app.secrets.iter().map(|s| s.key.clone()).collect();
    assert_eq!(remaining_keys, vec!["BETA", "GAMMA"]);
    assert!(app.status.text.contains("Deleted secret 'ALPHA'"));
}

#[test]
fn test_audit_log_screen_navigation() {
    let (mut app, mut client) = setup_test_env();
    let mut terminal = create_terminal();

    // Press 'A' to open audit log
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('A')));
    assert!(matches!(app.screen, Screen::AuditLog(_)));

    terminal
        .draw(|f| render_screen(f, &app, f.area()))
        .unwrap();
    let buffer = format!("{:?}", terminal.backend().buffer());
    assert!(buffer.contains("Audit Log"));

    // Scroll down with 'j' and Down
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('j')));
    handle_key_event(&mut app, &mut client, key(KeyCode::Down));

    // Scroll up with 'k' and Up
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('k')));
    handle_key_event(&mut app, &mut client, key(KeyCode::Up));

    // Jump bottom and top
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('G')));
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('g')));

    // Press 'q' to return to list
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('q')));
    assert_eq!(app.screen, Screen::SecretList);

    // Open again and return with Esc
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('A')));
    assert!(matches!(app.screen, Screen::AuditLog(_)));
    handle_key_event(&mut app, &mut client, key(KeyCode::Esc));
    assert_eq!(app.screen, Screen::SecretList);
}

#[test]
fn test_quitting_application() {
    let (mut app, mut client) = setup_test_env();

    assert_eq!(app.mode, AppMode::Running);

    // Press 'q' to quit
    handle_key_event(&mut app, &mut client, key(KeyCode::Char('q')));
    assert_eq!(app.mode, AppMode::Quit);
}

#[test]
fn test_run_app_exits_when_quit_mode() {
    let (mut app, mut client) = setup_test_env();
    let mut terminal = create_terminal();

    app.mode = AppMode::Quit;

    // run_app should immediately terminate when mode is Quit without polling crossterm
    let result = run_app(&mut terminal, &mut app, &mut client);
    assert!(result.is_ok());
}
