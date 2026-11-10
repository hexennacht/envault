use crate::schema::types::{SchemaEntry, SchemaType};
use anyhow::{Context, Result, bail};
use std::path::Path;

#[derive(Default)]
struct EntryBuilder {
    required: bool,
    sensitive: bool,
    schema_type: Option<SchemaType>,
    description_lines: Vec<String>,
}

impl EntryBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn reset(&mut self) {
        self.required = false;
        self.sensitive = false;
        self.schema_type = None;
        self.description_lines.clear();
    }

    fn description(&self) -> Option<String> {
        if self.description_lines.is_empty() {
            None
        } else {
            let joined = self.description_lines.join("\n");
            if joined.trim().is_empty() {
                None
            } else {
                Some(joined)
            }
        }
    }
}

fn tokenize_decorators(input: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        while i < n && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= n {
            break;
        }

        if chars[i] != '@' {
            let rest: String = chars[i..].iter().collect();
            bail!("expected decorator starting with '@', found: '{}'", rest);
        }

        let start = i;
        let mut in_parens = 0;

        while i < n {
            let c = chars[i];
            if c == '(' {
                in_parens += 1;
            } else if c == ')' {
                if in_parens == 0 {
                    bail!(
                        "unexpected closing parenthesis in decorator line: '{}'",
                        input
                    );
                }
                in_parens -= 1;
            } else if c.is_whitespace() && in_parens == 0 {
                break;
            }
            i += 1;
        }

        if in_parens > 0 {
            bail!("unclosed parenthesis in decorator: '{}'", input);
        }

        let token: String = chars[start..i].iter().collect();
        tokens.push(token);
    }

    Ok(tokens)
}

fn parse_type(type_str: &str) -> Result<SchemaType> {
    let type_str = type_str.trim();
    match type_str {
        "string" => Ok(SchemaType::String),
        "port" => Ok(SchemaType::Port),
        "url" => Ok(SchemaType::Url),
        "bool" => Ok(SchemaType::Bool),
        "integer" => Ok(SchemaType::Integer),
        s if s.starts_with("enum(") && s.ends_with(')') => {
            let inside = &s[5..s.len() - 1];
            let mut variants = Vec::new();
            for item in inside.split(',') {
                let trimmed = item.trim();
                let unquoted = if (trimmed.starts_with('"')
                    && trimmed.ends_with('"')
                    && trimmed.len() >= 2)
                    || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
                {
                    &trimmed[1..trimmed.len() - 1]
                } else {
                    trimmed
                };
                if unquoted.is_empty() {
                    bail!("empty enum variant in: '{}'", s);
                }
                variants.push(unquoted.to_string());
            }
            if variants.is_empty() {
                bail!("enum must have at least one variant: '{}'", s);
            }
            Ok(SchemaType::Enum(variants))
        }
        _ => bail!("unknown or invalid schema type: '{}'", type_str),
    }
}

fn parse_decorators(input: &str, builder: &mut EntryBuilder) -> Result<()> {
    let tokens = tokenize_decorators(input)?;
    for token in tokens {
        if token == "@required" {
            builder.required = true;
        } else if token == "@sensitive" {
            builder.sensitive = true;
        } else if let Some(type_str) = token.strip_prefix("@type=") {
            if type_str.is_empty() {
                bail!("missing type specification in '@type='");
            }
            builder.schema_type = Some(parse_type(type_str)?);
        } else if token == "@type" {
            bail!("missing value for @type decorator, expected @type=<type>");
        } else {
            bail!("unknown decorator: '{}'", token);
        }
    }
    Ok(())
}

/// Parses a schema from a string according to SPEC §7.
pub fn parse_schema_str(content: &str) -> Result<Vec<SchemaEntry>> {
    let mut entries = Vec::new();
    let mut builder = EntryBuilder::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Rule 1: Skip blank lines
        if trimmed.is_empty() {
            continue;
        }

        // Rule 2 & 3: Comments and Decorators
        if let Some(rest) = trimmed.strip_prefix('#') {
            let comment_body = rest.trim_start();
            if comment_body.starts_with('@') {
                // Rule 2: Collect decorator annotations
                parse_decorators(comment_body, &mut builder)
                    .map_err(|e| anyhow::anyhow!("line {}: {}", line_num + 1, e))?;
            } else {
                // Rule 3: Collect as a description comment
                let desc_line = rest.strip_prefix(' ').unwrap_or(rest).trim_end();
                builder.description_lines.push(desc_line.to_string());
            }
            continue;
        }

        // Rule 4: If the line contains '=', create a SchemaEntry with any collected annotations
        if let Some((raw_key, raw_val)) = line.split_once('=') {
            let key = raw_key.trim();
            if key.is_empty() {
                bail!("line {}: empty key name before '='", line_num + 1);
            }
            if key.chars().any(|c| c.is_whitespace()) {
                bail!(
                    "line {}: key name contains whitespace: '{}'",
                    line_num + 1,
                    key
                );
            }

            let val = raw_val.trim();
            let default_value = if val.is_empty() {
                None
            } else if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
                || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2)
            {
                Some(val[1..val.len() - 1].to_string())
            } else {
                Some(val.to_string())
            };

            let entry = SchemaEntry {
                key: key.to_string(),
                required: builder.required,
                sensitive: builder.sensitive,
                description: builder.description(),
                schema_type: builder.schema_type.take().unwrap_or(SchemaType::String),
                default_value,
            };

            entries.push(entry);

            // Rule 5: Reset the collected annotations after each entry
            builder.reset();
            continue;
        }

        // If line is neither blank, nor comment, nor contains '=': syntax error
        bail!(
            "line {}: invalid schema line (expected 'KEY=' or '#'): '{}'",
            line_num + 1,
            line
        );
    }

    Ok(entries)
}

/// Parses a schema from a file path according to SPEC §7.
pub fn parse_schema_file(path: &Path) -> Result<Vec<SchemaEntry>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read schema file: {}", path.display()))?;
    parse_schema_str(&content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_empty() {
        let entries = parse_schema_str("").unwrap();
        assert!(entries.is_empty());

        let entries = parse_schema_str("   \n\n  \t  \n").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_only_comments_and_blank_lines() {
        let content = r#"
        # This is a schema file
        # No entries here

        # Another comment
        "#;
        let entries = parse_schema_str(content).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_minimal_entry() {
        let content = "KEY=\n";
        let entries = parse_schema_str(content).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "KEY");
        assert!(!entries[0].required);
        assert!(!entries[0].sensitive);
        assert_eq!(entries[0].description, None);
        assert_eq!(entries[0].schema_type, SchemaType::String);
        assert_eq!(entries[0].default_value, None);
    }

    #[test]
    fn test_parse_entry_with_default() {
        let content = "PORT=8080\n";
        let entries = parse_schema_str(content).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "PORT");
        assert_eq!(entries[0].default_value, Some("8080".to_string()));
    }

    #[test]
    fn test_parse_quoted_defaults() {
        let content = r#"
        DOUBLE="hello world"
        SINGLE='hello world'
        EMPTY_DOUBLE=""
        EMPTY_SINGLE=''
        "#;
        let entries = parse_schema_str(content).unwrap();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].default_value, Some("hello world".to_string()));
        assert_eq!(entries[1].default_value, Some("hello world".to_string()));
        assert_eq!(entries[2].default_value, Some("".to_string()));
        assert_eq!(entries[3].default_value, Some("".to_string()));
    }

    #[test]
    fn test_parse_description_single_line() {
        let content = r#"
        # TCP port for the server
        PORT=8080
        "#;
        let entries = parse_schema_str(content).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].description,
            Some("TCP port for the server".to_string())
        );
    }

    #[test]
    fn test_parse_description_multi_line() {
        let content = r#"
        # Application environment flag.
        # Must be development, test, or production.
        APP_ENV=development
        "#;
        let entries = parse_schema_str(content).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].description,
            Some(
                "Application environment flag.\nMust be development, test, or production."
                    .to_string()
            )
        );
    }

    #[test]
    fn test_parse_required_and_sensitive_decorators() {
        let content = r#"
        # @required
        API_KEY=

        # @sensitive
        PASSWORD=

        # @required @sensitive
        SECRET_TOKEN=
        "#;
        let entries = parse_schema_str(content).unwrap();
        assert_eq!(entries.len(), 3);

        assert!(entries[0].required);
        assert!(!entries[0].sensitive);

        assert!(!entries[1].required);
        assert!(entries[1].sensitive);

        assert!(entries[2].required);
        assert!(entries[2].sensitive);
    }

    #[test]
    fn test_parse_all_schema_types() {
        let content = r#"
        # @type=string
        T_STRING=

        # @type=port
        T_PORT=8080

        # @type=url
        T_URL=https://example.com

        # @type=bool
        T_BOOL=true

        # @type=integer
        T_INT=42

        # @type=enum(dev,test,prod)
        T_ENUM=dev

        # @type=enum(foo, bar, baz)
        T_ENUM_SPACED=foo

        # @type=enum("alpha", "beta")
        T_ENUM_QUOTED=alpha
        "#;
        let entries = parse_schema_str(content).unwrap();
        assert_eq!(entries.len(), 8);

        assert_eq!(entries[0].schema_type, SchemaType::String);
        assert_eq!(entries[1].schema_type, SchemaType::Port);
        assert_eq!(entries[2].schema_type, SchemaType::Url);
        assert_eq!(entries[3].schema_type, SchemaType::Bool);
        assert_eq!(entries[4].schema_type, SchemaType::Integer);
        assert_eq!(
            entries[5].schema_type,
            SchemaType::Enum(vec![
                "dev".to_string(),
                "test".to_string(),
                "prod".to_string()
            ])
        );
        assert_eq!(
            entries[6].schema_type,
            SchemaType::Enum(vec![
                "foo".to_string(),
                "bar".to_string(),
                "baz".to_string()
            ])
        );
        assert_eq!(
            entries[7].schema_type,
            SchemaType::Enum(vec!["alpha".to_string(), "beta".to_string()])
        );
    }

    #[test]
    fn test_parse_multiple_decorators_separate_lines() {
        let content = r#"
        # Port for the server
        # @required
        # @sensitive
        # @type=port
        PORT=3000
        "#;
        let entries = parse_schema_str(content).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "PORT");
        assert!(entries[0].required);
        assert!(entries[0].sensitive);
        assert_eq!(entries[0].schema_type, SchemaType::Port);
        assert_eq!(
            entries[0].description,
            Some("Port for the server".to_string())
        );
        assert_eq!(entries[0].default_value, Some("3000".to_string()));
    }

    #[test]
    fn test_parse_spec_example() {
        let content = r#"
# Application environment flag
# @required @type=enum(development,test,production)
APP_ENV=development

# TCP port for the API server
# @required @type=port
API_PORT=8080

# Stripe secret key
# @required @sensitive @type=string
STRIPE_SECRET_KEY=
"#;
        let entries = parse_schema_str(content).unwrap();
        assert_eq!(entries.len(), 3);

        // Entry 1: APP_ENV
        assert_eq!(entries[0].key, "APP_ENV");
        assert!(entries[0].required);
        assert!(!entries[0].sensitive);
        assert_eq!(
            entries[0].description,
            Some("Application environment flag".to_string())
        );
        assert_eq!(
            entries[0].schema_type,
            SchemaType::Enum(vec![
                "development".to_string(),
                "test".to_string(),
                "production".to_string()
            ])
        );
        assert_eq!(entries[0].default_value, Some("development".to_string()));

        // Entry 2: API_PORT
        assert_eq!(entries[1].key, "API_PORT");
        assert!(entries[1].required);
        assert!(!entries[1].sensitive);
        assert_eq!(
            entries[1].description,
            Some("TCP port for the API server".to_string())
        );
        assert_eq!(entries[1].schema_type, SchemaType::Port);
        assert_eq!(entries[1].default_value, Some("8080".to_string()));

        // Entry 3: STRIPE_SECRET_KEY
        assert_eq!(entries[2].key, "STRIPE_SECRET_KEY");
        assert!(entries[2].required);
        assert!(entries[2].sensitive);
        assert_eq!(
            entries[2].description,
            Some("Stripe secret key".to_string())
        );
        assert_eq!(entries[2].schema_type, SchemaType::String);
        assert_eq!(entries[2].default_value, None);
    }

    #[test]
    fn test_parse_resets_annotations_between_entries() {
        let content = r#"
        # Important secret
        # @required @sensitive @type=integer
        ENTRY_ONE=42

        ENTRY_TWO=
        "#;
        let entries = parse_schema_str(content).unwrap();
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].key, "ENTRY_ONE");
        assert!(entries[0].required);
        assert!(entries[0].sensitive);
        assert_eq!(entries[0].schema_type, SchemaType::Integer);
        assert_eq!(entries[0].description, Some("Important secret".to_string()));
        assert_eq!(entries[0].default_value, Some("42".to_string()));

        assert_eq!(entries[1].key, "ENTRY_TWO");
        assert!(!entries[1].required);
        assert!(!entries[1].sensitive);
        assert_eq!(entries[1].schema_type, SchemaType::String);
        assert_eq!(entries[1].description, None);
        assert_eq!(entries[1].default_value, None);
    }

    #[test]
    fn test_parse_whitespace_tolerance() {
        let content = "   API_PORT   =   8080   \n";
        let entries = parse_schema_str(content).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "API_PORT");
        assert_eq!(entries[0].default_value, Some("8080".to_string()));
    }

    #[test]
    fn test_parse_value_with_equals() {
        let content = "DATABASE_URL=postgres://user:pass@localhost:5432/db?ssl=true\n";
        let entries = parse_schema_str(content).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "DATABASE_URL");
        assert_eq!(
            entries[0].default_value,
            Some("postgres://user:pass@localhost:5432/db?ssl=true".to_string())
        );
    }

    #[test]
    fn test_parse_schema_file() {
        let dir = std::env::temp_dir();
        let file_path = dir.join(format!(
            "envault_test_schema_{}.env.schema",
            std::process::id()
        ));

        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "# Test port").unwrap();
        writeln!(file, "# @required @type=port").unwrap();
        writeln!(file, "PORT=9000").unwrap();
        drop(file);

        let entries = parse_schema_file(&file_path).unwrap();
        let _ = std::fs::remove_file(&file_path);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "PORT");
        assert!(entries[0].required);
        assert_eq!(entries[0].schema_type, SchemaType::Port);
        assert_eq!(entries[0].default_value, Some("9000".to_string()));
    }

    #[test]
    fn test_parse_schema_file_not_found() {
        let non_existent = Path::new("/path/to/definitely/non_existent_file_12345.schema");
        let result = parse_schema_file(non_existent);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_missing_equals() {
        let content = "JUST_A_STRING\n";
        let result = parse_schema_str(content);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("invalid schema line"));
    }

    #[test]
    fn test_error_empty_key() {
        let content = "=value\n";
        let result = parse_schema_str(content);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("empty key name"));
    }

    #[test]
    fn test_error_key_contains_whitespace() {
        let content = "FOO BAR=value\n";
        let result = parse_schema_str(content);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("contains whitespace"));
    }

    #[test]
    fn test_error_unknown_decorator() {
        let content = "# @unknown\nKEY=\n";
        let result = parse_schema_str(content);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("unknown decorator"));
    }

    #[test]
    fn test_error_unknown_type() {
        let content = "# @type=unknown\nKEY=\n";
        let result = parse_schema_str(content);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("unknown or invalid schema type"));
    }

    #[test]
    fn test_error_type_missing_value() {
        let content = "# @type\nKEY=\n";
        let result = parse_schema_str(content);
        assert!(result.is_err());

        let content2 = "# @type=\nKEY=\n";
        let result2 = parse_schema_str(content2);
        assert!(result2.is_err());
    }

    #[test]
    fn test_error_enum_unclosed_paren() {
        let content = "# @type=enum(a,b\nKEY=\n";
        let result = parse_schema_str(content);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("unclosed parenthesis"));
    }

    #[test]
    fn test_error_enum_empty() {
        let content = "# @type=enum()\nKEY=\n";
        let result = parse_schema_str(content);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("empty enum variant"));
    }

    #[test]
    fn test_error_enum_empty_variant() {
        let content = "# @type=enum(a,,b)\nKEY=\n";
        let result = parse_schema_str(content);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("empty enum variant"));
    }

    #[test]
    fn test_error_decorator_line_junk() {
        let content = "# @required extra_junk\nKEY=\n";
        let result = parse_schema_str(content);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("expected decorator starting with '@'"));
    }
}
