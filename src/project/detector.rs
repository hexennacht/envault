use std::path::{Path, PathBuf};
use anyhow::Result;

pub const MARKERS: &[&str] = &["Cargo.toml", "package.json", "go.mod", ".envault"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInfo {
    pub root: PathBuf,
    pub name: String,
}

pub fn detect_project(start_dir: &Path) -> Result<ProjectInfo> {
    let start_dir_buf;
    let start: &Path = if start_dir.is_relative() {
        start_dir_buf = match std::env::current_dir() {
            Ok(cwd) => {
                if start_dir == Path::new(".") || start_dir.as_os_str().is_empty() {
                    cwd
                } else {
                    cwd.join(start_dir)
                }
            }
            Err(_) => start_dir.to_path_buf(),
        };
        &start_dir_buf
    } else {
        start_dir
    };

    for dir in start.ancestors() {
        if dir.as_os_str().is_empty() {
            continue;
        }

        for marker in MARKERS {
            if dir.join(marker).exists() {
                let name = extract_project_name(dir);
                return Ok(ProjectInfo {
                    root: dir.to_path_buf(),
                    name,
                });
            }
        }
    }

    Ok(ProjectInfo {
        root: start_dir.to_path_buf(),
        name: extract_project_name(start_dir),
    })
}

/// Alias for [`detect_project`] matching SPEC §13.1 testing plan.
pub fn detect(start_dir: &Path) -> Result<ProjectInfo> {
    detect_project(start_dir)
}

fn extract_project_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            path.canonicalize().ok().and_then(|p| {
                p.file_name()
                    .and_then(|name| name.to_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            })
        })
        .unwrap_or_else(|| {
            let s = path.to_string_lossy();
            if s.is_empty() {
                "unknown".to_string()
            } else {
                s.into_owned()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_detect_cargo_toml_in_start_dir() -> Result<()> {
        let temp = tempdir()?;
        let project_dir = temp.path().join("my-rust-project");
        fs::create_dir(&project_dir)?;
        fs::write(project_dir.join("Cargo.toml"), "[package]\nname = \"my-rust-project\"")?;

        let info = detect_project(&project_dir)?;
        assert_eq!(info.root, project_dir);
        assert_eq!(info.name, "my-rust-project");
        Ok(())
    }

    #[test]
    fn test_detect_nested_traversal_cargo_toml() -> Result<()> {
        let temp = tempdir()?;
        let project_dir = temp.path().join("my-workspace-app");
        let deep_dir = project_dir.join("src").join("nested").join("deep");
        fs::create_dir_all(&deep_dir)?;
        fs::write(project_dir.join("Cargo.toml"), "")?;

        let info = detect_project(&deep_dir)?;
        assert_eq!(info.root, project_dir);
        assert_eq!(info.name, "my-workspace-app");
        Ok(())
    }

    #[test]
    fn test_detect_package_json() -> Result<()> {
        let temp = tempdir()?;
        let project_dir = temp.path().join("my-node-app");
        let nested_dir = project_dir.join("packages").join("frontend");
        fs::create_dir_all(&nested_dir)?;
        fs::write(project_dir.join("package.json"), "{}")?;

        let info = detect_project(&nested_dir)?;
        assert_eq!(info.root, project_dir);
        assert_eq!(info.name, "my-node-app");
        Ok(())
    }

    #[test]
    fn test_detect_go_mod() -> Result<()> {
        let temp = tempdir()?;
        let project_dir = temp.path().join("my-go-service");
        let nested_dir = project_dir.join("cmd").join("server");
        fs::create_dir_all(&nested_dir)?;
        fs::write(project_dir.join("go.mod"), "module my-go-service")?;

        let info = detect_project(&nested_dir)?;
        assert_eq!(info.root, project_dir);
        assert_eq!(info.name, "my-go-service");
        Ok(())
    }

    #[test]
    fn test_detect_envault_file() -> Result<()> {
        let temp = tempdir()?;
        let project_dir = temp.path().join("my-envault-file-app");
        let nested_dir = project_dir.join("internal").join("config");
        fs::create_dir_all(&nested_dir)?;
        fs::write(project_dir.join(".envault"), "")?;

        let info = detect_project(&nested_dir)?;
        assert_eq!(info.root, project_dir);
        assert_eq!(info.name, "my-envault-file-app");
        Ok(())
    }

    #[test]
    fn test_detect_envault_dir() -> Result<()> {
        let temp = tempdir()?;
        let project_dir = temp.path().join("my-envault-dir-app");
        let nested_dir = project_dir.join("sub");
        fs::create_dir_all(&nested_dir)?;
        fs::create_dir_all(project_dir.join(".envault"))?;

        let info = detect_project(&nested_dir)?;
        assert_eq!(info.root, project_dir);
        assert_eq!(info.name, "my-envault-dir-app");
        Ok(())
    }

    #[test]
    fn test_detect_nested_marker_precedence() -> Result<()> {
        let temp = tempdir()?;
        let outer_dir = temp.path().join("monorepo");
        let inner_dir = outer_dir.join("services").join("auth");
        let deep_dir = inner_dir.join("src");
        fs::create_dir_all(&deep_dir)?;

        fs::write(outer_dir.join("package.json"), "{}")?;
        fs::write(inner_dir.join("Cargo.toml"), "")?;

        let info = detect_project(&deep_dir)?;
        assert_eq!(info.root, inner_dir);
        assert_eq!(info.name, "auth");
        Ok(())
    }

    #[test]
    fn test_detect_fallback_no_marker() -> Result<()> {
        let temp = tempdir()?;
        let plain_dir = temp.path().join("standalone-folder");
        fs::create_dir(&plain_dir)?;

        let info = detect_project(&plain_dir)?;
        assert_eq!(info.root, plain_dir);
        assert_eq!(info.name, "standalone-folder");
        Ok(())
    }

    #[test]
    fn test_detect_alias() -> Result<()> {
        let temp = tempdir()?;
        let project_dir = temp.path().join("alias-test");
        fs::create_dir(&project_dir)?;
        fs::write(project_dir.join("Cargo.toml"), "")?;

        let info = detect(&project_dir)?;
        assert_eq!(info.root, project_dir);
        assert_eq!(info.name, "alias-test");
        Ok(())
    }
}
