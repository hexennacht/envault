use std::collections::HashSet;

use crate::schema::types::SchemaEntry;
use crate::vault::audit::AuditEntry;

/// Running state of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Running,
    Quit,
}

/// Active screen view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    SecretList,
    AddSecret(AddSecretForm),
    EditSecret(EditSecretForm),
    AuditLog(AuditLogState),
}

/// Input focus zone within the SecretList screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusZone {
    List,
    SearchBar,
}

/// Form field selection for AddSecret screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    Key,
    Value,
}

/// State for the AddSecret screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddSecretForm {
    pub key_input: String,
    pub value_input: String,
    pub focused_field: FormField,
    pub cursor_pos: usize,
}

impl AddSecretForm {
    pub fn new() -> Self {
        Self {
            key_input: String::new(),
            value_input: String::new(),
            focused_field: FormField::Key,
            cursor_pos: 0,
        }
    }

    /// Toggle focus between Key and Value fields.
    pub fn toggle_focus(&mut self) {
        self.focused_field = match self.focused_field {
            FormField::Key => {
                self.cursor_pos = self.value_input.len();
                FormField::Value
            }
            FormField::Value => {
                self.cursor_pos = self.key_input.len();
                FormField::Key
            }
        };
    }
}

impl Default for AddSecretForm {
    fn default() -> Self {
        Self::new()
    }
}

/// State for the EditSecret screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditSecretForm {
    pub key: String,
    pub value_input: String,
    pub cursor_pos: usize,
}

impl EditSecretForm {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value_input: String::new(),
            cursor_pos: 0,
        }
    }
}

/// State for the scrollable AuditLog screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogState {
    pub entries: Vec<AuditEntry>,
    pub scroll_offset: usize,
    pub selected: usize,
}

impl AuditLogState {
    pub fn new(entries: Vec<AuditEntry>) -> Self {
        Self {
            entries,
            scroll_offset: 0,
            selected: 0,
        }
    }

    /// Move selection up one row.
    pub fn scroll_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            if self.selected < self.scroll_offset {
                self.scroll_offset = self.selected;
            }
        }
    }

    /// Move selection down one row.
    pub fn scroll_down(&mut self, visible_height: usize) {
        if self.entries.is_empty() {
            return;
        }
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
            if visible_height > 0 && self.selected >= self.scroll_offset + visible_height {
                self.scroll_offset = self.selected + 1 - visible_height;
            }
        }
    }

    /// Jump to the first entry.
    pub fn scroll_to_top(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Jump to the last entry.
    pub fn scroll_to_bottom(&mut self, visible_height: usize) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = self.entries.len() - 1;
        if visible_height > 0 && self.selected >= visible_height {
            self.scroll_offset = self.selected + 1 - visible_height;
        } else {
            self.scroll_offset = 0;
        }
    }
}

/// Modal popup overlay variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PopupKind {
    ConfirmDelete { key: String },
    ShowValue { key: String, value: zeroize::Zeroizing<String> },
    Error { title: String, detail: String },
}

/// Severity level for status bar notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Info,
    Error,
}

/// Status bar state displayed at the bottom of the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBar {
    pub text: String,
    pub level: StatusLevel,
}

impl StatusBar {
    pub fn new(text: impl Into<String>, level: StatusLevel) -> Self {
        Self {
            text: text.into(),
            level,
        }
    }

    pub fn info(text: impl Into<String>) -> Self {
        Self::new(text, StatusLevel::Info)
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self::new(text, StatusLevel::Error)
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self {
            text: String::new(),
            level: StatusLevel::Info,
        }
    }
}

/// Single row item in the secret list view.
/// Never stores plaintext secret values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretMeta {
    pub key: String,
    pub required: bool,
    pub sensitive: bool,
    pub has_value: bool,
}

impl SecretMeta {
    pub fn new(key: impl Into<String>, required: bool, sensitive: bool, has_value: bool) -> Self {
        Self {
            key: key.into(),
            required,
            sensitive,
            has_value,
        }
    }
}

/// Top-level application state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub mode: AppMode,
    pub screen: Screen,
    pub secrets: Vec<SecretMeta>,
    pub selected: usize,
    pub search_query: String,
    pub focus: FocusZone,
    pub project: String,
    pub scope: String,
    pub schema: Vec<SchemaEntry>,
    pub status: StatusBar,
    pub popup: Option<PopupKind>,
}

impl App {
    /// Construct a new App instance by merging schema entries with stored secret keys.
    ///
    /// Preserves schema ordering for schema keys, then appends any stored keys that are
    /// not present in the schema.
    pub fn new(
        project: String,
        scope: String,
        schema: Vec<SchemaEntry>,
        stored_keys: &[String],
    ) -> Self {
        let secrets = Self::merge_secrets(&schema, stored_keys);
        Self {
            mode: AppMode::Running,
            screen: Screen::SecretList,
            secrets,
            selected: 0,
            search_query: String::new(),
            focus: FocusZone::List,
            project,
            scope,
            schema,
            status: StatusBar::default(),
            popup: None,
        }
    }

    /// Merge schema entries and stored keys into a unified SecretMeta list.
    pub fn merge_secrets(schema: &[SchemaEntry], stored_keys: &[String]) -> Vec<SecretMeta> {
        let mut secrets = Vec::new();
        let mut seen = HashSet::new();
        let stored_set: HashSet<&str> = stored_keys.iter().map(|s| s.as_str()).collect();

        // Schema entries first, preserving schema order
        for entry in schema {
            if seen.insert(entry.key.clone()) {
                secrets.push(SecretMeta {
                    key: entry.key.clone(),
                    required: entry.required,
                    sensitive: entry.sensitive,
                    has_value: stored_set.contains(entry.key.as_str()),
                });
            }
        }

        // Stored keys that are not defined in schema
        for key in stored_keys {
            if seen.insert(key.clone()) {
                secrets.push(SecretMeta {
                    key: key.clone(),
                    required: false,
                    sensitive: false,
                    has_value: true,
                });
            }
        }

        secrets
    }

    /// Reload the secrets list using updated stored keys from the vault.
    pub fn reload_secrets(&mut self, stored_keys: &[String]) {
        self.secrets = Self::merge_secrets(&self.schema, stored_keys);
        self.clamp_selection();
    }

    /// Return filtered references to SecretMeta based on current search_query (case-insensitive substring).
    pub fn filtered_secrets(&self) -> Vec<&SecretMeta> {
        if self.search_query.is_empty() {
            self.secrets.iter().collect()
        } else {
            let needle = self.search_query.to_lowercase();
            self.secrets
                .iter()
                .filter(|secret| secret.key.to_lowercase().contains(&needle))
                .collect()
        }
    }

    /// Return the currently selected SecretMeta, if any.
    pub fn selected_secret(&self) -> Option<&SecretMeta> {
        self.filtered_secrets().get(self.selected).copied()
    }

    /// Move selection cursor up one item.
    pub fn move_selection_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection cursor down one item.
    pub fn move_selection_down(&mut self) {
        let count = self.filtered_secrets().len();
        if count > 0 && self.selected + 1 < count {
            self.selected += 1;
        }
    }

    /// Clamp the selected index to ensure it is within bounds of filtered_secrets.
    pub fn clamp_selection(&mut self) {
        let count = self.filtered_secrets().len();
        if count == 0 {
            self.selected = 0;
        } else if self.selected >= count {
            self.selected = count - 1;
        }
    }

    /// Set search query and re-clamp selection.
    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
        self.clamp_selection();
    }

    /// Clear the search query and re-clamp selection.
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.clamp_selection();
    }

    /// Transition to AddSecret screen.
    pub fn open_add_secret(&mut self) {
        self.screen = Screen::AddSecret(AddSecretForm::new());
    }

    /// Transition to EditSecret screen for the specified key.
    pub fn open_edit_secret(&mut self, key: impl Into<String>) {
        self.screen = Screen::EditSecret(EditSecretForm::new(key));
    }

    /// Transition to EditSecret screen for the currently selected secret.
    pub fn open_edit_selected(&mut self) -> bool {
        if let Some(secret) = self.selected_secret() {
            let key = secret.key.clone();
            self.open_edit_secret(key);
            true
        } else {
            false
        }
    }

    /// Open ConfirmDelete modal popup for the specified key.
    pub fn open_confirm_delete(&mut self, key: impl Into<String>) {
        self.popup = Some(PopupKind::ConfirmDelete { key: key.into() });
    }

    /// Open ConfirmDelete modal popup for the currently selected secret.
    pub fn open_confirm_delete_selected(&mut self) -> bool {
        if let Some(secret) = self.selected_secret() {
            let key = secret.key.clone();
            self.open_confirm_delete(key);
            true
        } else {
            false
        }
    }

    /// Open ShowValue modal popup with the decrypted secret value.
    pub fn open_show_value(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.popup = Some(PopupKind::ShowValue {
            key: key.into(),
            value: zeroize::Zeroizing::new(value.into()),
        });
    }

    /// Open Error modal popup with title and detail messages.
    pub fn show_error(&mut self, title: impl Into<String>, detail: impl Into<String>) {
        self.popup = Some(PopupKind::Error {
            title: title.into(),
            detail: detail.into(),
        });
    }

    /// Dismiss active modal popup and zeroize secret value if present.
    pub fn close_popup(&mut self) {
        if let Some(PopupKind::ShowValue { ref mut value, .. }) = self.popup {
            use zeroize::Zeroize;
            value.zeroize();
        }
        self.popup = None;
    }

    /// Transition to AuditLog screen.
    pub fn open_audit_log(&mut self, entries: Vec<AuditEntry>) {
        self.screen = Screen::AuditLog(AuditLogState::new(entries));
    }

    /// Return to SecretList screen.
    pub fn return_to_list(&mut self) {
        self.screen = Screen::SecretList;
    }

    /// Set input focus zone on SecretList screen.
    pub fn set_focus(&mut self, focus: FocusZone) {
        self.focus = focus;
    }

    /// Set status message and level.
    pub fn set_status(&mut self, text: impl Into<String>, level: StatusLevel) {
        self.status = StatusBar::new(text, level);
    }

    /// Set status info message.
    pub fn set_status_info(&mut self, text: impl Into<String>) {
        self.status = StatusBar::info(text);
    }

    /// Set status error message.
    pub fn set_status_error(&mut self, text: impl Into<String>) {
        self.status = StatusBar::error(text);
    }

    /// Request application quit.
    pub fn quit(&mut self) {
        self.mode = AppMode::Quit;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::types::{SchemaEntry, SchemaType};
    use crate::vault::audit::AuditAction;

    fn make_schema_entry(key: &str, required: bool, sensitive: bool) -> SchemaEntry {
        SchemaEntry {
            key: key.to_string(),
            required,
            sensitive,
            description: None,
            schema_type: SchemaType::String,
            default_value: None,
        }
    }

    #[test]
    fn test_schema_merging() {
        let schema = vec![
            make_schema_entry("DATABASE_URL", true, true),
            make_schema_entry("STRIPE_SECRET_KEY", true, true),
            make_schema_entry("APP_ENV", true, false),
        ];

        let stored_keys = vec![
            "DATABASE_URL".to_string(),
            "APP_ENV".to_string(),
            "CUSTOM_API_KEY".to_string(),
        ];

        let app = App::new(
            "test-project".into(),
            "development".into(),
            schema,
            &stored_keys,
        );

        assert_eq!(app.secrets.len(), 4);

        // First schema entry: present in stored_keys
        assert_eq!(
            app.secrets[0],
            SecretMeta {
                key: "DATABASE_URL".into(),
                required: true,
                sensitive: true,
                has_value: true,
            }
        );

        // Second schema entry: missing from stored_keys
        assert_eq!(
            app.secrets[1],
            SecretMeta {
                key: "STRIPE_SECRET_KEY".into(),
                required: true,
                sensitive: true,
                has_value: false,
            }
        );

        // Third schema entry: present in stored_keys
        assert_eq!(
            app.secrets[2],
            SecretMeta {
                key: "APP_ENV".into(),
                required: true,
                sensitive: false,
                has_value: true,
            }
        );

        // Stored key not in schema: appended with default metadata
        assert_eq!(
            app.secrets[3],
            SecretMeta {
                key: "CUSTOM_API_KEY".into(),
                required: false,
                sensitive: false,
                has_value: true,
            }
        );
    }

    #[test]
    fn test_schema_merging_deduplication() {
        let schema = vec![
            make_schema_entry("DATABASE_URL", true, true),
            make_schema_entry("DATABASE_URL", false, false),
        ];
        let stored = vec!["DATABASE_URL".to_string(), "DATABASE_URL".to_string()];

        let secrets = App::merge_secrets(&schema, &stored);
        assert_eq!(secrets.len(), 1);
        assert_eq!(secrets[0].key, "DATABASE_URL");
        assert!(secrets[0].required);
        assert!(secrets[0].has_value);
    }

    #[test]
    fn test_reload_secrets() {
        let schema = vec![make_schema_entry("DATABASE_URL", true, false)];
        let stored_initial: Vec<String> = vec![];

        let mut app = App::new(
            "my-app".into(),
            "default".into(),
            schema,
            &stored_initial,
        );

        assert!(!app.secrets[0].has_value);

        let stored_updated = vec!["DATABASE_URL".to_string(), "PORT".to_string()];
        app.reload_secrets(&stored_updated);

        assert_eq!(app.secrets.len(), 2);
        assert!(app.secrets[0].has_value);
        assert_eq!(app.secrets[1].key, "PORT");
        assert!(app.secrets[1].has_value);
    }

    #[test]
    fn test_filtered_secrets() {
        let schema = vec![
            make_schema_entry("DB_HOST", true, true),
            make_schema_entry("DB_PASSWORD", true, true),
            make_schema_entry("PORT", true, false),
        ];
        let app = App::new("test".into(), "test".into(), schema, &[]);

        // Empty query matches all
        let all = app.filtered_secrets();
        assert_eq!(all.len(), 3);

        // Case-insensitive match "db" matches DB_HOST and DB_PASSWORD
        let mut app_filtered = app.clone();
        app_filtered.set_search_query("db");
        let matched = app_filtered.filtered_secrets();
        assert_eq!(matched.len(), 2);
        assert_eq!(matched[0].key, "DB_HOST");
        assert_eq!(matched[1].key, "DB_PASSWORD");

        // Exact match "PORT"
        app_filtered.set_search_query("port");
        let port_match = app_filtered.filtered_secrets();
        assert_eq!(port_match.len(), 1);
        assert_eq!(port_match[0].key, "PORT");

        // Non-matching query
        app_filtered.set_search_query("nonexistent");
        assert!(app_filtered.filtered_secrets().is_empty());
    }

    #[test]
    fn test_selection_movement_and_clamping() {
        let schema = vec![
            make_schema_entry("KEY_1", false, false),
            make_schema_entry("KEY_2", false, false),
            make_schema_entry("KEY_3", false, false),
        ];
        let mut app = App::new("test".into(), "test".into(), schema, &[]);

        assert_eq!(app.selected, 0);

        // Move down within bounds
        app.move_selection_down();
        assert_eq!(app.selected, 1);
        app.move_selection_down();
        assert_eq!(app.selected, 2);

        // Move down at boundary
        app.move_selection_down();
        assert_eq!(app.selected, 2);

        // Move up within bounds
        app.move_selection_up();
        assert_eq!(app.selected, 1);
        app.move_selection_up();
        assert_eq!(app.selected, 0);

        // Move up at boundary
        app.move_selection_up();
        assert_eq!(app.selected, 0);

        // Selection clamping when filtering narrows list
        app.selected = 2;
        app.set_search_query("KEY_1");
        assert_eq!(app.filtered_secrets().len(), 1);
        assert_eq!(app.selected, 0);

        // Selection clamping on empty search result
        app.set_search_query("NO_MATCH");
        assert_eq!(app.filtered_secrets().len(), 0);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn test_state_transitions() {
        let schema = vec![make_schema_entry("DATABASE_URL", true, true)];
        let stored = vec!["DATABASE_URL".to_string()];
        let mut app = App::new("proj".into(), "scope".into(), schema, &stored);

        assert_eq!(app.screen, Screen::SecretList);
        assert_eq!(app.mode, AppMode::Running);
        assert_eq!(app.popup, None);
        assert_eq!(app.focus, FocusZone::List);

        // Open AddSecret
        app.open_add_secret();
        assert_eq!(
            app.screen,
            Screen::AddSecret(AddSecretForm {
                key_input: String::new(),
                value_input: String::new(),
                focused_field: FormField::Key,
                cursor_pos: 0,
            })
        );

        // Return to SecretList
        app.return_to_list();
        assert_eq!(app.screen, Screen::SecretList);

        // Open EditSecret
        app.open_edit_secret("DATABASE_URL");
        assert_eq!(
            app.screen,
            Screen::EditSecret(EditSecretForm {
                key: "DATABASE_URL".into(),
                value_input: String::new(),
                cursor_pos: 0,
            })
        );

        // Return to list
        app.return_to_list();

        // Open Edit selected
        assert!(app.open_edit_selected());
        assert_eq!(
            app.screen,
            Screen::EditSecret(EditSecretForm {
                key: "DATABASE_URL".into(),
                value_input: String::new(),
                cursor_pos: 0,
            })
        );
        app.return_to_list();

        // Open ConfirmDelete
        app.open_confirm_delete("DATABASE_URL");
        assert_eq!(
            app.popup,
            Some(PopupKind::ConfirmDelete {
                key: "DATABASE_URL".into()
            })
        );

        // Close popup
        app.close_popup();
        assert_eq!(app.popup, None);

        // Open ConfirmDelete on selected
        assert!(app.open_confirm_delete_selected());
        assert_eq!(
            app.popup,
            Some(PopupKind::ConfirmDelete {
                key: "DATABASE_URL".into()
            })
        );
        app.close_popup();

        // Open ShowValue
        app.open_show_value("DATABASE_URL", "postgres://localhost/db");
        assert_eq!(
            app.popup,
            Some(PopupKind::ShowValue {
                key: "DATABASE_URL".into(),
                value: zeroize::Zeroizing::new("postgres://localhost/db".into())
            })
        );
        app.close_popup();

        // Show Error
        app.show_error("Decryption Failed", "Invalid auth tag");
        assert_eq!(
            app.popup,
            Some(PopupKind::Error {
                title: "Decryption Failed".into(),
                detail: "Invalid auth tag".into()
            })
        );
        app.close_popup();

        // Open AuditLog
        let entries = vec![AuditEntry {
            id: 1,
            project: "proj".into(),
            scope: "scope".into(),
            key: "DATABASE_URL".into(),
            action: AuditAction::Read,
            at: 1700000000,
        }];
        app.open_audit_log(entries.clone());
        assert_eq!(
            app.screen,
            Screen::AuditLog(AuditLogState {
                entries,
                scroll_offset: 0,
                selected: 0,
            })
        );
        app.return_to_list();

        // Set focus
        app.set_focus(FocusZone::SearchBar);
        assert_eq!(app.focus, FocusZone::SearchBar);

        // Set status
        app.set_status_info("Copied to clipboard");
        assert_eq!(
            app.status,
            StatusBar {
                text: "Copied to clipboard".into(),
                level: StatusLevel::Info,
            }
        );

        app.set_status_error("Failed to save secret");
        assert_eq!(
            app.status,
            StatusBar {
                text: "Failed to save secret".into(),
                level: StatusLevel::Error,
            }
        );

        // Quit
        app.quit();
        assert_eq!(app.mode, AppMode::Quit);
    }

    #[test]
    fn test_add_secret_form_toggle_focus() {
        let mut form = AddSecretForm::new();
        form.key_input = "MY_KEY".into();
        form.value_input = "MY_VAL".into();
        assert_eq!(form.focused_field, FormField::Key);

        form.toggle_focus();
        assert_eq!(form.focused_field, FormField::Value);
        assert_eq!(form.cursor_pos, 6); // Length of "MY_VAL"

        form.toggle_focus();
        assert_eq!(form.focused_field, FormField::Key);
        assert_eq!(form.cursor_pos, 6); // Length of "MY_KEY"
    }

    #[test]
    fn test_audit_log_state_scrolling() {
        let entries = (0..10)
            .map(|i| AuditEntry {
                id: i,
                project: "proj".into(),
                scope: "scope".into(),
                key: format!("KEY_{i}"),
                action: AuditAction::Write,
                at: 1700000000 + i,
            })
            .collect();

        let mut state = AuditLogState::new(entries);
        assert_eq!(state.selected, 0);
        assert_eq!(state.scroll_offset, 0);

        // Scroll down
        state.scroll_down(5);
        assert_eq!(state.selected, 1);
        assert_eq!(state.scroll_offset, 0);

        // Scroll to bottom
        state.scroll_to_bottom(5);
        assert_eq!(state.selected, 9);
        assert_eq!(state.scroll_offset, 5);

        // Scroll up
        state.scroll_up();
        assert_eq!(state.selected, 8);

        // Scroll to top
        state.scroll_to_top();
        assert_eq!(state.selected, 0);
        assert_eq!(state.scroll_offset, 0);
    }
}
