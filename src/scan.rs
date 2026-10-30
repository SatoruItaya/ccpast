use std::fs;
use std::path::{Path, PathBuf};

/// Return the absolute path of the root `~/.claude/projects/` directory,
/// resolving `~` via the home directory provided by `dirs::home_dir`.
/// Returns `None` if the home directory cannot be determined.
pub fn projects_root() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".claude").join("projects"))
}

/// List `<root>/<project-dir>/*.jsonl`. Deeper files (e.g.
/// `<session-id>/subagents/agent-*.jsonl`) are subagent transcripts, not
/// sessions, so they are not visited.
/// Missing roots return an empty vector — never an error.
pub fn list_session_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(projects) = fs::read_dir(root) else {
        return out;
    };
    for project in projects.flatten() {
        let Ok(entries) = fs::read_dir(project.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_jsonl(&path) {
                out.push(path);
            }
        }
    }
    out
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

    #[test]
    fn skips_nested_subagent_transcripts() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let proj = root.join("-proj-a");
        let subagents = proj.join("aaaa").join("subagents");
        fs::create_dir_all(&subagents).unwrap();
        fs::write(proj.join("aaaa.jsonl"), "").unwrap();
        fs::write(subagents.join("agent-x.jsonl"), "").unwrap();

        let found = list_session_files(root);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("aaaa.jsonl"));
    }
}
