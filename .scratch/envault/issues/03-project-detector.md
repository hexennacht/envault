# 03: Project Detector

**What to build:** Project context detection engine that discovers the root directory and name of the current project by walking up the directory tree until it finds a marker file (`Cargo.toml`, `package.json`, `go.mod`, or `.envault`).

**Blocked by:** None (can start immediately)

**Status:** implemented

- [x] `detect_project(start_dir: &Path) -> anyhow::Result<ProjectInfo>` implemented in `src/project/detector.rs` matching SPEC §4.1 and §11.
- [x] Returns directory path and inferred project name (using directory name).
- [x] Falls back gracefully to the starting directory name if no marker is found.
- [x] Unit tests for finding marker files across nested directories.
