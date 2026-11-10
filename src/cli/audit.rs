use std::path::Path;
use anyhow::Context;

use crate::vault::audit::AuditEntry;
use crate::vault::VaultClient;

/// Formats a list of audit entries as a clean aligned table matching SPEC §8.8 / §9.1.
pub fn format_audit_log(entries: &[AuditEntry]) -> String {
    let mut out = String::new();
    let scope_width = entries
        .iter()
        .map(|e| e.scope.len())
        .max()
        .unwrap_or(5)
        .max(11);

    out.push_str(&format!(
        "{:<19}   {:<scope_width$}   {:<6}   {}\n",
        "Time", "Scope", "Action", "Key"
    ));

    for entry in entries {
        let time_str = chrono::DateTime::from_timestamp(entry.at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "0000-00-00 00:00:00".to_string());

        out.push_str(&format!(
            "{:<19}   {:<scope_width$}   {:<6}   {}\n",
            time_str,
            entry.scope,
            entry.action.as_str(),
            entry.key
        ));
    }

    out
}

/// Handles the `audit` subcommand by retrieving the audit log for the current project
/// and printing it formatted as a clean table to stdout.
pub fn handle_audit(
    db_path: &Path,
    project: &str,
    scope: &str,
    password: &str,
) -> anyhow::Result<()> {
    let client = VaultClient::open(db_path, project, scope, password)
        .with_context(|| format!("failed to open vault database at {}", db_path.display()))?;

    let entries = client
        .get_audit_log()
        .with_context(|| format!("failed to retrieve audit log for project '{project}'"))?;

    let table = format_audit_log(&entries);
    print!("{table}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::audit::AuditAction;
    use tempfile::tempdir;

    #[test]
    fn test_format_audit_log() {
        let entries = vec![
            AuditEntry {
                id: 1,
                project: "my-app".to_string(),
                scope: "development".to_string(),
                key: "DATABASE_URL".to_string(),
                action: AuditAction::Read,
                at: 1736937121, // 2025-01-15 10:32:01 UTC
            },
            AuditEntry {
                id: 2,
                project: "my-app".to_string(),
                scope: "development".to_string(),
                key: "STRIPE_SECRET_KEY".to_string(),
                action: AuditAction::Write,
                at: 1736937045, // 2025-01-15 10:30:45 UTC
            },
            AuditEntry {
                id: 3,
                project: "my-app".to_string(),
                scope: "default".to_string(),
                key: "OLD_API_KEY".to_string(),
                action: AuditAction::Delete,
                at: 1736932320, // 2025-01-15 09:12:00 UTC
            },
        ];

        let table = format_audit_log(&entries);
        assert!(table.contains("Time                  Scope         Action   Key"));
        assert!(table.contains("2025-01-15 10:32:01   development   READ     DATABASE_URL"));
        assert!(table.contains("2025-01-15 10:30:45   development   WRITE    STRIPE_SECRET_KEY"));
        assert!(table.contains("2025-01-15 09:12:00   default       DELETE   OLD_API_KEY"));
    }

    #[test]
    fn test_handle_audit_end_to_end() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("vault.db");
        let project = "audit-proj";
        let scope = "default";
        let password = "pass";

        let client = VaultClient::open(&db_path, project, scope, password).unwrap();
        client.set("KEY1", "VAL1").unwrap();
        client.get("KEY1").unwrap();
        client.delete("KEY1").unwrap();

        let entries = client.get_audit_log().unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].action, AuditAction::Delete);
        assert_eq!(entries[1].action, AuditAction::Read);
        assert_eq!(entries[2].action, AuditAction::Write);

        // handle_audit should succeed without error
        handle_audit(&db_path, project, scope, password).unwrap();
    }
}
