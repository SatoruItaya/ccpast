use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct TrashEntry {
    #[allow(dead_code)]
    pub trashed_path: PathBuf,
    #[allow(dead_code)]
    pub original_path: PathBuf,
    #[allow(dead_code)]
    pub session_id: String,
    #[allow(dead_code)]
    pub deleted_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct IndexLine<'a> {
    trashed_path: &'a Path,
    original_path: &'a Path,
    session_id: &'a str,
    deleted_at: String,
}

/// Move `original_path` into `<trash_root>/<timestamp>-<session_id>.jsonl` and
/// append a JSON line describing the deletion to `<trash_root>/index.jsonl`.
pub fn move_to_trash(
    trash_root: &Path,
    original_path: &Path,
    session_id: &str,
) -> Result<TrashEntry> {
    fs::create_dir_all(trash_root)?;
    let deleted_at = Utc::now();
    let stamp = deleted_at.format("%Y%m%dT%H%M%S%.3fZ");
    let trashed_path = trash_root.join(format!("{stamp}-{session_id}.jsonl"));

    match fs::rename(original_path, &trashed_path) {
        Ok(()) => {}
        Err(err) if err.raw_os_error() == Some(libc::EXDEV) => {
            fs::copy(original_path, &trashed_path)?;
            fs::remove_file(original_path)?;
        }
        Err(err) => return Err(err.into()),
    }

    let index_path = trash_root.join("index.jsonl");
    let line = IndexLine {
        trashed_path: &trashed_path,
        original_path,
        session_id,
        deleted_at: deleted_at.to_rfc3339(),
    };
    let json = serde_json::to_string(&line)?;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&index_path)?;
    writeln!(f, "{json}")?;

    Ok(TrashEntry {
        trashed_path,
        original_path: original_path.to_path_buf(),
        session_id: session_id.to_string(),
        deleted_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn moves_file_and_appends_index_line() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("orig.jsonl");
        fs::write(&src, b"{\"type\":\"user\"}\n").unwrap();
        let trash = tmp.path().join(".trash");

        let entry = move_to_trash(&trash, &src, "session-x").unwrap();

        assert!(!src.exists(), "original should have been moved");
        assert!(entry.trashed_path.exists(), "trashed file should exist");
        assert!(entry.trashed_path.starts_with(&trash));

        let index = fs::read_to_string(trash.join("index.jsonl")).unwrap();
        assert!(index.contains("\"session_id\":\"session-x\""));
        assert!(index.contains("\"original_path\""));
    }

    #[test]
    fn appends_when_index_already_exists() {
        let tmp = tempdir().unwrap();
        let trash = tmp.path().join(".trash");
        fs::create_dir_all(&trash).unwrap();
        fs::write(trash.join("index.jsonl"), b"{\"prior\":true}\n").unwrap();

        let a = tmp.path().join("a.jsonl");
        fs::write(&a, b"x").unwrap();
        move_to_trash(&trash, &a, "id-a").unwrap();

        let index = fs::read_to_string(trash.join("index.jsonl")).unwrap();
        assert_eq!(index.lines().count(), 2);
    }
}
