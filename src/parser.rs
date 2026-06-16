#![allow(dead_code)]

use chrono::{DateTime, Utc};

#[derive(Debug, PartialEq, Eq)]
pub enum ContentBlock {
    Text(String),
    Thinking,
    ToolUse { name: String },
    ToolResult,
    Other,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MessageContent {
    None,
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct MessageRecord {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug)]
pub enum ParsedRecord {
    UserOrAssistant {
        kind: RecordKind,
        message: MessageRecord,
        cwd: Option<String>,
        timestamp: Option<DateTime<Utc>>,
    },
    Summary,
    Other,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RecordKind {
    User,
    Assistant,
}

pub fn parse_line(line: &str) -> Option<ParsedRecord> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let ty = v.get("type")?.as_str()?;

    let kind = match ty {
        "user" => RecordKind::User,
        "assistant" => RecordKind::Assistant,
        "summary" => return Some(ParsedRecord::Summary),
        _ => return Some(ParsedRecord::Other),
    };

    let role = v
        .get("message")
        .and_then(|m| m.get("role"))
        .and_then(|r| r.as_str())
        .unwrap_or(match kind {
            RecordKind::User => "user",
            RecordKind::Assistant => "assistant",
        })
        .to_string();

    let content = match v.get("message").and_then(|m| m.get("content")) {
        None => MessageContent::None,
        Some(serde_json::Value::Null) => MessageContent::None,
        Some(serde_json::Value::String(s)) => MessageContent::Text(s.clone()),
        Some(serde_json::Value::Array(items)) => {
            let blocks = items.iter().map(parse_block).collect();
            MessageContent::Blocks(blocks)
        }
        _ => MessageContent::None,
    };

    let cwd = v.get("cwd").and_then(|c| c.as_str()).map(|s| s.to_string());
    let timestamp = v
        .get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Some(ParsedRecord::UserOrAssistant {
        kind,
        message: MessageRecord { role, content },
        cwd,
        timestamp,
    })
}

fn parse_block(v: &serde_json::Value) -> ContentBlock {
    let Some(ty) = v.get("type").and_then(|t| t.as_str()) else {
        return ContentBlock::Other;
    };
    match ty {
        "text" => v
            .get("text")
            .and_then(|t| t.as_str())
            .map(|s| ContentBlock::Text(s.to_string()))
            .unwrap_or(ContentBlock::Other),
        "thinking" => ContentBlock::Thinking,
        "tool_use" => ContentBlock::ToolUse {
            name: v
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string(),
        },
        "tool_result" => ContentBlock::ToolResult,
        _ => ContentBlock::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> ParsedRecord {
        parse_line(s).expect("expected Some")
    }

    #[test]
    fn parses_user_with_string_content() {
        let line = r#"{"type":"user","message":{"role":"user","content":"hi"},"cwd":"/p","timestamp":"2026-01-01T00:00:00.000Z"}"#;
        let r = parse(line);
        match r {
            ParsedRecord::UserOrAssistant {
                kind,
                message,
                cwd,
                timestamp,
            } => {
                assert_eq!(kind, RecordKind::User);
                assert_eq!(message.role, "user");
                assert_eq!(message.content, MessageContent::Text("hi".into()));
                assert_eq!(cwd.as_deref(), Some("/p"));
                assert!(timestamp.is_some());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parses_user_with_array_content_and_first_text() {
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"hello"},{"type":"tool_use","name":"Bash"}]}}"#;
        let r = parse(line);
        match r {
            ParsedRecord::UserOrAssistant { message, .. } => match message.content {
                MessageContent::Blocks(blocks) => {
                    assert_eq!(blocks.first(), Some(&ContentBlock::Text("hello".into())));
                    assert!(
                        matches!(blocks.get(1), Some(ContentBlock::ToolUse { name }) if name == "Bash")
                    );
                }
                other => panic!("expected blocks, got {other:?}"),
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parses_tool_result_only_user() {
        let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"x"}]}}"#;
        match parse(line) {
            ParsedRecord::UserOrAssistant { message, .. } => match message.content {
                MessageContent::Blocks(blocks) => {
                    assert_eq!(blocks, vec![ContentBlock::ToolResult]);
                }
                other => panic!("expected blocks, got {other:?}"),
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parses_user_without_content_as_none() {
        let line = r#"{"type":"user","message":{"role":"user"}}"#;
        match parse(line) {
            ParsedRecord::UserOrAssistant { message, .. } => {
                assert_eq!(message.content, MessageContent::None);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn summary_returns_summary_variant() {
        let line = r#"{"type":"summary","summary":"s","leafUuid":"x"}"#;
        assert!(matches!(parse(line), ParsedRecord::Summary));
    }

    #[test]
    fn unknown_type_returns_other() {
        let line = r#"{"type":"attachment","cwd":"/p"}"#;
        assert!(matches!(parse(line), ParsedRecord::Other));
    }

    #[test]
    fn malformed_json_returns_none() {
        assert!(parse_line("{not json").is_none());
    }
}
