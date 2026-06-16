#![allow(dead_code)]

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub session_id: String,
    pub path: PathBuf,
    pub cwd: Option<String>,
    pub cwd_exists: bool,
    pub last_activity: DateTime<Utc>,
    pub title: String,
    pub message_count: usize,
}

pub const NO_TITLE: &str = "(no title)";

/// Build lightweight metadata for one JSONL file. Returns `None` if the file
/// cannot be opened. Files that exist but contain no usable records still
/// produce a `SessionMeta` (with `title == NO_TITLE` and mtime fallback).
pub fn extract_meta(path: &Path) -> Option<SessionMeta> {
    use std::fs;
    use std::io::{BufRead, BufReader};

    use crate::parser::{parse_line, ParsedRecord, RecordKind};

    let file = fs::File::open(path).ok()?;
    let session_id = path.file_stem()?.to_string_lossy().to_string();

    let mut cwd: Option<String> = None;
    let mut last_ts: Option<DateTime<Utc>> = None;
    let mut title: Option<String> = None;
    let mut message_count: usize = 0;

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Some(record) = parse_line(&line) else {
            continue;
        };
        if let ParsedRecord::UserOrAssistant {
            kind,
            message,
            cwd: c,
            timestamp,
        } = record
        {
            message_count += 1;
            if cwd.is_none() {
                if let Some(c) = c {
                    cwd = Some(c);
                }
            }
            if let Some(ts) = timestamp {
                last_ts = Some(match last_ts {
                    Some(prev) if prev > ts => prev,
                    _ => ts,
                });
            }
            if title.is_none() && kind == RecordKind::User {
                if let Some(t) = title_from_user(&message.content) {
                    title = Some(collapse_whitespace(&t));
                }
            }
            let _ = kind;
        }
    }

    let last_activity = last_ts.unwrap_or_else(|| mtime_or_now(path));
    let cwd_exists = match &cwd {
        Some(c) => Path::new(c).is_dir(),
        None => false,
    };

    Some(SessionMeta {
        session_id,
        path: path.to_path_buf(),
        cwd,
        cwd_exists,
        last_activity,
        title: title.unwrap_or_else(|| NO_TITLE.to_string()),
        message_count,
    })
}

fn title_from_user(content: &crate::parser::MessageContent) -> Option<String> {
    use crate::parser::{ContentBlock, MessageContent};
    match content {
        MessageContent::Text(s) if !s.trim().is_empty() => Some(s.clone()),
        MessageContent::Blocks(blocks) => blocks.iter().find_map(|b| match b {
            ContentBlock::Text(s) if !s.trim().is_empty() => Some(s.clone()),
            _ => None,
        }),
        _ => None,
    }
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn mtime_or_now(path: &Path) -> DateTime<Utc> {
    use std::fs;
    if let Ok(meta) = fs::metadata(path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                return DateTime::<Utc>::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
                    .unwrap_or_else(Utc::now);
            }
        }
    }
    Utc::now()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn extracts_first_user_text_as_title() {
        let m = extract_meta(&fixture("session_with_text.jsonl")).unwrap();
        assert_eq!(m.session_id, "session_with_text");
        assert_eq!(m.cwd.as_deref(), Some("/tmp/proj"));
        assert_eq!(m.title, "explain Box<T> please");
        assert_eq!(m.message_count, 3);
    }

    #[test]
    fn last_activity_is_max_timestamp() {
        let m = extract_meta(&fixture("session_with_text.jsonl")).unwrap();
        let expected: DateTime<Utc> = "2026-01-01T10:00:30.000Z".parse().unwrap();
        assert_eq!(m.last_activity, expected);
    }

    #[test]
    fn falls_back_when_first_user_is_tool_result_only() {
        let m = extract_meta(&fixture("session_only_tool_results.jsonl")).unwrap();
        assert_eq!(m.title, NO_TITLE);
        assert_eq!(m.message_count, 2);
    }

    #[test]
    fn empty_file_uses_mtime_and_no_title() {
        let m = extract_meta(&fixture("session_empty.jsonl")).unwrap();
        assert_eq!(m.title, NO_TITLE);
        assert_eq!(m.message_count, 0);
        // last_activity is the file's mtime; we only assert it's not the unix epoch
        assert!(m.last_activity.timestamp() > 1_700_000_000);
    }
}
