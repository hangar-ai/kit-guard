//! The one-time pass, from the tab's click to the hook's retry: this agent, this
//! exact command, two minutes, once. The tab's Rust writes them, the hook's real
//! Node code (`hooks/decide.js`, `hooks/store.js`) takes them, on a real folder.
//! It is test/pass.test.js again, with the tab on the other side of the file.
//!
//! These need Node, which the hook is written for. Without it they say so and
//! pass; with GUARD_REQUIRE_NODE set (CI does), they fail instead.

mod common;

use serde_json::{Value, json};

use common::*;
use kit_guard_tab::logic::{self, PASS_TTL_MS, pass_path};

/// `hooks/decide.js` on one Bash call as Claude Code starts it, for the agent
/// `pane-1` named Ava: what it answers (`null` lets the call through).
const DECIDE: &str = r#"
const path = require("node:path");
const [hooks, dir, project, now, command] = process.argv.slice(1);
const { decide } = require(path.join(hooks, "decide.js"));
const event = { session_id: "session-1", cwd: project, hook_event_name: "PreToolUse", tool_name: "Bash", tool_input: { command } };
const env = { HANGAR_PANE_ID: "pane-1", HANGAR_PANE_NAME: "Ava", CLAUDE_PROJECT_DIR: project };
const answer = decide(JSON.stringify(event), { env, now: Number(now), context: { dataDir: dir } });
console.log(JSON.stringify(answer));
"#;

/// `takePass` of the hook's store: whether it accepted the pass.
const TAKE: &str = r#"
const path = require("node:path");
const [hooks, dir, key, agentId, now] = process.argv.slice(1);
const store = require(path.join(hooks, "store.js"));
console.log(JSON.stringify(store.takePass(dir, key, agentId, Number(now))));
"#;

struct Hook {
    hooks: String,
    data: TempDir,
    project: TempDir,
}

impl Hook {
    fn new() -> Option<Hook> {
        node()?;
        Some(Hook {
            hooks: kit().join("hooks").to_str().expect("a path").to_owned(),
            data: TempDir::new("data"),
            project: TempDir::new("project"),
        })
    }

    /// The hook's answer to a Bash call at `now`, `Value::Null` when it lets it go.
    fn decide(&self, command: &str, now: i64) -> Value {
        let at = now.to_string();
        let printed = node_eval(
            DECIDE,
            &[
                &self.hooks,
                self.data.str(),
                self.project.str(),
                &at,
                command,
            ],
        );
        serde_json::from_str(&printed).expect("JSON")
    }

    fn take(&self, key: &str, agent: &str, now: i64) -> bool {
        let at = now.to_string();
        node_eval(TAKE, &[&self.hooks, self.data.str(), key, agent, &at]) == "true"
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.data.path().join(logic::LOG)).unwrap_or_default()
    }

    fn put_pass(&self, key: &str, text: &str) {
        let file = self.data.path().join(pass_path(key));
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("passes/");
        std::fs::write(file, text).expect("the pass");
    }
}

fn denied(answer: &Value) -> bool {
    answer["hookSpecificOutput"]["permissionDecision"] == "deny"
}

/// The tab started on a log, its first row's « Allow once » pressed: the text
/// it wrote, and under which path.
fn click(log: &str, locale_fr: bool) -> (String, String) {
    let mut setup = Setup::new().file(logic::LOG, log);
    if locale_fr {
        setup = setup.fr();
    }
    let mut tab = setup.start();
    tab.press(if locale_fr {
        "Autoriser une fois"
    } else {
        "Allow once"
    });
    let writes = tab.hangar.calls_to("data.write");
    assert_eq!(writes.len(), 1);
    (
        writes[0]["path"].as_str().expect("a path").to_owned(),
        writes[0]["text"].as_str().expect("a text").to_owned(),
    )
}

#[test]
fn the_pass_is_the_file_the_hook_reads_field_for_field() {
    let at = now();
    let (path, text) = click(&lines(&[refusal(KEY_A, at - 1000, json!({}))]), false);
    assert_eq!(path, format!("passes/{KEY_A}.json"));
    assert_eq!(
        text,
        format!(
            "{{\"key\":\"{KEY_A}\",\"agentId\":\"pane-1\",\"agent\":\"Ava\",\"tool\":\"Bash\",\"command\":\"git reset --hard\",\"createdAt\":{at},\"expiresAt\":{}}}",
            at + 120_000
        )
    );
}

#[test]
fn the_pass_is_byte_for_byte_what_the_page_wrote() {
    let Some(hook) = Hook::new() else {
        eprintln!("skipped: node is not on the PATH");
        return;
    };
    let at = now();
    let commands = [
        "git reset --hard",
        "echo \"quoted\" && cat 'a\\b' > /dev/null",
        "printf 'tab\there\nnew line' | cat .env",
        "rm -rf ~/projets/été — 日本語 😀 \u{2028} \u{7f} \u{1}",
        "x",
    ];
    for command in commands {
        let log = lines(&[refusal(
            KEY_A,
            at - 1000,
            json!({ "command": command, "agent": "Ava · Explore" }),
        )]);
        let (_, written) = click(&log, false);
        let page = node_eval(
            r#"
            const path = require("node:path");
            const [kit, log, now] = process.argv.slice(1);
            const logic = require(path.join(kit, "tabs", "guard", "logic.js"));
            const rows = logic.buildRows(logic.parseLines(log));
            process.stdout.write(JSON.stringify(logic.passFor(rows[0], Number(now))));
            "#,
            &[kit().to_str().expect("a path"), &log, &at.to_string()],
        );
        assert_eq!(written, page, "{command:?}");
    }
    drop(hook);
}

#[test]
fn the_hooks_clock_for_a_pass_is_the_tabs() {
    let Some(hook) = Hook::new() else {
        eprintln!("skipped: node is not on the PATH");
        return;
    };
    let ttl = node_eval(
        r#"
        const path = require("node:path");
        console.log(require(path.join(process.argv[1], "store.js")).PASS_TTL_MS)
        "#,
        &[&hook.hooks],
    );
    assert_eq!(ttl, PASS_TTL_MS.to_string());
    assert_eq!(PASS_TTL_MS, 2 * 60 * 1000);
}

#[test]
fn from_the_hooks_refusal_through_the_tabs_click_to_the_hooks_retry() {
    let Some(hook) = Hook::new() else {
        eprintln!("skipped: node is not on the PATH");
        return;
    };
    let at = now();
    let command = "git reset --hard";

    // The hook refuses, and writes down who, what and why, as it always did.
    assert!(denied(&hook.decide(command, at - 10_000)));
    let log = hook.log();
    assert!(log.contains("\"type\":\"refusal\""), "{log}");

    // The tab reads that very log and shows the refusal.
    let mut tab = Setup::new().file(logic::LOG, log.clone()).start();
    assert_eq!(
        tab.h.kit().rows().len(),
        1,
        "a row from the hook's own line"
    );
    let key = tab.h.kit().rows()[0].key.clone();
    assert!(logic::is_key(&key));
    assert!(tab.h.shows("Ava"));
    assert!(tab.h.shows("git reset --hard"));

    // The person clicks once. The file lands where the hook looks for it.
    tab.press("Allow once");
    let writes = tab.hangar.calls_to("data.write");
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0]["path"], format!("passes/{key}.json"));
    hook.put_pass(&key, writes[0]["text"].as_str().expect("a text"));

    // The retry goes through, once; the next one is refused again.
    assert_eq!(
        hook.decide(command, at + 5_000),
        Value::Null,
        "the pass lets it through"
    );
    assert!(hook.log().contains("\"type\":\"pass-used\""));
    assert!(
        denied(&hook.decide(command, at + 6_000)),
        "once, and no more"
    );

    // The tab sees the retry in the log, as the hook wrote it.
    tab.hangar.files.insert(logic::LOG.into(), hook.log());
    tab.set_clock(at + 7_000);
    tab.poll();
    let row_states: Vec<_> = tab
        .h
        .kit()
        .rows()
        .iter()
        .map(|row| row.used_at.is_some())
        .collect();
    assert_eq!(
        row_states,
        [false, true],
        "the new refusal, and the one let through"
    );
    assert!(tab.h.shows("Let through once"));
}

#[test]
fn a_pass_is_for_this_agent_this_command_and_two_minutes() {
    let Some(hook) = Hook::new() else {
        eprintln!("skipped: node is not on the PATH");
        return;
    };
    let at = now();
    assert!(denied(&hook.decide("git reset --hard", at - 10_000)));
    let key = {
        let tab = Setup::new().file(logic::LOG, hook.log()).start();
        tab.h.kit().rows()[0].key.clone()
    };
    let write = |agent_at: i64| -> String {
        let mut tab = Setup::new().file(logic::LOG, hook.log()).start();
        tab.set_clock(agent_at);
        tab.press("Allow once");
        tab.hangar.calls_to("data.write")[0]["text"]
            .as_str()
            .expect("text")
            .to_owned()
    };

    // Taken by the agent it was for, in time.
    hook.put_pass(&key, &write(at));
    assert!(hook.take(&key, "pane-1", at + 1));

    // Not by another agent, who asked for another command anyway.
    hook.put_pass(&key, &write(at));
    assert!(!hook.take(&key, "pane-2", at + 1));
    assert!(
        !hook.take(&key, "pane-1", at + 1),
        "and taken out of the way"
    );

    // Not after two minutes.
    hook.put_pass(&key, &write(at));
    assert!(!hook.take(&key, "pane-1", at + 120_000));

    // Just before.
    hook.put_pass(&key, &write(at));
    assert!(hook.take(&key, "pane-1", at + 119_999));

    // A pass for another key is nothing to this one.
    let other = write(at).replace(&key, KEY_B);
    hook.put_pass(&key, &other);
    assert!(!hook.take(&key, "pane-1", at + 1));
}
