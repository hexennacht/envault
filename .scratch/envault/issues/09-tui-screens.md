# 09: TUI Screens

**What to build:** The dedicated screen views for the terminal UI: the SecretList main view with schema status badges, the AddSecret form with masked input, the EditSecret form with locked key name, and the scrollable AuditLog table.

**Blocked by:** 08: TUI App State and Components

**Status:** done

- [x] `SecretList` screen in `src/tui/screens/secret_list.rs` showing keys, `● set` vs `○ missing`, and schema metadata badges.
- [x] `AddSecret` screen in `src/tui/screens/add_secret.rs` with Key validation `[A-Za-z0-9_]`, masked `█` value input, and Tab field navigation.
- [x] `EditSecret` screen in `src/tui/screens/edit_secret.rs` with read-only key name and masked value input.
- [x] `AuditLog` screen in `src/tui/screens/audit_log.rs` with scrollable table and j/k/g/G navigation.
- [x] Tests verifying screen rendering logic without terminal panics.
