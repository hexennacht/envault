# 05: Schema Validator

**What to build:** The schema validation engine that verifies stored secrets against the rules parsed from `.env.schema`. It checks required fields and validates data types (string, port, url, bool, integer, enum).

**Blocked by:** 02: Schema Parser and Types

**Status:** ready-for-agent

- [ ] `validate(entries: &[SchemaEntry], secrets: &HashMap<String, String>) -> Vec<ValidationError>` in `src/schema/validator.rs` matching SPEC §7.2 and §11.
- [ ] Validates presence for `@required` entries.
- [ ] Validates type constraints: `string` (non-empty), `port` (1-65535), `url` (http/https), `bool` (`true`/`false`), `integer` (valid whole number), and `enum` (allowed members).
- [ ] Returns clear `ValidationError` structs with error messages.
- [ ] Unit tests for valid configurations and all error conditions.
