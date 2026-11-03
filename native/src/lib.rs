//! The Guardrails tab, native: the refusals the hook logged, and the one-time
//! pass a click writes. `tabs/guard/` is the same tab as a page, which an
//! older Hangar still opens; this is `tabs/guard.wasm`.
//!
//! The guard itself, the hook that refuses a dangerous command or the reading
//! of a secret before it runs, is Claude Code hook code (`hooks/`) and is not
//! here. The tab only shows what it refused and lets the person let one call
//! through, once, by writing a pass file the hook takes on the retry:
//!
//! - `data.read` of `refusals.jsonl` and `errors.jsonl` every two seconds, the
//!   rows built from them, the badge of the tab set to how many are still
//!   refused;
//! - `data.write` of `passes/<key>.json` on a click of « Allow once », and of
//!   `state.json` on « Clear the list ».
//!
//! Those are its two permissions, as the page's were, and nothing else is
//! called but `tab.setTitle` and `tab.setBadge`.
//!
//! Where the page awaited, the tab has a message: a poll is two reads in
//! flight, answered one after the other, and a second poll does not start
//! while one is waiting.

pub mod logic;
pub mod strings;
mod view;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use hangar_kit_sdk::api::{
    BadgeState, DataRead, DataText, DataWrite, Done, TabSetBadge, TabSetTitle, Theme,
};
use hangar_kit_sdk::prelude::*;

use logic::{
    ERRORS, ErrorSummary, Event, Grant, LOG, PassState, Row, STATE, badge_for, build_rows,
    cleared_at_from, error_summary, grant_from_pass, parse_lines, pass_for, pass_path, pass_state,
    passable,
};
use strings::{for_locale, pick_lang};

/// How often the hook's log is read.
pub const POLL: Duration = Duration::from_secs(2);
/// How often the countdown of a pass is drawn again, while one runs.
const CLOCK: Duration = Duration::from_secs(1);
/// After a reload, the passes still good are found again from their files:
/// the newest rows' only.
pub const PASSES_READ_AT_START: usize = 16;

/// What the page is, before its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Loading,
    List,
    Denied,
    Error,
}

/// Why a pass was not written, as the row says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// `denied`, `rate_limited` or `failed`: what the strings have a line for.
    pub code: &'static str,
    pub detail: String,
}

/// A poll's two reads, as their answers come.
#[derive(Default)]
struct Reads {
    log: Option<Result<String, KitError>>,
    errors: Option<Result<String, KitError>>,
}

pub struct Guard {
    t: Strings,
    /// The clock at the last thing that happened: what the view reads times by.
    now: i64,
    utc_offset: i32,
    visible: bool,
    phase: Phase,
    events: Vec<Event>,
    error_events: Vec<Event>,
    rows: Vec<Row>,
    errors: Option<ErrorSummary>,
    /// key → the pass this tab wrote, or found written.
    grants: HashMap<String, Grant>,
    /// key → why its pass was not written.
    problems: HashMap<String, Problem>,
    /// Keys whose pass is being written.
    pending: HashSet<String>,
    stale: bool,
    clear_failed: bool,
    cleared_at: f64,
    last_log: Option<String>,
    last_errors: Option<String>,
    /// What the badge was last set to: `None` before the first time.
    last_badge: Option<Option<usize>>,
    /// `state.json` has been read: the first poll may start.
    started: bool,
    /// The reads of the poll in flight, none between two polls.
    reads: Option<Reads>,
    grants_read: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// `state.json`, read first: the rows depend on when the list was cleared.
    Started(Result<DataText, KitError>),
    /// The poll's timer, and « Try again ».
    Poll,
    Log(Result<DataText, KitError>),
    Errors(Result<DataText, KitError>),
    /// A pass file, read at start to find the passes still good.
    Pass(String, Result<DataText, KitError>),
    /// A click on « Allow once » of the row of this key.
    Allow(String),
    Allowed {
        key: String,
        created_at: i64,
        expires_at: i64,
        outcome: Result<Done, KitError>,
    },
    Clear,
    Cleared(i64, Result<Done, KitError>),
    Retry,
    /// The countdown of a running pass.
    Clock,
    Visible(bool),
    Theme(Theme),
}

/// The text of a data file, empty when it is not there yet; any other
/// refusal is the caller's.
fn text_of(answer: Result<DataText, KitError>) -> Result<String, KitError> {
    match answer {
        Ok(read) => Ok(read.text),
        Err(error) if error.is(KitError::NOT_FOUND) => Ok(String::new()),
        Err(error) => Err(error),
    }
}

impl Guard {
    fn set_title(&self, cx: &mut Context<Message>) {
        cx.call_quietly(TabSetTitle {
            title: self.t.t("tab"),
        });
    }

    /// Sets the badge when it changed: the list says it too, so a refusal of
    /// the call is not worth a word.
    fn set_badge(&mut self, badge: Option<usize>, cx: &mut Context<Message>) {
        if self.last_badge == Some(badge) {
            return;
        }
        self.last_badge = Some(badge);
        cx.call_quietly(match badge {
            Some(count) => TabSetBadge {
                state: Some(BadgeState::Failure),
                text: Some(count.to_string()),
            },
            None => TabSetBadge {
                state: None,
                text: None,
            },
        });
    }

    fn rebuild(&mut self, cx: &mut Context<Message>) {
        self.rows = build_rows(&self.events, self.cleared_at);
        self.errors = error_summary(&self.error_events, self.cleared_at);
        self.set_badge(badge_for(&self.rows), cx);
    }

    fn start_poll(&mut self, cx: &mut Context<Message>) {
        if self.reads.is_some() {
            return;
        }
        self.reads = Some(Reads::default());
        cx.call(DataRead { path: LOG.into() }, Message::Log);
        cx.call(
            DataRead {
                path: ERRORS.into(),
            },
            Message::Errors,
        );
    }

    /// One of the poll's reads came back; both did, and the poll is over.
    fn poll_answered(&mut self, cx: &mut Context<Message>) {
        let both = matches!(
            &self.reads,
            Some(Reads {
                log: Some(_),
                errors: Some(_)
            })
        );
        if !both {
            return;
        }
        let Some(Reads {
            log: Some(log),
            errors: Some(errors),
        }) = self.reads.take()
        else {
            return;
        };
        // The log's refusal first, as the page read one then the other.
        match log.and_then(|log| errors.map(|errors| (log, errors))) {
            Ok((log, errors)) => {
                if self.last_log.as_deref() != Some(log.as_str())
                    || self.last_errors.as_deref() != Some(errors.as_str())
                {
                    self.events = parse_lines(&log);
                    self.error_events = parse_lines(&errors);
                    self.last_log = Some(log);
                    self.last_errors = Some(errors);
                    self.rebuild(cx);
                }
                self.phase = Phase::List;
                self.stale = false;
            }
            Err(error) if error.is(KitError::DENIED) => {
                self.phase = Phase::Denied;
                self.last_log = None;
                self.last_errors = None;
                self.set_badge(None, cx);
            }
            // What is listed stays; the next poll may do better.
            Err(_) if self.phase == Phase::List => self.stale = true,
            Err(_) => self.phase = Phase::Error,
        }
        if self.phase == Phase::List && !self.grants_read {
            self.grants_read = true;
            self.read_grants(cx);
        }
    }

    /// After a reload, what the pass files say. Passes written before it still hold.
    fn read_grants(&self, cx: &mut Context<Message>) {
        for row in self
            .rows
            .iter()
            .filter(|row| row.used_at.is_none())
            .take(PASSES_READ_AT_START)
        {
            let key = row.key.clone();
            cx.call(
                DataRead {
                    path: pass_path(&key),
                },
                move |answer| Message::Pass(key, answer),
            );
        }
    }

    /// Writes the one-time pass for this row: this agent, this tool, this
    /// exact command, two minutes. Nothing is written for a key no row holds,
    /// for a row already let through, for one whose pass is being written, nor
    /// for anything that is not a key.
    fn allow(&mut self, key: String, cx: &mut Context<Message>) {
        let pending = self.pending.contains(&key);
        let Some(row) = passable(&self.rows, &key, pending) else {
            return;
        };
        let pass = pass_for(row, cx.now());
        self.pending.insert(key.clone());
        self.problems.remove(&key);
        let (created_at, expires_at) = (pass.created_at, pass.expires_at);
        cx.call(
            DataWrite {
                path: pass_path(&key),
                text: pass.to_text(),
            },
            move |outcome| Message::Allowed {
                key,
                created_at,
                expires_at,
                outcome,
            },
        );
    }

    /// Whether some pass is running: its countdown is drawn every second.
    fn counting_down(&self) -> bool {
        self.rows.iter().any(|row| {
            matches!(
                pass_state(row, self.grants.get(&row.key), self.now),
                PassState::Active { .. }
            )
        })
    }
}

impl Kit for Guard {
    type Message = Message;

    fn init(start: &Start, cx: &mut Context<Message>) -> Self {
        let guard = Guard {
            t: for_locale(&start.theme.locale),
            now: cx.now(),
            utc_offset: start.utc_offset,
            visible: start.visible,
            phase: Phase::Loading,
            events: Vec::new(),
            error_events: Vec::new(),
            rows: Vec::new(),
            errors: None,
            grants: HashMap::new(),
            problems: HashMap::new(),
            pending: HashSet::new(),
            stale: false,
            clear_failed: false,
            cleared_at: 0.0,
            last_log: None,
            last_errors: None,
            last_badge: None,
            started: false,
            reads: None,
            grants_read: false,
        };
        guard.set_title(cx);
        cx.call(DataRead { path: STATE.into() }, Message::Started);
        guard
    }

    fn update(&mut self, message: Message, cx: &mut Context<Message>) {
        self.now = cx.now();
        match message {
            Message::Started(answer) => {
                // Unreadable, or not ours: the first poll says which.
                if let Some(at) = answer.ok().and_then(|read| cleared_at_from(&read.text)) {
                    self.cleared_at = at;
                }
                self.started = true;
                self.start_poll(cx);
            }
            Message::Poll | Message::Retry => {
                if self.started {
                    self.start_poll(cx);
                }
            }
            Message::Log(answer) => {
                if let Some(reads) = self.reads.as_mut() {
                    reads.log = Some(text_of(answer));
                }
                self.poll_answered(cx);
            }
            Message::Errors(answer) => {
                if let Some(reads) = self.reads.as_mut() {
                    reads.errors = Some(text_of(answer));
                }
                self.poll_answered(cx);
            }
            Message::Pass(key, answer) => {
                let found = answer
                    .ok()
                    .and_then(|read| grant_from_pass(&read.text, &key, self.now));
                if let Some(found) = found {
                    // A pass written since is newer than the file read.
                    let keep = self
                        .grants
                        .get(&key)
                        .is_some_and(|grant| grant.expires_at >= found.expires_at);
                    if !keep {
                        self.grants.insert(key, found);
                    }
                }
            }
            Message::Allow(key) => self.allow(key, cx),
            Message::Allowed {
                key,
                created_at,
                expires_at,
                outcome,
            } => {
                match outcome {
                    Ok(_) => {
                        self.grants.insert(
                            key.clone(),
                            Grant {
                                expires_at: expires_at as f64,
                                granted_at: created_at as f64,
                            },
                        );
                    }
                    Err(error) => {
                        let code = if error.is(KitError::DENIED) {
                            "denied"
                        } else if error.is(KitError::RATE_LIMITED) {
                            "rate_limited"
                        } else {
                            "failed"
                        };
                        self.problems.insert(
                            key.clone(),
                            Problem {
                                code,
                                detail: error.message,
                            },
                        );
                    }
                }
                self.pending.remove(&key);
            }
            // Empties the list: what the log holds stays, and is not shown.
            Message::Clear => {
                let at = cx.now();
                cx.call(
                    DataWrite {
                        path: STATE.into(),
                        text: serde_json::json!({ "clearedAt": at }).to_string(),
                    },
                    move |outcome| Message::Cleared(at, outcome),
                );
            }
            Message::Cleared(at, Ok(_)) => {
                self.cleared_at = at as f64;
                self.clear_failed = false;
                self.rebuild(cx);
            }
            Message::Cleared(_, Err(_)) => self.clear_failed = true,
            Message::Clock => {}
            Message::Visible(visible) => self.visible = visible,
            // Hangar's language changed: the strip's title follows.
            Message::Theme(theme) => {
                if pick_lang(&theme.locale) != pick_lang(self.t.locale()) {
                    self.t = for_locale(&theme.locale);
                    self.set_title(cx);
                }
            }
        }
    }

    fn view(&self) -> Element<Message> {
        self.page()
    }

    fn subscriptions(&self) -> Subscriptions<Message> {
        let subscriptions = Subscriptions::none()
            // The poll goes on behind another tab: the badge is the point.
            .every("poll", POLL, Message::Poll)
            .visibility(Message::Visible)
            .theme(Message::Theme);
        // The countdown is only read in front.
        if self.visible && self.counting_down() {
            subscriptions.every("clock", CLOCK, Message::Clock)
        } else {
            subscriptions
        }
    }
}

export_kit!(Guard);

impl Guard {
    /// The rows listed, newest first.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The pass this tab knows of for a row.
    pub fn grant(&self, key: &str) -> Option<&Grant> {
        self.grants.get(key)
    }

    pub fn problem(&self, key: &str) -> Option<&Problem> {
        self.problems.get(key)
    }

    pub fn is_pending(&self, key: &str) -> bool {
        self.pending.contains(key)
    }
}
