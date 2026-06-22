use std::path::Path;

use anyhow::Result;

use crate::parser::{parse_line, ContentBlock, MessageContent, ParsedRecord, RecordKind};

#[derive(Debug, Clone)]
pub struct Turn {
    pub role: Role,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// Load up to `limit` turns from the start of the file. `limit = None` means all turns.
pub fn load_turns(path: &Path, limit: Option<usize>) -> Result<Vec<Turn>> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open(path)?;
    let mut turns: Vec<Turn> = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if let Some(t) = parse_turn(&line) {
            turns.push(t);
            if let Some(n) = limit {
                if turns.len() >= n {
                    break;
                }
            }
        }
    }
    Ok(turns)
}

fn parse_turn(line: &str) -> Option<Turn> {
    let record = parse_line(line)?;
    let ParsedRecord::UserOrAssistant { kind, message, .. } = record else {
        return None;
    };
    let role = match kind {
        RecordKind::User => Role::User,
        RecordKind::Assistant => Role::Assistant,
    };
    let body = match message.content {
        MessageContent::None => return None,
        MessageContent::Text(s) => s,
        MessageContent::Blocks(blocks) => format_blocks(&blocks),
    };
    if body.trim().is_empty() {
        return None;
    }
    Some(Turn { role, body })
}

fn format_blocks(blocks: &[ContentBlock]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for b in blocks {
        match b {
            ContentBlock::Text(s) => parts.push(s.clone()),
            ContentBlock::ToolUse { name } => parts.push(format!("[tool: {name}]")),
            ContentBlock::ToolResult => parts.push("[tool result]".to_string()),
            ContentBlock::Thinking | ContentBlock::Other => {}
        }
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fx(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn loads_turns_in_order_and_renders_tool_blocks() {
        let turns = load_turns(&fx("session_with_text.jsonl"), None).unwrap();
        assert_eq!(turns.len(), 3);
        assert_eq!(turns[0].role, Role::User);
        assert!(turns[0].body.contains("explain Box<T>"));
        assert_eq!(turns[1].role, Role::Assistant);
        assert_eq!(turns[1].body, "sure");
    }

    #[test]
    fn limit_caps_the_count() {
        let turns = load_turns(&fx("session_with_text.jsonl"), Some(1)).unwrap();
        assert_eq!(turns.len(), 1);
    }
}
