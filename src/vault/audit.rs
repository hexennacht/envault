use std::fmt;
use std::str::FromStr;

use anyhow::Context;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Error returned when parsing an unrecognized audit action string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseAuditActionError(pub String);

impl fmt::Display for ParseAuditActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown audit action: '{}'", self.0)
    }
}

impl std::error::Error for ParseAuditActionError {}

/// Audit actions supported by envault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AuditAction {
    Read,
    Write,
    Delete,
}

impl AuditAction {
    /// Action name formatted for database persistence and display.
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditAction::Read => "READ",
            AuditAction::Write => "WRITE",
            AuditAction::Delete => "DELETE",
        }
    }
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for AuditAction {
    type Err = ParseAuditActionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "READ" => Ok(AuditAction::Read),
            "WRITE" => Ok(AuditAction::Write),
            "DELETE" => Ok(AuditAction::Delete),
            other => Err(ParseAuditActionError(other.to_string())),
        }
    }
}

/// Audit log record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub project: String,
    pub scope: String,
    pub key: String,
    pub action: AuditAction,
    pub at: i64,
}

/// Record an audit event in the database with the current Unix timestamp.
pub fn record_audit(
    conn: &Connection,
    project: &str,
    scope: &str,
    key: &str,
    action: AuditAction,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().timestamp();
    record_audit_at(conn, project, scope, key, action, now)
}

/// Record an audit event in the database with an explicit timestamp.
pub fn record_audit_at(
    conn: &Connection,
    project: &str,
    scope: &str,
    key: &str,
    action: AuditAction,
    at: i64,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO audit_log (project, scope, key, action, at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![project, scope, key, action.as_str(), at],
    )
    .with_context(|| {
        format!(
            "failed to record audit entry: project='{project}', scope='{scope}', key='{key}', action='{action}'"
        )
    })?;
    Ok(())
}

/// Query audit log entries for a given project, sorted newest first.
pub fn get_audit_log(conn: &Connection, project: &str) -> anyhow::Result<Vec<AuditEntry>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, project, scope, key, action, at FROM audit_log WHERE project = ?1 ORDER BY at DESC, id DESC",
        )
        .context("failed to prepare audit log select query")?;

    let entries = stmt
        .query_map(rusqlite::params![project], |row| {
            let action_str: String = row.get(4)?;
            let action = action_str.parse::<AuditAction>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
            })?;
            Ok(AuditEntry {
                id: row.get(0)?,
                project: row.get(1)?,
                scope: row.get(2)?,
                key: row.get(3)?,
                action,
                at: row.get(5)?,
            })
        })
        .context("failed to execute audit log query")?
        .collect::<Result<Vec<AuditEntry>, _>>()
        .context("failed to read audit log entries")?;

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::db::open_in_memory;

    #[test]
    fn test_record_and_get_audit_log() {
        let conn = open_in_memory().expect("open should succeed");

        record_audit_at(&conn, "my-app", "development", "DATABASE_URL", AuditAction::Write, 100).unwrap();
        record_audit_at(&conn, "my-app", "development", "DATABASE_URL", AuditAction::Read, 200).unwrap();
        record_audit_at(&conn, "my-app", "development", "OLD_SECRET", AuditAction::Delete, 300).unwrap();

        // Other project entries
        record_audit_at(&conn, "other-project", "default", "SECRET", AuditAction::Write, 250).unwrap();

        let logs = get_audit_log(&conn, "my-app").expect("get_audit_log should succeed");
        assert_eq!(logs.len(), 3);

        // Verify sorted newest first (at DESC)
        assert_eq!(logs[0].key, "OLD_SECRET");
        assert_eq!(logs[0].action, AuditAction::Delete);
        assert_eq!(logs[0].at, 300);

        assert_eq!(logs[1].key, "DATABASE_URL");
        assert_eq!(logs[1].action, AuditAction::Read);
        assert_eq!(logs[1].at, 200);

        assert_eq!(logs[2].key, "DATABASE_URL");
        assert_eq!(logs[2].action, AuditAction::Write);
        assert_eq!(logs[2].at, 100);

        // Verify other project has its own log
        let other_logs = get_audit_log(&conn, "other-project").unwrap();
        assert_eq!(other_logs.len(), 1);
        assert_eq!(other_logs[0].key, "SECRET");
    }

    #[test]
    fn test_audit_action_parsing_and_display() {
        assert_eq!(AuditAction::Read.to_string(), "READ");
        assert_eq!(AuditAction::Write.to_string(), "WRITE");
        assert_eq!(AuditAction::Delete.to_string(), "DELETE");

        assert_eq!("READ".parse::<AuditAction>().unwrap(), AuditAction::Read);
        assert_eq!("read".parse::<AuditAction>().unwrap(), AuditAction::Read);
        assert_eq!("WRITE".parse::<AuditAction>().unwrap(), AuditAction::Write);
        assert_eq!("write".parse::<AuditAction>().unwrap(), AuditAction::Write);
        assert_eq!("DELETE".parse::<AuditAction>().unwrap(), AuditAction::Delete);
        assert_eq!("delete".parse::<AuditAction>().unwrap(), AuditAction::Delete);

        assert!("UNKNOWN".parse::<AuditAction>().is_err());
    }
}
