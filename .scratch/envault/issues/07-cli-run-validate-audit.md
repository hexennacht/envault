# 07: CLI Run, Validate, and Audit Commands

**What to build:** The execution and auditing CLI commands: `envault run -- <command>` which decrypts and injects secrets as environment variables into child processes; `envault validate` which compares current secrets against `.env.schema`; and `envault audit` which outputs access logs.

**Blocked by:** 05: Schema Validator, 06: CLI Core Commands

**Status:** done

- [x] `envault run -- <cmd>` implemented in `src/cli/run.rs` matching SPEC §9.3.
- [x] Child process receives decrypted environment variables and returns matching exit code.
- [x] `envault validate` implemented in `src/cli/validate.rs` matching SPEC §9.4, reporting OK / MISSING / INVALID and exiting with code 1 on failure.
- [x] `envault audit` implemented in `src/cli/audit.rs` formatting audit events as a table.
- [x] Integration tests verifying child environment injection and schema validation status codes.
