use std::fs;
use std::path::{Path, PathBuf};

/// The directory Claude Code uses when `CLAUDE_CONFIG_DIR` is unset.
pub fn default_config_dir() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".claude"))
}

/// Every Claude Code config directory to scan: `~/.claude`, each `~/.claude-*`
/// that has a `projects/` subdirectory, and `$CLAUDE_CONFIG_DIR`. People who
/// switch accounts per directory (e.g. via direnv) keep sessions in several of
/// these, and ccpast is usually launched from a shell where only one is active.
pub fn config_dirs() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let env_dir = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from);
    discover_config_dirs(&home, env_dir)
}

fn discover_config_dirs(home: &Path, env_dir: Option<PathBuf>) -> Vec<PathBuf> {
    let mut candidates = vec![home.join(".claude")];
    if let Ok(entries) = fs::read_dir(home) {
        let mut extra: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(".claude-"))
            })
            .filter(|p| p.join("projects").is_dir())
            .collect();
        extra.sort();
        candidates.extend(extra);
    }
    candidates.extend(env_dir);

    let mut seen = Vec::new();
    let mut out = Vec::new();
    for dir in candidates {
        let key = fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
        if !seen.contains(&key) {
            seen.push(key);
            out.push(dir);
        }
    }
    out
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
    fn discovers_default_and_prefixed_config_dirs() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        fs::create_dir_all(home.join(".claude-work").join("projects")).unwrap();
        fs::create_dir_all(home.join(".claude-empty")).unwrap();
        fs::create_dir_all(home.join(".config")).unwrap();

        let dirs = discover_config_dirs(home, None);
        assert_eq!(dirs, vec![home.join(".claude"), home.join(".claude-work")]);
    }

    #[test]
    fn includes_env_config_dir_without_duplicates() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        let work = home.join(".claude-work");
        let custom = home.join("custom-claude");
        fs::create_dir_all(work.join("projects")).unwrap();
        fs::create_dir_all(&custom).unwrap();

        assert_eq!(discover_config_dirs(home, Some(work.clone())).len(), 2);
        assert_eq!(
            discover_config_dirs(home, Some(custom.clone())),
            vec![home.join(".claude"), work, custom]
        );
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
