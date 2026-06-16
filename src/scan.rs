#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

/// Return the absolute path of the root `~/.claude/projects/` directory,
/// resolving `~` via the home directory provided by `dirs::home_dir`.
/// Returns `None` if the home directory cannot be determined.
pub fn projects_root() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".claude").join("projects"))
}

/// Recursively list every `*.jsonl` file under `root` (exactly one level deep
/// is sufficient in practice but we still walk recursively for safety).
/// Missing roots return an empty vector — never an error.
pub fn list_session_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_dir(&path, &mut out);
        } else if is_jsonl(&path) {
            out.push(path);
        }
    }
    out
}

fn visit_dir(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_dir(&path, out);
        } else if is_jsonl(&path) {
            out.push(path);
        }
    }
}

fn is_jsonl(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()) == Some("jsonl")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn missing_root_returns_empty() {
        let tmp = tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        assert!(list_session_files(&missing).is_empty());
    }

    #[test]
    fn lists_jsonl_files_and_ignores_others() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let proj_a = root.join("-proj-a");
        let proj_b = root.join("-proj-b");
        fs::create_dir_all(&proj_a).unwrap();
        fs::create_dir_all(&proj_b).unwrap();
        fs::write(proj_a.join("aaaa.jsonl"), "").unwrap();
        fs::write(proj_a.join("notes.txt"), "ignored").unwrap();
        fs::write(proj_b.join("bbbb.jsonl"), "").unwrap();

        let mut found = list_session_files(root);
        found.sort();
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|p| p.ends_with("aaaa.jsonl")));
        assert!(found.iter().any(|p| p.ends_with("bbbb.jsonl")));
    }
}
