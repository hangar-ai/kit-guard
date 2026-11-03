//! A pretend Hangar around the tab, for the tests: what `fakeHangar`, `fakeClock`
//! and `settle` of test/helpers.js were for the page. It answers the calls the
//! way Hangar does for the two permissions the tab holds, keeps a data folder in
//! memory, and moves the clock a second at a time, firing the timers the tab
//! declared when they are due.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;

use serde_json::{Value, json};

use hangar_kit_sdk::api::KitError;
use hangar_kit_sdk::format;
use hangar_kit_sdk::testing::{self, Harness, StartExt};
use hangar_kit_sdk::ui::abi::{Call, CallId};
use hangar_kit_sdk::ui::{Element, HandlerId};

use kit_guard_tab::Guard;
use kit_guard_tab::logic::{ERRORS, LOG, STATE};

pub const KEY_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const KEY_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// `Date.parse("2026-10-31T18:00:00Z")`, which the page's tests used.
pub fn now() -> i64 {
    format::parse_iso("2026-10-31T18:00:00Z").expect("a date")
}

/// The kit's folder, one up from the crate.
pub fn kit() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("native/ is inside the kit")
        .to_path_buf()
}

/// A refusal as the hook logs it, with what a test changes laid over it.
pub fn refusal(key: &str, at: i64, extra: Value) -> Value {
    let mut event = json!({
        "type": "refusal",
        "at": at,
        "key": key,
        "agent": "Ava",
        "agentId": "pane-1",
        "tool": "Bash",
        "command": "git reset --hard",
        "length": 16,
        "rule": "git-reset-hard",
        "kind": "any",
        "subject": "",
        "why": "git reset --hard throws away uncommitted work, which git cannot bring back",
        "folder": "C:\\work\\project",
    });
    if let (Some(event), Some(extra)) = (event.as_object_mut(), extra.as_object()) {
        for (name, value) in extra {
            event.insert(name.clone(), value.clone());
        }
    }
    event
}

/// Events as the hook writes them: one JSON object a line.
pub fn lines(events: &[Value]) -> String {
    events.iter().map(|event| format!("{event}\n")).collect()
}

/// What Hangar does with the calls of the tab.
#[derive(Default)]
pub struct Hangar {
    /// The kit's data folder.
    pub files: BTreeMap<String, String>,
    /// Every call seen, in order: method and params.
    pub calls: Vec<(String, Value)>,
    pub deny_read: bool,
    pub deny_write: bool,
    pub read_error: Option<&'static str>,
    pub write_error: Option<&'static str>,
    /// `data.write` is left unanswered: a write that takes its time.
    pub hold_writes: bool,
    /// The reads of pass files are left unanswered: Hangar is slow with them.
    pub hold_pass_reads: bool,
    /// What a pass file read answers instead of the folder: a read that was
    /// made before a pass was written, and answers after.
    pub frozen_pass: Option<String>,
}

impl Hangar {
    pub fn calls_to(&self, method: &str) -> Vec<&Value> {
        self.calls
            .iter()
            .filter(|(name, _)| name == method)
            .map(|(_, params)| params)
            .collect()
    }

    fn answer(&mut self, call: &Call) -> Option<Result<Value, KitError>> {
        Some(match call.method.as_str() {
            "tab.setTitle" | "tab.setBadge" => Ok(json!({})),
            "data.read" => {
                let path = call.params["path"].as_str().unwrap_or_default();
                if path.starts_with("passes/") {
                    if self.hold_pass_reads {
                        return None;
                    }
                    if let Some(text) = &self.frozen_pass {
                        return Some(Ok(json!({ "text": text })));
                    }
                }
                if self.deny_read {
                    Err(KitError::new(
                        KitError::DENIED,
                        "this tab was not granted data.read",
                    ))
                } else if let Some(code) = self.read_error {
                    Err(KitError::new(code, code))
                } else {
                    match self.files.get(path) {
                        Some(text) => Ok(json!({ "text": text })),
                        None => Err(KitError::new(KitError::NOT_FOUND, "no such file")),
                    }
                }
            }
            "data.write" => {
                if self.hold_writes {
                    return None;
                }
                if self.deny_write {
                    Err(KitError::new(
                        KitError::DENIED,
                        "this tab was not granted data.write",
                    ))
                } else if let Some(code) = self.write_error {
                    Err(KitError::new(code, format!("could not: {code}")))
                } else {
                    let path = call.params["path"].as_str().unwrap_or_default();
                    let text = call.params["text"].as_str().unwrap_or_default();
                    self.files.insert(path.to_owned(), text.to_owned());
                    Ok(json!({}))
                }
            }
            method => Err(KitError::new(
                KitError::DENIED,
                format!("this tab was not granted what {method} needs"),
            )),
        })
    }
}

/// How a test starts the tab.
pub struct Setup {
    locale: &'static str,
    hangar: Hangar,
}

impl Setup {
    pub fn new() -> Self {
        Setup {
            locale: "en",
            hangar: Hangar::default(),
        }
    }

    pub fn fr(mut self) -> Self {
        self.locale = "fr";
        self
    }

    pub fn file(mut self, path: &str, text: impl Into<String>) -> Self {
        self.hangar.files.insert(path.to_owned(), text.into());
        self
    }

    pub fn log(self, events: &[Value]) -> Self {
        self.file(LOG, lines(events))
    }

    pub fn errors(self, events: &[Value]) -> Self {
        self.file(ERRORS, lines(events))
    }

    pub fn deny_read(mut self) -> Self {
        self.hangar.deny_read = true;
        self
    }

    pub fn deny_write(mut self) -> Self {
        self.hangar.deny_write = true;
        self
    }

    pub fn read_error(mut self, code: &'static str) -> Self {
        self.hangar.read_error = Some(code);
        self
    }

    pub fn files(mut self, files: &BTreeMap<String, String>) -> Self {
        self.hangar.files.extend(files.clone());
        self
    }

    /// Hangar is slow answering the reads of pass files, and answers them
    /// with `text` (what the folder held when they were made) once it does.
    pub fn slow_pass_reads(mut self, text: impl Into<String>) -> Self {
        self.hangar.hold_pass_reads = true;
        self.hangar.frozen_pass = Some(text.into());
        self
    }

    pub fn write_error(mut self, code: &'static str) -> Self {
        self.hangar.write_error = Some(code);
        self
    }

    /// Starts the tab at 18:00 on the 31st, with the two permissions it holds,
    /// and lets it read what it reads first.
    pub fn start(self) -> Tab {
        let clock = now();
        testing::set_now(Some(clock));
        let start = testing::start()
            .grant(["data.read", "data.write"])
            .locale(self.locale);
        let mut tab = Tab {
            h: Harness::new(start),
            hangar: self.hangar,
            clock,
            seen: HashSet::new(),
            due: HashMap::new(),
        };
        tab.pump();
        tab.sync_timers();
        tab
    }
}

pub struct Tab {
    pub h: Harness<Guard>,
    pub hangar: Hangar,
    pub clock: i64,
    seen: HashSet<CallId>,
    due: HashMap<String, i64>,
}

impl Tab {
    /// Answers every call waiting, and those their answers make, until only
    /// the ones Hangar holds back are left.
    pub fn pump(&mut self) {
        loop {
            let waiting: Vec<Call> = self.h.calls().to_vec();
            let mut answered = false;
            for call in waiting {
                if self.seen.insert(call.id) {
                    self.hangar
                        .calls
                        .push((call.method.clone(), call.params.clone()));
                }
                if let Some(answer) = self.hangar.answer(&call) {
                    self.h.reply(call.id, answer);
                    answered = true;
                }
            }
            if !answered {
                break;
            }
        }
    }

    fn timers(&self) -> Vec<(String, i64)> {
        self.h
            .subscriptions()
            .timers
            .iter()
            .map(|timer| (timer.id.clone(), i64::from(timer.every_ms)))
            .collect()
    }

    /// Timers the tab declares start counting now; ones it dropped are forgotten.
    fn sync_timers(&mut self) {
        let declared = self.timers();
        self.due
            .retain(|id, _| declared.iter().any(|(name, _)| name == id));
        for (id, every) in declared {
            self.due.entry(id).or_insert(self.clock + every);
        }
    }

    /// Lets `ms` pass, a second at a time, firing what is due, as the page's
    /// `clock.advance` did.
    pub fn advance(&mut self, ms: i64) {
        let end = self.clock + ms;
        while self.clock < end {
            self.sync_timers();
            self.clock += (end - self.clock).min(1000);
            testing::set_now(Some(self.clock));
            for (id, every) in self.timers() {
                if self.due.get(&id).is_some_and(|due| *due <= self.clock) {
                    self.due.insert(id.clone(), self.clock + every);
                    self.h.timer(&id);
                    self.pump();
                    self.sync_timers();
                }
            }
        }
    }

    /// Sets the clock without firing anything: the next thing that happens reads it.
    pub fn set_clock(&mut self, at: i64) {
        self.clock = at;
        testing::set_now(Some(at));
    }

    /// Fires the poll now, as its timer would, and answers its reads.
    pub fn poll(&mut self) {
        self.h.timer("poll");
        self.pump();
        self.sync_timers();
    }

    pub fn press(&mut self, label: &str) {
        self.h.press(label);
        self.pump();
    }

    /// Presses the `n`th button of this label that can be pressed, from 0.
    pub fn press_nth(&mut self, label: &str, n: usize) {
        let handler =
            self.pressable(label).get(n).copied().unwrap_or_else(|| {
                panic!("no {n}th button {label:?}; there are {:?}", self.buttons())
            });
        self.h.ui(handler, None);
        self.pump();
    }

    /// Every button's label, in the order of the page.
    pub fn buttons(&self) -> Vec<String> {
        let mut found = Vec::new();
        self.h.view().root.for_each(&mut |element| {
            if let Element::Button(button) = element {
                found.extend(button.label.clone());
            }
        });
        found
    }

    /// The handlers of the buttons of this label that can be pressed.
    pub fn pressable(&self, label: &str) -> Vec<HandlerId> {
        let mut found = Vec::new();
        self.h.view().root.for_each(&mut |element| {
            if let Element::Button(button) = element
                && button.label.as_deref() == Some(label)
                && let Some(handler) = button.on_press
            {
                found.push(handler);
            }
        });
        found
    }

    /// Whether a button of this label is drawn, pressable or not.
    pub fn has_button(&self, label: &str) -> bool {
        self.buttons().iter().any(|name| name == label)
    }

    /// Everything the page says, one text a line.
    pub fn text(&self) -> String {
        self.h.texts().join("\n")
    }

    /// How many widgets of the page the test calls its own.
    pub fn count(&self, mut wanted: impl FnMut(&Element) -> bool) -> usize {
        let mut count = 0;
        self.h.view().root.for_each(&mut |element| {
            if wanted(element) {
                count += 1;
            }
        });
        count
    }

    /// The paths of the `data.write` calls so far.
    pub fn written(&self) -> Vec<String> {
        self.hangar
            .calls_to("data.write")
            .iter()
            .map(|params| params["path"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    /// What the tab last told the strip about its badge.
    pub fn badge(&self) -> Option<Value> {
        self.hangar
            .calls_to("tab.setBadge")
            .last()
            .map(|v| (*v).clone())
    }

    /// The last title the tab set.
    pub fn title(&self) -> Option<String> {
        self.hangar
            .calls_to("tab.setTitle")
            .last()
            .and_then(|params| params["title"].as_str().map(str::to_owned))
    }
}

/// Node, when this machine has it: the hook is Node code, and a few tests ask
/// the real thing. CI sets GUARD_REQUIRE_NODE, so that a missing Node is a
/// failure there, not a quiet skip.
pub fn node() -> Option<PathBuf> {
    let found = Command::new("node")
        .arg("--version")
        .output()
        .ok()
        .filter(|out| out.status.success());
    if found.is_none() && std::env::var_os("GUARD_REQUIRE_NODE").is_some() {
        panic!("GUARD_REQUIRE_NODE is set and node is not on the PATH");
    }
    found.map(|_| PathBuf::from("node"))
}

/// Runs a snippet of Node with `args` (`process.argv[1..]`) and answers what it printed.
pub fn node_eval(script: &str, args: &[&str]) -> String {
    let output = Command::new("node")
        .arg("--eval")
        .arg(script)
        .args(args)
        .output()
        .expect("node starts");
    assert!(
        output.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

pub fn state_json(clear: i64) -> String {
    json!({ "clearedAt": clear }).to_string()
}

pub const STATE_FILE: &str = STATE;

/// A folder of its own under the system's temporary one, gone with the test.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let path =
            std::env::temp_dir().join(format!("guard-kit-{name}-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).expect("a temporary folder");
        TempDir(path)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0
    }

    pub fn str(&self) -> &str {
        self.0.to_str().expect("a path")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
