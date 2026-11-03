//! The tab against a pretend Hangar (`common::Tab`): what it reads and shows,
//! what a click writes, in both languages, and what it does when Hangar denies
//! it.

mod common;

use std::collections::BTreeSet;

use serde_json::{Value, json};

use hangar_kit_sdk::format;
use hangar_kit_sdk::prelude::*;
use hangar_kit_sdk::testing::{self, Harness, StartExt};
use hangar_kit_sdk::ui::Element as Widget;

use common::*;
use kit_guard_tab::logic::{ERRORS, LOG, STATE, pass_path};
use kit_guard_tab::{Guard, POLL, Phase};

const NBSP: &str = "\u{a0}";
const NNBSP: &str = "\u{202f}";

fn badge(state: &str, text: &str) -> Option<Value> {
    Some(json!({ "state": state, "text": text }))
}

fn no_badge() -> Option<Value> {
    Some(json!({ "state": null }))
}

fn labelled(tab: &Tab, status: Status) -> Vec<String> {
    let mut found = Vec::new();
    tab.h.view().root.for_each(&mut |element| {
        if let Widget::State(state) = element
            && state.status == status
        {
            found.push(state.label.clone());
        }
    });
    found
}

fn primaries(tab: &Tab) -> usize {
    tab.count(|e| matches!(e, Widget::Button(b) if b.kind == ButtonKind::Primary))
}

fn banners(tab: &Tab, kind: Notice) -> usize {
    tab.count(|e| matches!(e, Widget::Banner(b) if b.kind == kind))
}

#[test]
fn the_empty_list_says_what_will_fill_it_no_badge_and_the_limits_are_there() {
    let tab = Setup::new().start();
    assert!(tab.h.shows("Nothing refused yet"));
    assert!(
        tab.h
            .shows("the refusal shows here, and you can allow it once")
    );
    assert!(
        tab.h.texts().iter().any(|text| text == "Nothing refused"),
        "the state, in the header"
    );
    assert!(tab.h.shows("It reads the command line, and nothing else"));
    assert!(
        tab.h
            .shows("It is no sandbox: a script can do anything once started")
    );
    assert!(tab.buttons().is_empty());
    assert_eq!(tab.badge(), no_badge());
    assert_eq!(tab.title().as_deref(), Some("Guardrails"));
    assert_eq!(tab.h.kit().phase(), Phase::List);
}

#[test]
fn it_says_it_is_reading_until_it_has_read() {
    testing::set_now(Some(now()));
    let tab = Harness::<Guard>::new(testing::start().grant(["data.read", "data.write"]));
    assert!(tab.shows("Reading the refusals…"));
    assert!(
        !tab.shows("Nothing refused"),
        "no count it does not know yet"
    );
    assert_eq!(tab.methods(), ["tab.setTitle", "data.read"]);
}

#[test]
fn a_refusal_agent_command_why_when_and_one_button() {
    let at = now();
    let tab = Setup::new()
        .log(&[refusal(
            KEY_A,
            at - 180_000,
            json!({ "agent": "Ava · Explore" }),
        )])
        .start();
    assert!(tab.h.shows("Ava · Explore"));
    assert!(tab.h.texts().iter().any(|text| text == "git reset --hard"));
    assert!(
        tab.h.shows("git reset --hard throws away uncommitted work"),
        "a command keeps its name's case"
    );
    assert!(tab.h.shows(&format!("3{NBSP}min ago")));
    assert!(tab.h.shows("project"));
    assert!(tab.has_button("Allow once"));
    assert_eq!(tab.badge(), badge("failure", "1"));
    assert_eq!(labelled(&tab, Status::Failure), ["1 refused"]);
    assert_eq!(
        primaries(&tab),
        0,
        "several rows, several buttons: none is the one primary action"
    );
    // A block that scrolls: its height is capped.
    let mut heights = Vec::new();
    tab.h.view().root.for_each(&mut |element| {
        if let Widget::Pre(pre) = element {
            heights.push(pre.height);
        }
    });
    assert_eq!(heights, [Some(160.0)]);
}

#[test]
fn refused_again_and_again_one_row_and_it_says_how_many_times() {
    let at = now();
    let tab = Setup::new()
        .log(&[
            refusal(KEY_A, at - 3000, json!({})),
            refusal(KEY_A, at - 2000, json!({})),
            refusal(KEY_A, at - 1000, json!({})),
        ])
        .start();
    assert_eq!(tab.count(|e| matches!(e, Widget::Pre(_))), 1);
    assert!(tab.h.shows("refused 3 times"));
    assert_eq!(tab.badge(), badge("failure", "1"));
}

#[test]
fn in_french_with_accents_and_the_limits_too() {
    let at = now();
    let tab = Setup::new()
        .fr()
        .log(&[
            refusal(KEY_A, at - 180_000, json!({})),
            refusal(KEY_A, at - 170_000, json!({})),
        ])
        .start();
    assert_eq!(tab.title().as_deref(), Some("Garde-fous"));
    assert!(tab.h.shows("Appels refusés"));
    assert!(tab.h.shows("jette le travail non commité"));
    assert!(tab.h.shows(&format!("il y a 2{NBSP}min")));
    assert!(tab.h.shows("refusé 2 fois"));
    assert!(tab.has_button("Autoriser une fois"));
    // Hangar's typography: a narrow no-break space before a colon.
    assert!(tab.h.shows(&format!(
        "Ce n’est pas un bac à sable{NNBSP}: un script peut tout faire une fois lancé"
    )));
}

#[test]
fn hangars_formats_are_used_for_the_times() {
    let at = now();
    let tab = Setup::new()
        .log(&[refusal(KEY_A, at - 1000, json!({}))])
        .start();
    assert!(tab.h.shows("just now"));
    // The full date, on hover.
    let mut tips = Vec::new();
    tab.h.view().root.for_each(&mut |element| {
        if let Widget::List(list) = element {
            tips.extend(list.items.iter().filter_map(|item| item.tooltip.clone()));
        }
    });
    assert_eq!(tips, [format::date(at - 1000, at, 0, "en")]);
    assert_eq!(tips, [format!("Oct 31, 05:59{NNBSP}PM")]);
}

#[test]
fn the_click_writes_the_pass_and_the_row_says_what_to_do_next() {
    let at = now();
    let mut tab = Setup::new()
        .log(&[refusal(KEY_A, at - 1000, json!({}))])
        .start();
    tab.press("Allow once");

    let writes = tab.hangar.calls_to("data.write");
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0]["path"], format!("passes/{KEY_A}.json"));
    let pass: Value =
        serde_json::from_str(writes[0]["text"].as_str().expect("text")).expect("JSON");
    assert_eq!(pass["key"], KEY_A);
    assert_eq!(pass["agentId"], "pane-1");
    assert_eq!(pass["createdAt"], at);
    assert_eq!(pass["expiresAt"], at + 120_000);

    assert!(tab.h.shows("Allowed once, 2:00 left"));
    assert!(tab.h.shows("Tell Ava to retry this exact command."));
    assert!(!tab.has_button("Allow once"));
    assert_eq!(
        labelled(&tab, Status::Waiting),
        ["Allowed once, 2:00 left"],
        "a state: a glyph and a word"
    );

    tab.advance(POLL.as_millis() as i64 * 5);
    assert!(tab.h.shows("1:50 left"), "the time left goes down");

    tab.advance(120_000);
    assert!(tab.h.shows("expired unused"));
    assert!(tab.has_button("Allow once"), "and can be given again");
}

#[test]
fn the_countdown_is_drawn_every_second_while_a_pass_runs_and_the_tab_is_in_front() {
    let at = now();
    let mut tab = Setup::new()
        .log(&[refusal(KEY_A, at - 1000, json!({}))])
        .start();
    let timers = |tab: &Tab| -> Vec<(String, u32)> {
        tab.h
            .subscriptions()
            .timers
            .iter()
            .map(|t| (t.id.clone(), t.every_ms))
            .collect()
    };
    assert_eq!(
        timers(&tab),
        [("poll".to_owned(), 2000)],
        "nothing counts down yet"
    );

    tab.press("Allow once");
    assert_eq!(
        timers(&tab),
        [("poll".to_owned(), 2000), ("clock".to_owned(), 1000)]
    );

    tab.h.event("tab.visibility", json!({ "visible": false }));
    assert_eq!(
        timers(&tab),
        [("poll".to_owned(), 2000)],
        "behind another tab nobody reads it, and the badge still needs the poll"
    );
    tab.h.event("tab.visibility", json!({ "visible": true }));
    assert_eq!(timers(&tab).len(), 2);

    tab.advance(121_000);
    assert_eq!(
        timers(&tab),
        [("poll".to_owned(), 2000)],
        "an expired pass stops it"
    );
}

#[test]
fn a_poll_with_nothing_new_changes_nothing_on_the_page() {
    let at = now();
    let mut tab = Setup::new()
        .log(&[refusal(KEY_A, at - 1000, json!({}))])
        .start();
    let before = tab.h.view().root.clone();
    tab.advance(POLL.as_millis() as i64 * 10);
    assert_eq!(
        tab.h.view().root,
        before,
        "the same page, so a selection and the keyboard stay"
    );

    tab.press("Allow once");
    let code = tab
        .h
        .texts()
        .into_iter()
        .find(|text| text == "git reset --hard");
    tab.advance(POLL.as_millis() as i64 * 5);
    assert_eq!(
        tab.h
            .texts()
            .into_iter()
            .find(|text| text == "git reset --hard"),
        code,
        "the countdown alone changes"
    );
    assert!(tab.h.shows("1:50 left"));

    let more = refusal(KEY_B, tab.clock, json!({ "command": "git clean -fd" }));
    tab.hangar
        .files
        .get_mut(LOG)
        .expect("log")
        .push_str(&lines(&[more]));
    tab.advance(POLL.as_millis() as i64);
    assert!(tab.h.shows("git clean -fd"), "what is new is drawn");
}

#[test]
fn nothing_is_ever_allowed_without_its_own_click_no_such_button_no_such_pass() {
    let at = now();
    let mut tab = Setup::new()
        .log(&[
            refusal(KEY_A, at - 1000, json!({})),
            refusal(KEY_B, at - 500, json!({ "command": "git clean -fd" })),
        ])
        .start();
    tab.advance(60_000);
    let mut names = tab.buttons();
    names.sort();
    assert_eq!(names, ["Allow once", "Allow once", "Clear the list"]);
    assert!(
        tab.hangar.calls_to("data.write").is_empty(),
        "polling and reading write nothing"
    );

    // Newest first: B is the first row, A the second.
    tab.press_nth("Allow once", 1);
    assert_eq!(tab.written(), [format!("passes/{KEY_A}.json")], "that one");
}

#[test]
fn a_click_on_a_list_drawn_before_a_new_refusal_lets_through_the_command_it_was_drawn_beside() {
    let at = now();
    let mut tab = Setup::new()
        .log(&[refusal(KEY_A, at - 1000, json!({}))])
        .start();
    let drawn = tab.h.view().seq;
    let old = tab.pressable("Allow once")[0];

    // A newer refusal comes first in the list now: the old button would be
    // the second, if it were found by its place.
    let newer = refusal(KEY_B, at - 500, json!({ "command": "git clean -fd" }));
    tab.hangar
        .files
        .get_mut(LOG)
        .expect("log")
        .push_str(&lines(&[newer]));
    tab.poll();
    assert!(tab.h.shows("git clean -fd"));

    testing::take_logs();
    tab.h.ui_from(drawn, old, None);
    tab.pump();
    assert!(
        gone(),
        "the view it was drawn in is still kept: the click lands"
    );
    assert_eq!(tab.written(), [format!("passes/{KEY_A}.json")]);
}

/// Whether no click met a view the SDK had let go of: the click was heard.
fn gone() -> bool {
    !testing::take_logs()
        .iter()
        .any(|(_, line)| line.contains("which is gone"))
}

#[test]
fn a_pass_being_written_or_a_row_already_through_writes_nothing_more() {
    let at = now();
    let mut tab = Setup::new()
        .log(&[refusal(KEY_A, at - 1000, json!({}))])
        .start();
    tab.hangar.hold_writes = true;
    let drawn = tab.h.view().seq;
    let allow = tab.pressable("Allow once")[0];

    testing::take_logs();
    tab.h.ui(allow, None);
    tab.pump();
    assert!(tab.h.kit().is_pending(KEY_A));
    // The same button of the page as it was drawn, pressed again.
    tab.h.ui_from(drawn, allow, None);
    tab.pump();
    assert_eq!(tab.hangar.calls_to("data.write").len(), 1);

    tab.hangar.hold_writes = false;
    tab.pump();
    assert!(!tab.h.kit().is_pending(KEY_A));
    assert_eq!(tab.hangar.calls_to("data.write").len(), 1);

    // Let through since: the button of a page drawn before is no use.
    let used = json!({ "type": "pass-used", "at": at, "key": KEY_A });
    tab.hangar
        .files
        .get_mut(LOG)
        .expect("log")
        .push_str(&lines(&[used]));
    tab.poll();
    assert!(!tab.has_button("Allow once"));
    tab.h.ui_from(drawn, allow, None);
    tab.pump();
    assert!(gone(), "the click was heard, and refused");
    assert_eq!(tab.hangar.calls_to("data.write").len(), 1);
}

#[test]
fn while_the_pass_is_written_the_button_says_so_and_cannot_be_pressed() {
    let at = now();
    let mut tab = Setup::new()
        .log(&[refusal(KEY_A, at - 1000, json!({}))])
        .start();
    tab.hangar.hold_writes = true;
    tab.press("Allow once");
    assert!(tab.has_button("Writing the pass…"));
    assert!(
        tab.pressable("Writing the pass…").is_empty(),
        "drawn disabled"
    );
    assert!(!tab.has_button("Allow once"));

    tab.hangar.hold_writes = false;
    tab.pump();
    assert!(tab.h.shows("Allowed once"));
}

#[test]
fn a_retry_seen_in_the_log_the_row_says_so_the_badge_drops() {
    let at = now();
    let mut tab = Setup::new()
        .log(&[refusal(KEY_A, at - 10_000, json!({}))])
        .start();
    tab.press("Allow once");
    let used = json!({ "type": "pass-used", "at": at + 1000, "key": KEY_A });
    tab.hangar
        .files
        .get_mut(LOG)
        .expect("log")
        .push_str(&lines(&[used]));
    tab.advance(POLL.as_millis() as i64);
    assert!(tab.h.shows("Let through once, just now"));
    assert_eq!(tab.badge(), no_badge());
    assert!(!tab.has_button("Allow once"));
    let glyphs = tab.count(|e| matches!(e, Widget::Glyph(g) if g.status == Status::Success && g.label == "Let through once"));
    assert_eq!(glyphs, 1);
}

#[test]
fn a_new_refusal_shows_up_by_itself() {
    let at = now();
    let mut tab = Setup::new().start();
    assert!(tab.h.shows("Nothing refused yet"));
    tab.hangar
        .files
        .insert(LOG.into(), lines(&[refusal(KEY_A, at, json!({}))]));
    tab.advance(POLL.as_millis() as i64);
    assert!(tab.h.shows("git reset --hard"));
    assert_eq!(tab.badge(), badge("failure", "1"));

    let second = refusal(KEY_B, at, json!({ "command": "git clean -fd" }));
    tab.hangar
        .files
        .get_mut(LOG)
        .expect("log")
        .push_str(&lines(&[second]));
    tab.advance(POLL.as_millis() as i64);
    assert_eq!(tab.badge(), badge("failure", "2"));
}

#[test]
fn the_badge_is_only_set_when_it_changes() {
    let at = now();
    let mut tab = Setup::new().log(&[refusal(KEY_A, at, json!({}))]).start();
    tab.advance(POLL.as_millis() as i64 * 10);
    assert_eq!(tab.hangar.calls_to("tab.setBadge").len(), 1);

    // The log grows, the list is built again, and still one is refused.
    let again = refusal(KEY_A, at + 1000, json!({}));
    tab.hangar
        .files
        .get_mut(LOG)
        .expect("log")
        .push_str(&lines(&[again]));
    tab.advance(POLL.as_millis() as i64);
    assert!(tab.h.shows("refused 2 times"));
    assert_eq!(
        tab.hangar.calls_to("tab.setBadge").len(),
        1,
        "no news for the strip"
    );
}

#[test]
fn hangars_language_changing_the_strips_title_follows_once() {
    let mut tab = Setup::new().start();
    let theme = json!({ "scheme": "dark", "locale": "fr" });
    tab.h.event("theme", theme.clone());
    tab.pump();
    tab.h.event("theme", theme);
    tab.pump();
    let titles: Vec<_> = tab
        .hangar
        .calls_to("tab.setTitle")
        .iter()
        .map(|params| params["title"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(titles, ["Guardrails", "Garde-fous"]);
    assert!(tab.h.shows("Rien de refusé"), "and the page speaks it");
}

#[test]
fn a_reload_finds_its_passes_from_their_files() {
    let at = now();
    let tab = Setup::new()
        .log(&[refusal(KEY_A, at - 30_000, json!({}))])
        .file(
            &pass_path(KEY_A),
            json!({ "key": KEY_A, "createdAt": at - 20_000, "expiresAt": at + 100_000 })
                .to_string(),
        )
        .start();
    assert!(tab.h.shows("Allowed once, 1:40 left"));
}

#[test]
fn a_pass_found_in_a_file_never_takes_the_place_of_a_newer_one_just_written() {
    let at = now();
    let old =
        json!({ "key": KEY_A, "createdAt": at - 20_000, "expiresAt": at + 100_000 }).to_string();
    let mut tab = Setup::new()
        .log(&[refusal(KEY_A, at - 30_000, json!({}))])
        .slow_pass_reads(old)
        .start();
    // The reads of the files are still out when the click comes.
    assert!(tab.has_button("Allow once"));
    tab.press("Allow once");
    assert!(tab.h.shows("Allowed once, 2:00 left"));
    tab.hangar.hold_pass_reads = false;
    tab.pump();
    assert!(
        tab.h.shows("Allowed once, 2:00 left"),
        "not the 1:40 of the file"
    );
}

#[test]
fn only_the_newest_rows_look_for_their_pass_files() {
    let at = now();
    let events: Vec<Value> = (0..20)
        .map(|n| refusal(&format!("{n:064x}"), at - 100_000 + n * 1000, json!({})))
        .collect();
    let tab = Setup::new().log(&events).start();
    let read: BTreeSet<String> = tab
        .hangar
        .calls_to("data.read")
        .iter()
        .map(|params| params["path"].as_str().unwrap_or_default().to_owned())
        .filter(|path| path.starts_with("passes/"))
        .collect();
    assert_eq!(read.len(), 16);
    for n in 4..20 {
        assert!(read.contains(&pass_path(&format!("{n:064x}"))), "{n}");
    }
}

#[test]
fn clearing_the_list_empties_stays_so_after_a_reload_newer_ones_show() {
    let at = now();
    let mut tab = Setup::new()
        .log(&[refusal(KEY_A, at - 1000, json!({}))])
        .start();
    tab.press("Clear the list");
    assert!(tab.h.shows("Nothing refused yet"));
    assert_eq!(tab.badge(), no_badge());
    let kept: Value = serde_json::from_str(&tab.hangar.files[STATE]).expect("JSON");
    assert_eq!(kept, json!({ "clearedAt": at }));
    assert!(
        tab.hangar.files.contains_key(LOG),
        "the log itself is the hook's, and stays"
    );

    let again = Setup::new().files(&tab.hangar.files).start();
    assert!(again.h.shows("Nothing refused yet"));

    tab.advance(1000);
    let newer = refusal(KEY_B, tab.clock, json!({ "command": "git clean -fd" }));
    tab.hangar
        .files
        .get_mut(LOG)
        .expect("log")
        .push_str(&lines(&[newer]));
    tab.advance(POLL.as_millis() as i64);
    assert!(tab.h.shows("git clean -fd"));
    let keys: Vec<_> = tab
        .h
        .kit()
        .rows()
        .iter()
        .map(|row| row.key.as_str())
        .collect();
    assert_eq!(keys, [KEY_B], "what was cleared stays out");
}

#[test]
fn denied_reading_says_what_is_missing_and_where_it_is_given_no_badge_a_way_to_retry() {
    let mut tab = Setup::new().deny_read().start();
    assert!(tab.h.shows("This tab may not read the kit’s data"));
    assert!(tab.h.shows("Settings › Kits"));
    assert!(tab.h.shows("It is no sandbox"), "the limits are still said");
    assert_eq!(
        tab.count(|e| matches!(e, Widget::Empty(empty) if empty.kind == EmptyKind::Denied)),
        1,
        "drawn as a permission missing"
    );
    assert_eq!(
        tab.count(|e| matches!(e, Widget::State(_))),
        0,
        "no count it cannot know"
    );
    assert_eq!(tab.badge(), no_badge());
    assert!(tab.has_button("Try again"));
    assert_eq!(primaries(&tab), 0);

    let reads = tab.hangar.calls_to("data.read").len();
    tab.hangar.deny_read = false;
    tab.press("Try again");
    assert!(tab.hangar.calls_to("data.read").len() > reads);
    assert_eq!(tab.h.kit().phase(), Phase::List);
}

#[test]
fn a_grant_taken_back_while_it_runs_the_next_read_says_so_and_it_comes_back_by_itself() {
    let at = now();
    let mut tab = Setup::new().log(&[refusal(KEY_A, at, json!({}))]).start();
    assert_eq!(tab.h.kit().phase(), Phase::List);
    tab.hangar.deny_read = true;
    tab.advance(POLL.as_millis() as i64);
    assert_eq!(tab.h.kit().phase(), Phase::Denied);
    assert_eq!(tab.badge(), no_badge());
    tab.hangar.deny_read = false;
    tab.advance(POLL.as_millis() as i64);
    assert_eq!(tab.h.kit().phase(), Phase::List);
    assert_eq!(tab.h.kit().rows().len(), 1);
    assert_eq!(tab.badge(), badge("failure", "1"));
}

#[test]
fn denied_writing_the_list_still_shows_the_row_says_why_nothing_happened() {
    let at = now();
    let mut tab = Setup::new()
        .log(&[refusal(KEY_A, at, json!({}))])
        .deny_write()
        .start();
    tab.press("Allow once");
    assert!(
        tab.h
            .shows("may not write passes. Accept data.write in Settings › Kits")
    );
    assert_eq!(banners(&tab, Notice::Error), 1);
    assert!(tab.has_button("Allow once"), "still there to try again");
    assert!(tab.h.kit().grant(KEY_A).is_none());
    assert!(tab.h.kit().problem(KEY_A).is_some());

    tab.press("Clear the list");
    assert!(tab.h.shows("could not be cleared"));
    assert!(tab.h.shows("git reset --hard"));

    // Trying again forgets what went wrong the time before.
    tab.hangar.deny_write = false;
    tab.press("Allow once");
    assert!(tab.h.kit().problem(KEY_A).is_none());
    assert!(tab.h.shows("Allowed once"));
    tab.press("Clear the list");
    assert!(!tab.h.shows("could not be cleared"));
    assert!(tab.h.shows("Nothing refused yet"));
}

#[test]
fn over_the_rate_limit_or_a_failed_write_said_on_the_row() {
    let at = now();
    let mut limited = Setup::new()
        .log(&[refusal(KEY_A, at, json!({}))])
        .write_error("rate_limited")
        .start();
    limited.press("Allow once");
    assert!(limited.h.shows("Too many writes in a minute"));

    let mut failed = Setup::new()
        .fr()
        .log(&[refusal(KEY_A, at, json!({}))])
        .write_error("failed")
        .start();
    failed.press("Autoriser une fois");
    assert!(failed.h.shows(&format!(
        "Le laissez-passer n’a pas pu être écrit{NNBSP}: could not: failed"
    )));
}

#[test]
fn a_read_that_fails_says_so_tries_again_and_keeps_what_it_shows() {
    let first = Setup::new().read_error("failed").start();
    assert!(first.h.shows("The refusals could not be read"));
    assert!(first.has_button("Try again"));
    assert_eq!(primaries(&first), 1, "the one thing to do");
    assert_eq!(first.h.kit().phase(), Phase::Error);

    let at = now();
    let mut tab = Setup::new().log(&[refusal(KEY_A, at, json!({}))]).start();
    tab.hangar.read_error = Some("failed");
    tab.advance(POLL.as_millis() as i64);
    assert!(tab.h.shows("The last read failed"));
    assert!(tab.h.shows("git reset --hard"));
    tab.hangar.read_error = None;
    tab.advance(POLL.as_millis() as i64);
    assert!(!tab.h.shows("The last read failed"));
}

#[test]
fn the_guards_own_failures_are_said_not_hidden() {
    let at = now();
    let tab = Setup::new()
        .errors(&[
            json!({ "at": at - 2, "message": "first" }),
            json!({ "at": at - 1, "message": ".hangar/guard.json is not valid JSON" }),
        ])
        .start();
    assert!(tab.h.shows("The guard hit a problem of its own, 2 times"));
    assert!(
        tab.h.shows(
            "either a call went through unchecked, or a broken .hangar/guard.json was ignored"
        )
    );
    assert!(tab.h.shows("guard.json is not valid JSON"));
    assert!(!tab.h.shows("first"));
    assert_eq!(banners(&tab, Notice::Error), 1);
    assert!(tab.hangar.files.contains_key(ERRORS));
}

#[test]
fn what_an_agent_wrote_is_text_never_markup() {
    let at = now();
    let hostile = "<img src=x onerror=alert(1)><script>alert(1)</script>";
    let tab = Setup::new()
        .log(&[refusal(
            KEY_A,
            at,
            json!({
                "command": hostile,
                "agent": hostile,
                "folder": format!("/x/{hostile}"),
                "why": hostile,
                "subject": hostile,
            }),
        )])
        .start();
    let texts = tab.h.texts();
    assert!(
        texts.iter().any(|text| text == hostile),
        "the command, as it was"
    );
    assert!(texts.iter().any(|text| text.contains(hostile)));
    // It is drawn as the text of a block and of a title, which are text.
    let mut as_text = 0;
    tab.h.view().root.for_each(&mut |element| match element {
        Widget::Pre(pre) if pre.text == hostile => as_text += 1,
        Widget::List(list) if list.items.iter().any(|item| item.title == hostile) => as_text += 1,
        _ => {}
    });
    assert_eq!(as_text, 2);
}

#[test]
fn a_command_cut_for_the_log_says_so() {
    let at = now();
    let tab = Setup::new()
        .log(&[refusal(
            KEY_A,
            at,
            json!({ "command": "x".repeat(2000), "length": 5123 }),
        )])
        .start();
    assert!(tab.h.shows("Cut for display: 5123 characters in all"));

    let whole = Setup::new().log(&[refusal(KEY_A, at, json!({}))]).start();
    assert!(!whole.h.shows("Cut for display"));
}

#[test]
fn the_poll_is_one_timer_of_two_seconds_and_a_poll_still_waiting_is_not_doubled() {
    let mut tab = Setup::new().start();
    let timers: Vec<_> = tab
        .h
        .subscriptions()
        .timers
        .iter()
        .map(|timer| (timer.id.as_str(), timer.every_ms))
        .collect();
    assert_eq!(timers, [("poll", 2000)]);
    assert!(tab.h.subscriptions().events.contains(&"theme".to_owned()));

    // Hangar slow to answer: the timer fires again, and no read piles on.
    tab.hangar
        .files
        .insert(LOG.into(), lines(&[refusal(KEY_A, now(), json!({}))]));
    let reads = tab.hangar.calls_to("data.read").len();
    tab.h.timer("poll");
    tab.h.timer("poll");
    tab.h.timer("poll");
    let waiting = tab
        .h
        .calls()
        .iter()
        .filter(|call| call.method == "data.read")
        .count();
    assert_eq!(waiting, 2, "the log and the errors, once");
    tab.pump();
    assert_eq!(
        tab.hangar.calls_to("data.read").len(),
        reads + 2,
        "one poll"
    );
    assert!(tab.h.shows("git reset --hard"));
}

#[test]
fn the_tab_calls_only_what_it_holds() {
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(kit().join("hangar-kit.json")).expect("the manifest"),
    )
    .expect("JSON");
    let held: Vec<&str> = manifest["tabs"][0]["permissions"]
        .as_array()
        .expect("permissions")
        .iter()
        .filter_map(Value::as_str)
        .collect();

    let at = now();
    let mut tab = Setup::new()
        .log(&[
            refusal(KEY_A, at - 1000, json!({})),
            json!({ "type": "pass-used", "at": at - 500, "key": KEY_A }),
            refusal(KEY_B, at - 100, json!({ "command": "git clean -fd" })),
        ])
        .errors(&[json!({ "at": at, "message": "x" })])
        .start();
    tab.press("Allow once");
    tab.advance(10_000);
    tab.press("Clear the list");
    tab.h
        .event("theme", json!({ "scheme": "dark", "locale": "fr" }));
    tab.pump();

    let mut methods = BTreeSet::new();
    for (method, params) in &tab.hangar.calls {
        methods.insert(method.as_str());
        match method.as_str() {
            "data.read" | "data.write" => {
                let path = params["path"].as_str().expect("a path");
                let known = path == LOG
                    || path == ERRORS
                    || path == STATE
                    || path
                        .strip_prefix("passes/")
                        .and_then(|name| name.strip_suffix(".json"))
                        .is_some_and(kit_guard_tab::logic::is_key);
                assert!(known, "{method} {path}");
                if method == "data.write" {
                    assert!(path == STATE || path.starts_with("passes/"), "{path}");
                }
            }
            _ => {}
        }
    }
    assert_eq!(
        methods.iter().copied().collect::<Vec<_>>(),
        ["data.read", "data.write", "tab.setBadge", "tab.setTitle"]
    );
    for method in methods {
        match api::needs(method) {
            Some(api::Needs::Permission(permission)) => {
                assert!(held.contains(&permission), "{method} needs {permission}");
            }
            Some(api::Needs::Nothing) => {}
            other => panic!("{method}: {other:?}"),
        }
    }
}
