//! The tab's pure logic: the hook's log turned
//! into what the tab lists, and the one-time pass a click writes. No Hangar
//! here, no widget: the tab and its tests both use it.
//!
//! The log is written by the hook (`hooks/store.js`) and the pass is read by
//! it (`takePass`), so the shapes below are a contract with that code: they
//! stay as they are, field for field.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::{Map, Value};

/// A pass lasts two minutes. The hook refuses one set further ahead than this
/// and a little margin, so a pass is never longer than that.
pub const PASS_TTL_MS: i64 = 2 * 60 * 1000;
/// The hook's log of refusals, in the kit's data folder.
pub const LOG: &str = "refusals.jsonl";
/// The guard's own failures, apart so that a broken override repeated at every
/// call pushes no refusal out of the log.
pub const ERRORS: &str = "errors.jsonl";
/// What the tab keeps: when the list was last cleared.
pub const STATE: &str = "state.json";

/// A key is the 64 hexadecimal digits of a SHA-256: the hook's `passKey`.
/// Nothing else is ever put in the name of a pass file.
pub fn is_key(key: &str) -> bool {
    key.len() == 64 && key.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A string's `length` in JavaScript: UTF-16 code units, which is what the
/// hook measured a command with.
pub fn js_length(text: &str) -> usize {
    text.encode_utf16().count()
}

/// One line of a log: a JSON object with a finite `at`. The rest is read as
/// read leniently, by [`Event::text`].
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub at: f64,
    fields: Map<String, Value>,
}

impl Event {
    /// A string field, and nothing for anything else.
    pub fn text(&self, name: &str) -> &str {
        match self.fields.get(name) {
            Some(Value::String(text)) => text,
            _ => "",
        }
    }

    /// A string field, when there is one (an empty one is one).
    pub fn text_opt(&self, name: &str) -> Option<&str> {
        match self.fields.get(name) {
            Some(Value::String(text)) => Some(text),
            _ => None,
        }
    }

    fn number(&self, name: &str) -> Option<f64> {
        self.fields
            .get(name)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite())
    }
}

/// One JSON object a line. A line cut short by a crash is passed over, and so
/// is anything that is not an object with a time.
pub fn parse_lines(text: &str) -> Vec<Event> {
    let mut events = Vec::new();
    for line in text.split('\n') {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let at = fields
            .get("at")
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite());
        if let Some(at) = at {
            events.push(Event { at, fields });
        }
    }
    events
}

/// A refusal, as a row of the list.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub key: String,
    pub agent: String,
    pub agent_id: String,
    pub tool: String,
    /// As the hook logged it: cut at 2 000 characters.
    pub command: String,
    /// What the whole command measured, before the cut.
    pub length: f64,
    pub rule: String,
    pub kind: String,
    pub subject: String,
    pub why: String,
    pub folder: String,
    pub at: f64,
    pub count: u32,
    pub used_at: Option<f64>,
}

/// What to list, newest first: one row per agent and command, however many
/// times it was refused, until a pass is used for it. What follows starts a
/// new row.
pub fn build_rows(events: &[Event], cleared_at: f64) -> Vec<Row> {
    let mut open: HashMap<&str, usize> = HashMap::new();
    let mut rows: Vec<Row> = Vec::new();
    for event in events {
        let key = match event.text_opt("key") {
            Some(key) if is_key(key) => key,
            _ => continue,
        };
        if event.at <= cleared_at {
            continue;
        }
        match event.text("type") {
            "refusal" => {
                if let Some(&at) = open.get(key) {
                    rows[at].count += 1;
                    rows[at].at = rows[at].at.max(event.at);
                    continue;
                }
                let command = event.text("command");
                let agent = event.text("agent");
                rows.push(Row {
                    key: key.to_owned(),
                    agent: if agent.is_empty() { "?" } else { agent }.to_owned(),
                    agent_id: event.text("agentId").to_owned(),
                    tool: event.text("tool").to_owned(),
                    command: command.to_owned(),
                    length: event
                        .number("length")
                        .unwrap_or_else(|| js_length(command) as f64),
                    rule: event.text("rule").to_owned(),
                    kind: event.text("kind").to_owned(),
                    subject: event.text("subject").to_owned(),
                    why: event.text("why").to_owned(),
                    folder: event.text("folder").to_owned(),
                    at: event.at,
                    count: 1,
                    used_at: None,
                });
                open.insert(key, rows.len() - 1);
            }
            "pass-used" => {
                if let Some(at) = open.remove(key) {
                    rows[at].used_at = Some(event.at);
                }
            }
            _ => {}
        }
    }
    // Stable: rows of the same time keep the log's order.
    rows.sort_by(|a, b| b.at.total_cmp(&a.at));
    rows
}

/// Where a pass is written, in the kit's data folder. The hook takes it from
/// the same place.
pub fn pass_path(key: &str) -> String {
    format!("passes/{key}.json")
}

/// The file « Allow once » writes: this agent, this tool, this exact command,
/// two minutes. The hook reads `key`, `agentId` and `expiresAt`; the rest says
/// whom and what it was for.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pass {
    pub key: String,
    pub agent_id: String,
    pub agent: String,
    pub tool: String,
    pub command: String,
    pub created_at: i64,
    pub expires_at: i64,
}

impl Pass {
    /// The text of the file, as `JSON.stringify` wrote it.
    pub fn to_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

pub fn pass_for(row: &Row, now: i64) -> Pass {
    Pass {
        key: row.key.clone(),
        agent_id: row.agent_id.clone(),
        agent: row.agent.clone(),
        tool: row.tool.clone(),
        command: row.command.clone(),
        created_at: now,
        expires_at: now + PASS_TTL_MS,
    }
}

/// A pass the tab knows is written: until when, and since when.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grant {
    pub expires_at: f64,
    pub granted_at: f64,
}

/// A pass read back from its file, while it can still be used.
pub fn grant_from_pass(text: &str, key: &str, now: i64) -> Option<Grant> {
    let Value::Object(pass) = serde_json::from_str::<Value>(text).ok()? else {
        return None;
    };
    if pass.get("key").and_then(Value::as_str) != Some(key) {
        return None;
    }
    let expires_at = pass
        .get("expiresAt")
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())?;
    if expires_at <= now as f64 {
        return None;
    }
    let granted_at = pass
        .get("createdAt")
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
        .unwrap_or(now as f64);
    Some(Grant {
        expires_at,
        granted_at,
    })
}

/// Where a row stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PassState {
    None,
    Active { remaining_ms: f64 },
    Expired,
    Used { at: f64 },
}

pub fn pass_state(row: &Row, grant: Option<&Grant>, now: i64) -> PassState {
    if let Some(at) = row.used_at {
        return PassState::Used { at };
    }
    // Refused since the click: the hook took that pass already, or it was never good.
    let Some(grant) = grant.filter(|grant| grant.granted_at >= row.at) else {
        return PassState::None;
    };
    if grant.expires_at > now as f64 {
        PassState::Active {
            remaining_ms: grant.expires_at - now as f64,
        }
    } else {
        PassState::Expired
    }
}

/// The row a click on « Allow once » may write a pass for. None when no row
/// holds that key, when the row was let through already, when its pass is being
/// written, and for anything that is not a key: the name of a pass file is
/// never made of what a click or a log hands over.
pub fn passable<'a>(rows: &'a [Row], key: &str, pending: bool) -> Option<&'a Row> {
    if pending || !is_key(key) {
        return None;
    }
    rows.iter()
        .find(|row| row.key == key)
        .filter(|row| row.used_at.is_none())
}

/// What was refused and not let through.
pub fn refused_count(rows: &[Row]) -> usize {
    rows.iter().filter(|row| row.used_at.is_none()).count()
}

/// The tab's badge: that count, when there is one.
pub fn badge_for(rows: &[Row]) -> Option<usize> {
    Some(refused_count(rows)).filter(|count| *count > 0)
}

/// `2:00`, `0:07`: the time a pass has left, rounded up to the second.
pub fn format_remaining(ms: f64) -> String {
    let seconds = (ms / 1000.0).ceil().max(0.0) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// The last name of a folder, either slash, a trailing one ignored.
pub fn folder_name(folder: &str) -> &str {
    let trimmed = folder.trim_end_matches(['/', '\\']);
    trimmed.rsplit(['/', '\\']).next().unwrap_or("")
}

/// The guard's own failures: how many since the list was cleared, and the latest.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorSummary {
    pub count: usize,
    pub last: String,
}

pub fn error_summary(events: &[Event], cleared_at: f64) -> Option<ErrorSummary> {
    let kept: Vec<&str> = events
        .iter()
        .filter(|event| event.at > cleared_at)
        .filter_map(|event| event.text_opt("message"))
        .collect();
    Some(ErrorSummary {
        count: kept.len(),
        last: (*kept.last()?).to_owned(),
    })
}

/// The clearing time `state.json` holds, when it holds one.
pub fn cleared_at_from(text: &str) -> Option<f64> {
    let text = if text.is_empty() { "{}" } else { text };
    serde_json::from_str::<Value>(text)
        .ok()?
        .get("clearedAt")?
        .as_f64()
        .filter(|n| n.is_finite())
}
