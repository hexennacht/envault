# 10: TUI Event Loop and Integration

**What to build:** The terminal lifecycle engine and top-level integration. It initializes crossterm in alternate screen raw mode, runs the tick and event polling loop, dispatches keystrokes according to the SPEC §6.3 call graph, guarantees terminal cleanup on exit or panic, and routes bare `envault` to the TUI.

**Blocked by:** 07: CLI Run, Validate, and Audit Commands, 09: TUI Screens

**Status:** done

- [x] `src/tui/mod.rs` initializes and cleanly restores terminal backend using crossterm / ratatui.
- [x] Event loop executes poll, read, and render matching the design graph in SPEC §6.3.
- [x] Keystroke dispatch routes to popup first when active, otherwise active screen handler.
- [x] Bare `envault` launches TUI; subcommands execute CLI commands.
- [x] Complete test suite passes with zero warnings (`cargo test`, `cargo clippy`).
