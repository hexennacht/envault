use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::schema::types::{SchemaEntry, SchemaType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationError {
    pub key: String,
    pub rule: String,
    pub message: String,
}

impl ValidationError {
    pub fn new(
        key: impl Into<String>,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            rule: rule.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.key, self.message)
    }
}

impl std::error::Error for ValidationError {}

pub fn validate(
    entries: &[SchemaEntry],
    secrets: &HashMap<String, String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for entry in entries {
        let secret_val = secrets.get(&entry.key);
        let is_missing = match secret_val {
            None => true,
            Some(val) => val.is_empty(),
        };

        let mut required_error_emitted = false;
        if entry.required && is_missing {
            let has_default = entry
                .default_value
                .as_deref()
                .is_some_and(|d| !d.is_empty());
            if !has_default {
                errors.push(ValidationError {
                    key: entry.key.clone(),
                    rule: "required".to_string(),
                    message: "Secret is required but not set".to_string(),
                });
                required_error_emitted = true;
            }
        }

        if let (Some(val), false) = (secret_val, required_error_emitted) {
            validate_type(&entry.key, &entry.schema_type, val, &mut errors);
        }
    }

    errors
}

fn validate_type(
    key: &str,
    schema_type: &SchemaType,
    val: &str,
    errors: &mut Vec<ValidationError>,
) {
    match schema_type {
        SchemaType::String => {
            if val.is_empty() {
                errors.push(ValidationError {
                    key: key.to_string(),
                    rule: "string".to_string(),
                    message: "expected non-empty string, got empty value".to_string(),
                });
            }
        }
        SchemaType::Port => match val.parse::<u32>() {
            Ok(port) if (1..=65535).contains(&port) => {}
            _ => {
                errors.push(ValidationError {
                    key: key.to_string(),
                    rule: "port".to_string(),
                    message: format!("expected port (1-65535), got \"{}\"", val),
                });
            }
        },
        SchemaType::Url => {
            if !val.starts_with("http://") && !val.starts_with("https://") {
                errors.push(ValidationError {
                    key: key.to_string(),
                    rule: "url".to_string(),
                    message: format!(
                        "expected URL starting with http:// or https://, got \"{}\"",
                        val
                    ),
                });
            }
        }
        SchemaType::Bool => {
            if !val.eq_ignore_ascii_case("true") && !val.eq_ignore_ascii_case("false") {
                errors.push(ValidationError {
                    key: key.to_string(),
                    rule: "bool".to_string(),
                    message: format!("expected bool (true or false), got \"{}\"", val),
                });
            }
        }
        SchemaType::Integer => {
            if val.parse::<i64>().is_err() {
                errors.push(ValidationError {
                    key: key.to_string(),
                    rule: "integer".to_string(),
                    message: format!("expected integer, got \"{}\"", val),
                });
            }
        }
        SchemaType::Enum(variants) => {
            if !variants.iter().any(|v| v == val) {
                errors.push(ValidationError {
                    key: key.to_string(),
                    rule: "enum".to_string(),
                    message: format!(
                        "expected one of [{}], got \"{}\"",
                        variants.join(", "),
                        val
                    ),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(key: &str, required: bool, schema_type: SchemaType) -> SchemaEntry {
        SchemaEntry {
            key: key.to_string(),
            required,
            sensitive: false,
            description: None,
            schema_type,
            default_value: None,
        }
    }

    #[test]
    fn test_missing_required_key() {
        let entries = vec![make_entry("DATABASE_URL", true, SchemaType::String)];
        let secrets = HashMap::new();

        let errors = validate(&entries, &secrets);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].key, "DATABASE_URL");
        assert_eq!(errors[0].rule, "required");
        assert_eq!(errors[0].message, "Secret is required but not set");
    }

    #[test]
    fn test_missing_required_key_empty_value() {
        let entries = vec![make_entry("API_KEY", true, SchemaType::String)];
        let mut secrets = HashMap::new();
        secrets.insert("API_KEY".to_string(), "".to_string());

        let errors = validate(&entries, &secrets);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].key, "API_KEY");
        assert_eq!(errors[0].rule, "required");
        assert_eq!(errors[0].message, "Secret is required but not set");
    }

    #[test]
    fn test_required_key_missing_with_default_value_treated_as_satisfied() {
        let mut entry = make_entry("APP_ENV", true, SchemaType::Enum(vec!["development".into(), "production".into()]));
        entry.default_value = Some("development".to_string());
        let entries = vec![entry];
        let secrets = HashMap::new();

        let errors = validate(&entries, &secrets);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_required_key_present_passes() {
        let entries = vec![make_entry("API_KEY", true, SchemaType::String)];
        let mut secrets = HashMap::new();
        secrets.insert("API_KEY".to_string(), "sk-secret-123".to_string());

        let errors = validate(&entries, &secrets);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_optional_keys_absent_passes() {
        let entries = vec![
            make_entry("PORT", false, SchemaType::Port),
            make_entry("URL", false, SchemaType::Url),
            make_entry("DEBUG", false, SchemaType::Bool),
            make_entry("RETRIES", false, SchemaType::Integer),
            make_entry("ENV", false, SchemaType::Enum(vec!["staging".into()])),
        ];
        let secrets = HashMap::new();

        let errors = validate(&entries, &secrets);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_port_validation() {
        let entries = vec![make_entry("PORT", false, SchemaType::Port)];

        // Valid port: 8080
        let mut secrets = HashMap::new();
        secrets.insert("PORT".to_string(), "8080".to_string());
        assert!(validate(&entries, &secrets).is_empty());

        // Valid edge cases: 1 and 65535
        secrets.insert("PORT".to_string(), "1".to_string());
        assert!(validate(&entries, &secrets).is_empty());

        secrets.insert("PORT".to_string(), "65535".to_string());
        assert!(validate(&entries, &secrets).is_empty());

        // Invalid port: 0
        secrets.insert("PORT".to_string(), "0".to_string());
        let errors = validate(&entries, &secrets);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "port");
        assert_eq!(errors[0].message, "expected port (1-65535), got \"0\"");

        // Invalid port: 70000
        secrets.insert("PORT".to_string(), "70000".to_string());
        let errors = validate(&entries, &secrets);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "port");
        assert_eq!(errors[0].message, "expected port (1-65535), got \"70000\"");

        // Invalid port: "abc"
        secrets.insert("PORT".to_string(), "abc".to_string());
        let errors = validate(&entries, &secrets);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "port");
        assert_eq!(errors[0].message, "expected port (1-65535), got \"abc\"");

        // Invalid port: "-1"
        secrets.insert("PORT".to_string(), "-1".to_string());
        let errors = validate(&entries, &secrets);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "port");
    }

    #[test]
    fn test_url_validation() {
        let entries = vec![make_entry("ENDPOINT", false, SchemaType::Url)];

        // Valid URLs
        let mut secrets = HashMap::new();
        secrets.insert("ENDPOINT".to_string(), "https://example.com".to_string());
        assert!(validate(&entries, &secrets).is_empty());

        secrets.insert("ENDPOINT".to_string(), "http://localhost:8080/api".to_string());
        assert!(validate(&entries, &secrets).is_empty());

        // Invalid URL: "ftp://..."
        secrets.insert("ENDPOINT".to_string(), "ftp://example.com".to_string());
        let errors = validate(&entries, &secrets);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "url");
        assert_eq!(
            errors[0].message,
            "expected URL starting with http:// or https://, got \"ftp://example.com\""
        );

        // Invalid URL: plain domain without scheme
        secrets.insert("ENDPOINT".to_string(), "example.com".to_string());
        let errors = validate(&entries, &secrets);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "url");
    }

    #[test]
    fn test_bool_validation() {
        let entries = vec![make_entry("FLAG", false, SchemaType::Bool)];

        let valid_cases = ["true", "false", "TRUE", "FALSE", "True", "False"];
        for case in valid_cases {
            let mut secrets = HashMap::new();
            secrets.insert("FLAG".to_string(), case.to_string());
            assert!(
                validate(&entries, &secrets).is_empty(),
                "expected '{}' to be valid bool",
                case
            );
        }

        let invalid_cases = ["invalid", "1", "0", "yes", "no"];
        for case in invalid_cases {
            let mut secrets = HashMap::new();
            secrets.insert("FLAG".to_string(), case.to_string());
            let errors = validate(&entries, &secrets);
            assert_eq!(errors.len(), 1, "expected '{}' to fail bool validation", case);
            assert_eq!(errors[0].rule, "bool");
            assert_eq!(
                errors[0].message,
                format!("expected bool (true or false), got \"{}\"", case)
            );
        }
    }

    #[test]
    fn test_integer_validation() {
        let entries = vec![make_entry("COUNT", false, SchemaType::Integer)];

        let valid_cases = ["123", "-4", "0", "9223372036854775807", "-9223372036854775808"];
        for case in valid_cases {
            let mut secrets = HashMap::new();
            secrets.insert("COUNT".to_string(), case.to_string());
            assert!(
                validate(&entries, &secrets).is_empty(),
                "expected '{}' to be valid integer",
                case
            );
        }

        let invalid_cases = ["12.3", "abc", "", "1e5", "9999999999999999999999999999"];
        for case in invalid_cases {
            let mut secrets = HashMap::new();
            secrets.insert("COUNT".to_string(), case.to_string());
            let errors = validate(&entries, &secrets);
            assert_eq!(errors.len(), 1, "expected '{}' to fail integer validation", case);
            assert_eq!(errors[0].rule, "integer");
            assert_eq!(
                errors[0].message,
                format!("expected integer, got \"{}\"", case)
            );
        }
    }

    #[test]
    fn test_enum_validation() {
        let entries = vec![make_entry(
            "ENV",
            false,
            SchemaType::Enum(vec!["development".to_string(), "production".to_string()]),
        )];

        // Valid variants
        let mut secrets = HashMap::new();
        secrets.insert("ENV".to_string(), "development".to_string());
        assert!(validate(&entries, &secrets).is_empty());

        secrets.insert("ENV".to_string(), "production".to_string());
        assert!(validate(&entries, &secrets).is_empty());

        // Invalid: "staging"
        secrets.insert("ENV".to_string(), "staging".to_string());
        let errors = validate(&entries, &secrets);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "enum");
        assert_eq!(
            errors[0].message,
            "expected one of [development, production], got \"staging\""
        );

        // Invalid: case-sensitive check ("DEVELOPMENT")
        secrets.insert("ENV".to_string(), "DEVELOPMENT".to_string());
        let errors = validate(&entries, &secrets);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "enum");
    }

    #[test]
    fn test_string_validation() {
        let entries = vec![make_entry("NAME", false, SchemaType::String)];

        let mut secrets = HashMap::new();
        secrets.insert("NAME".to_string(), "envault".to_string());
        assert!(validate(&entries, &secrets).is_empty());

        secrets.insert("NAME".to_string(), "".to_string());
        let errors = validate(&entries, &secrets);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, "string");
        assert_eq!(errors[0].message, "expected non-empty string, got empty value");
    }

    #[test]
    fn test_multiple_entries_validation() {
        let entries = vec![
            make_entry("DB_URL", true, SchemaType::Url),
            make_entry("PORT", true, SchemaType::Port),
            make_entry("LOG_LEVEL", false, SchemaType::Enum(vec!["debug".into(), "info".into(), "error".into()])),
        ];

        let mut secrets = HashMap::new();
        // DB_URL missing (required)
        // PORT has invalid port
        secrets.insert("PORT".to_string(), "invalid_port".to_string());
        // LOG_LEVEL has invalid variant
        secrets.insert("LOG_LEVEL".to_string(), "warn".to_string());

        let errors = validate(&entries, &secrets);
        assert_eq!(errors.len(), 3);

        assert_eq!(errors[0].key, "DB_URL");
        assert_eq!(errors[0].rule, "required");

        assert_eq!(errors[1].key, "PORT");
        assert_eq!(errors[1].rule, "port");

        assert_eq!(errors[2].key, "LOG_LEVEL");
        assert_eq!(errors[2].rule, "enum");
    }
}
