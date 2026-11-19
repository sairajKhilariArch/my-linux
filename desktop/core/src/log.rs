//! Structured installation events.
//!
//! Every apply, restore, rollback and profile application is recorded as one
//! JSON object per line in `state/events.jsonl`. The log is append-only and is
//! written by the core itself, so a failed operation leaves a record even when
//! the command that triggered it exits with an error.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use config_manager::install_content;

/// Event log path, relative to the installation root.
pub const EVENTS_RELATIVE_PATH: &str = "state/events.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    fn parse(label: &str) -> Self {
        match label {
            "error" => Self::Error,
            "warn" => Self::Warn,
            _ => Self::Info,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub timestamp: u64,
    pub level: Level,
    pub action: String,
    pub detail: String,
}

/// File that holds the event log for an installation root.
pub fn events_file(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join(EVENTS_RELATIVE_PATH)
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());

    for character in text.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }

    escaped
}

fn unescape(text: &str) -> String {
    let mut unescaped = String::with_capacity(text.len());
    let mut characters = text.chars();

    while let Some(character) = characters.next() {
        if character != '\\' {
            unescaped.push(character);
            continue;
        }

        match characters.next() {
            Some('n') => unescaped.push('\n'),
            Some('r') => unescaped.push('\r'),
            Some('t') => unescaped.push('\t'),
            Some('"') => unescaped.push('"'),
            Some('\\') => unescaped.push('\\'),
            Some(other) => unescaped.push(other),
            None => break,
        }
    }

    unescaped
}

/// Appends one event to the log.
pub fn append_event(
    root: impl AsRef<Path>,
    level: Level,
    action: &str,
    detail: &str,
) -> Result<(), config_manager::ConfigError> {
    let file = events_file(root);
    let line = format!(
        "{{\"timestamp\":{},\"level\":\"{}\",\"action\":\"{}\",\"detail\":\"{}\"}}\n",
        timestamp(),
        level.label(),
        escape(action),
        escape(detail)
    );

    let mut content = String::new();

    if let Ok(existing) = fs::read_to_string(&file) {
        content.push_str(&existing);
    }

    content.push_str(&line);

    install_content(content.as_bytes(), file)
}

fn parse_event(line: &str) -> Option<Event> {
    let line = line.trim();
    let body = line.strip_prefix('{')?.strip_suffix('}')?;

    let (timestamp, rest) = body.split_once(",\"level\":\"")?;
    let timestamp = timestamp.strip_prefix("\"timestamp\":")?.parse().ok()?;

    let (level, rest) = rest.split_once("\",\"action\":\"")?;
    let (action, rest) = rest.split_once("\",\"detail\":\"")?;
    let detail = rest.strip_suffix('"')?;

    Some(Event {
        timestamp,
        level: Level::parse(level),
        action: unescape(action),
        detail: unescape(detail),
    })
}

/// Every recorded event, oldest first. Malformed lines are skipped.
pub fn read_events(root: impl AsRef<Path>) -> Vec<Event> {
    fs::read_to_string(events_file(root))
        .map(|contents| contents.lines().filter_map(parse_event).collect())
        .unwrap_or_default()
}

/// The most recent event, if any.
pub fn last_event(root: impl AsRef<Path>) -> Option<Event> {
    read_events(root).pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("my-linux-log-{}-{}", name, std::process::id()));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        directory
    }

    #[test]
    fn appends_and_reads_events_in_order() {
        let directory = scratch("append");

        append_event(&directory, Level::Info, "apply", "theme=cyberpunk files=3").unwrap();
        append_event(&directory, Level::Error, "apply-failed", "disk full").unwrap();

        let events = read_events(&directory);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].level, Level::Info);
        assert_eq!(events[0].action, "apply");
        assert_eq!(events[0].detail, "theme=cyberpunk files=3");
        assert_eq!(events[1].level, Level::Error);
        assert_eq!(last_event(&directory).unwrap().action, "apply-failed");
        assert!(events[0].timestamp > 0);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn escapes_quotes_and_newlines() {
        let directory = scratch("escape");

        append_event(
            &directory,
            Level::Warn,
            "note",
            "path \"with quotes\"\nand a newline\tand a tab",
        )
        .unwrap();

        let events = read_events(&directory);

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].detail,
            "path \"with quotes\"\nand a newline\tand a tab"
        );

        // The file really is one JSON object per line.
        let contents = fs::read_to_string(events_file(&directory)).unwrap();

        assert_eq!(contents.lines().count(), 1);
        assert!(contents.starts_with("{\"timestamp\":"));

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn skips_malformed_lines() {
        let directory = scratch("malformed");

        fs::create_dir_all(directory.join("state")).unwrap();
        fs::write(
            events_file(&directory),
            "not json\n{\"timestamp\":1,\"level\":\"info\",\"action\":\"apply\",\"detail\":\"x\"}\n",
        )
        .unwrap();

        let events = read_events(&directory);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "apply");

        fs::remove_dir_all(directory).unwrap();
    }
}
