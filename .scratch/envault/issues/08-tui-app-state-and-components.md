# 08: TUI App State and Components

**What to build:** The TUI state machine and reusable UI widgets according to the Design Graph Thinking architecture defined in SPEC §6.3 and §8. Implements `App`, `Screen`, `FocusZone`, `PopupKind`, and modular components for popup, search bar, and status bar.

**Blocked by:** 04: Vault Client and Keychain, 05: Schema Validator

**Status:** done

- [x] `App` state struct and state enums in `src/tui/app.rs` matching SPEC §6.3.
- [x] Single source of truth for key routing using `FocusZone` (List | SearchBar) and `popup: Option<PopupKind>`.
- [x] Centered modal popup widget in `src/tui/components/popup.rs` supporting ConfirmDelete, ShowValue, and Error.
- [x] Search input component in `src/tui/components/search_bar.rs`.
- [x] Status bar component in `src/tui/components/status_bar.rs`.
- [x] Unit tests for state transitions and search filtering.
