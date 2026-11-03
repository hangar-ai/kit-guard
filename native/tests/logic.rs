//! The tab's logic: the log read into rows, the pass, the badge, the words,
//! on `logic.rs` and the strings.

mod common;

use serde_json::{Value, json};

use hangar_kit_sdk::format;

use common::*;
use kit_guard_tab::logic::{
    self, Grant, PassState, Row, badge_for, build_rows, error_summary, folder_name,
    format_remaining, grant_from_pass, is_key, parse_lines, pass_for, pass_path, pass_state,
    passable,
};
use kit_guard_tab::strings::{self, EN, FR, why_for};

fn rows_of(events: &[Value], cleared_at: f64) -> Vec<Row> {
    build_rows(&parse_lines(&lines(events)), cleared_at)
}

#[test]
fn languages() {
    assert_eq!(strings::pick_lang("fr-FR"), "fr");
    assert_eq!(strings::pick_lang("fr"), "fr");
    assert_eq!(strings::pick_lang("FR"), "fr");
    assert_eq!(strings::pick_lang("en-US"), "en");
    assert_eq!(strings::pick_lang("de"), "en");
    assert_eq!(strings::pick_lang(""), "en");
    // `\b`: « fr » is a word of its own, not the start of another.
    assert_eq!(strings::pick_lang("fra"), "en");
    assert_eq!(strings::pick_lang("fr_CA"), "en");
    assert_eq!(strings::pick_lang("fré"), "fr");

    fn keys(table: &'static [(&'static str, &'static str)], reasons: bool) -> Vec<&'static str> {
        let mut keys: Vec<&str> = table
            .iter()
            .map(|(key, _)| *key)
            .filter(|key| reasons || !key.starts_with("why."))
            .collect();
        keys.sort_unstable();
        keys
    }
    assert_eq!(
        keys(FR, false),
        keys(EN, false),
        "French says all English does"
    );
    assert!(keys(EN, true).iter().all(|key| !key.starts_with("why.")));
}

/// The reasons the hook can give, from the hook's own code.
fn hook_reasons() -> Option<serde_json::Map<String, Value>> {
    node()?;
    let rules = kit().join("hooks").join("rules.js");
    let printed = node_eval(
        "console.log(JSON.stringify(require(process.argv[1]).WHY))",
        &[rules.to_str().expect("a path")],
    );
    match serde_json::from_str(&printed).expect("JSON") {
        Value::Object(reasons) => Some(reasons),
        other => panic!("WHY is not an object: {other}"),
    }
}

#[test]
fn every_reason_the_hook_can_give_reads_in_french_and_in_english_as_it_told_the_agent() {
    let Some(reasons) = hook_reasons() else {
        eprintln!("skipped: node is not on the PATH");
        return;
    };
    assert!(reasons.len() >= 14, "it found them");
    let english = strings::for_locale("en");
    let french = strings::for_locale("fr");
    for (code, words) in &reasons {
        let words = words.as_str().expect("words");
        assert!(
            FR.iter().any(|(key, _)| *key == format!("why.{code}")),
            "{code}"
        );
        let (rule, kind) = code.split_once('/').expect("rule/kind");
        let said = words.replace("{subject}", ".env");
        let in_english = why_for(&english, rule, kind, ".env", &said);
        assert!(
            in_english.starts_with("It ") || in_english.starts_with("git "),
            "{code}: {in_english}"
        );
        assert!(in_english.ends_with('.'), "{code}");
        assert!(
            in_english
                .to_lowercase()
                .starts_with(&said[..5].to_lowercase()),
            "{code}"
        );
        if words.contains("{subject}") {
            assert!(
                why_for(&french, rule, kind, ".env", &said).contains(".env"),
                "{code}"
            );
        }
    }
    assert_eq!(
        why_for(&french, "new-rule", "x", "", "it does a new thing"),
        "It does a new thing.",
        "a rule the tab does not know yet"
    );
    let row = "secret-file/read";
    assert!(FR.iter().any(|(key, _)| *key == format!("why.{row}")));
    assert_eq!(
        why_for(&french, "secret-file", "read", ".env", ""),
        "Il lit ou affiche .env, un fichier secret."
    );
}

#[test]
fn the_strings_have_the_shape_hangar_ui_i18n_reads_and_a_count_picks_its_form() {
    let en = strings::for_locale("en");
    let fr = strings::for_locale("fr");
    assert!(
        en.tn("errors", 1, &[])
            .starts_with("The guard hit a problem of its own, once:")
    );
    assert!(en.tn("errors", 3, &[]).contains("3 times"));
    assert_eq!(fr.tn("stateRefused", 2, &[]), "2 refus");
    assert_eq!(en.t("nope"), "nope");
    assert_eq!(strings::for_locale("xx").t("allow"), "Allow once");
    assert_eq!(en.t("passActive"), "Allowed once, {remaining} left");
    for (language, table) in [("en", EN), ("fr", FR)] {
        for (key, text) in table {
            let mut rest = *text;
            while let Some(open) = rest.find('{') {
                let close = rest[open..].find('}').expect("a placeholder closes") + open;
                let name = &rest[open + 1..close];
                assert!(
                    [
                        "n",
                        "time",
                        "remaining",
                        "agent",
                        "when",
                        "detail",
                        "subject"
                    ]
                    .contains(&name),
                    "{language}.{key}: {name}"
                );
                rest = &rest[close + 1..];
            }
        }
    }
}

#[test]
fn parsing_the_log_passes_over_what_is_not_a_whole_line() {
    let whole = refusal(KEY_A, 1, json!({}));
    let text = format!(
        "{whole}\n{{\"type\":\"refusal\",\"at\":2,\"key\":\"cut sho\n\n42\nnull\n{{\"no\":\"time\"}}\n"
    );
    assert_eq!(parse_lines(&text).len(), 1);
    assert!(parse_lines("").is_empty());
    // A time that is not a finite number is no time.
    assert!(parse_lines("{\"at\":\"5\"}\n{\"at\":1e999}\n[1]\n").is_empty());
    assert_eq!(parse_lines("{\"at\":5}\r\n{\"at\":6}").len(), 2);
}

#[test]
fn rows_one_per_agent_and_command_newest_first_until_a_pass_is_used() {
    let rows = rows_of(
        &[
            refusal(KEY_A, 1, json!({})),
            refusal(KEY_B, 2, json!({ "command": "git clean -fd" })),
            refusal(KEY_A, 3, json!({})),
        ],
        0.0,
    );
    let shape: Vec<_> = rows
        .iter()
        .map(|row| (row.key.as_str(), row.count, row.at))
        .collect();
    assert_eq!(shape, [(KEY_A, 2, 3.0), (KEY_B, 1, 2.0)]);

    let after = rows_of(
        &[
            refusal(KEY_A, 1, json!({})),
            json!({ "type": "pass-used", "at": 2, "key": KEY_A }),
            refusal(KEY_A, 3, json!({})),
        ],
        0.0,
    );
    let shape: Vec<_> = after.iter().map(|row| (row.count, row.used_at)).collect();
    assert_eq!(
        shape,
        [(1, None), (1, Some(2.0))],
        "refused again after: a new row"
    );

    assert_eq!(
        rows_of(
            &[refusal(KEY_A, 1, json!({})), refusal(KEY_B, 5, json!({}))],
            3.0
        )
        .len(),
        1,
        "what was cleared stays out"
    );
    assert!(
        rows_of(
            &[
                refusal("../../evil", 1, json!({})),
                refusal(&"A".repeat(64), 1, json!({}))
            ],
            0.0
        )
        .is_empty(),
        "a key is 64 hex digits, or nothing"
    );
    assert!(
        rows_of(
            &[json!({ "type": "pass-used", "at": 1, "key": KEY_A })],
            0.0
        )
        .is_empty()
    );
    let odd = rows_of(
        &[refusal(KEY_A, 1, json!({ "agent": 42, "command": null }))],
        0.0,
    );
    assert_eq!(odd[0].agent, "?");
    assert_eq!(odd[0].command, "");
    assert_eq!(odd[0].length, 16.0, "the length the hook logged");
    let without = rows_of(
        &[refusal(
            KEY_A,
            1,
            json!({ "length": "x", "command": "é😀" }),
        )],
        0.0,
    );
    assert_eq!(without[0].length, 3.0, "UTF-16 units, as the hook counts");
}

#[test]
fn only_a_key_names_a_pass_file() {
    assert!(is_key(KEY_A));
    assert!(is_key(&"0123456789abcdef".repeat(4)));
    assert!(!is_key(&"A".repeat(64)));
    assert!(!is_key(&"a".repeat(63)));
    assert!(!is_key(&format!("{}\n", "a".repeat(64))));
    assert!(!is_key("../../evil"));
    assert!(!is_key(""));
    assert_eq!(pass_path(KEY_A), format!("passes/{KEY_A}.json"));
}

#[test]
fn the_pass_the_rows_key_and_agent_two_minutes() {
    let rows = rows_of(&[refusal(KEY_A, 1, json!({}))], 0.0);
    let pass = pass_for(&rows[0], now());
    assert_eq!(pass.key, KEY_A);
    assert_eq!(pass.agent_id, "pane-1");
    assert_eq!(pass.expires_at - pass.created_at, 120_000);
    assert_eq!(logic::PASS_TTL_MS, 120_000);
}

#[test]
fn where_a_row_stands() {
    let at = now();
    let rows = rows_of(&[refusal(KEY_A, at, json!({}))], 0.0);
    let row = &rows[0];
    let grant = Grant {
        granted_at: (at + 1) as f64,
        expires_at: (at + 120_001) as f64,
    };
    assert_eq!(pass_state(row, None, at), PassState::None);
    assert_eq!(
        pass_state(row, Some(&grant), at + 1001),
        PassState::Active {
            remaining_ms: 119_000.0
        }
    );
    assert_eq!(
        pass_state(row, Some(&grant), at + 120_001),
        PassState::Expired
    );
    let before = Grant {
        granted_at: (at - 5) as f64,
        expires_at: (at + 100) as f64,
    };
    assert_eq!(
        pass_state(row, Some(&before), at),
        PassState::None,
        "refused since the click"
    );
    let used = Row {
        used_at: Some(9.0),
        ..row.clone()
    };
    assert_eq!(
        pass_state(&used, Some(&grant), at),
        PassState::Used { at: 9.0 }
    );
}

#[test]
fn a_pass_read_back_from_its_file() {
    let at = now();
    let good = json!({ "key": KEY_A, "createdAt": at, "expiresAt": at + 1000 }).to_string();
    assert_eq!(
        grant_from_pass(&good, KEY_A, at),
        Some(Grant {
            granted_at: at as f64,
            expires_at: (at + 1000) as f64
        })
    );
    assert_eq!(grant_from_pass(&good, KEY_A, at + 1000), None);
    assert_eq!(grant_from_pass(&good, KEY_B, at), None);
    assert_eq!(grant_from_pass("", KEY_A, at), None);
    assert_eq!(grant_from_pass("{", KEY_A, at), None);
    assert_eq!(grant_from_pass("[]", KEY_A, at), None);
    let no_time = json!({ "key": KEY_A, "expiresAt": at + 1000 }).to_string();
    assert_eq!(
        grant_from_pass(&no_time, KEY_A, at).map(|grant| grant.granted_at),
        Some(at as f64),
        "without its creation, since now"
    );
}

#[test]
fn small_things() {
    assert_eq!(format_remaining(120_000.0), "2:00");
    assert_eq!(format_remaining(61_001.0), "1:02");
    assert_eq!(format_remaining(0.0), "0:00");
    assert_eq!(format_remaining(-5.0), "0:00");
    assert_eq!(folder_name("C:\\work\\project\\"), "project");
    assert_eq!(folder_name("/home/dev/project"), "project");
    assert_eq!(folder_name(""), "");
    // The page's own `relativeTime` was for a page without Hangar UI: the tab
    // writes times as Hangar does.
    let at = now();
    assert_eq!(format::since(at - 10_000, at, 0, "en"), "just now");
    assert_eq!(
        format::since(at - 180_000, at, 0, "fr"),
        "il y a 3\u{a0}min"
    );
    assert_eq!(format::since(at - 180_000, at, 0, "en"), "3\u{a0}min ago");
}

#[test]
fn the_badge_counts_what_is_refused_and_not_let_through() {
    let rows = rows_of(
        &[
            refusal(KEY_A, 1, json!({})),
            refusal(KEY_B, 2, json!({ "command": "git clean -fd" })),
            json!({ "type": "pass-used", "at": 3, "key": KEY_A }),
        ],
        0.0,
    );
    assert_eq!(logic::refused_count(&rows), 1);
    assert_eq!(badge_for(&rows), Some(1));
    assert_eq!(badge_for(&[]), None);
}

#[test]
fn the_guards_own_failures_since_the_list_was_cleared() {
    let events = parse_lines(&lines(&[
        json!({ "at": 1, "message": "first" }),
        json!({ "at": 5, "message": 7 }),
        json!({ "at": 9, "message": ".hangar/guard.json is not valid JSON" }),
    ]));
    let all = error_summary(&events, 0.0).expect("two");
    assert_eq!(
        (all.count, all.last.as_str()),
        (2, ".hangar/guard.json is not valid JSON")
    );
    let later = error_summary(&events, 1.0).expect("one");
    assert_eq!(later.count, 1);
    assert_eq!(error_summary(&events, 9.0), None);
    assert_eq!(error_summary(&[], 0.0), None);
}

#[test]
fn when_the_list_was_cleared_is_read_from_state_json() {
    assert_eq!(logic::cleared_at_from("{\"clearedAt\":5}"), Some(5.0));
    assert_eq!(logic::cleared_at_from(""), None);
    assert_eq!(logic::cleared_at_from("{"), None);
    assert_eq!(logic::cleared_at_from("null"), None);
    assert_eq!(logic::cleared_at_from("{\"clearedAt\":\"5\"}"), None);
}

#[test]
fn a_click_may_write_a_pass_only_for_a_row_still_refused_and_not_being_written() {
    let rows = rows_of(
        &[
            refusal(KEY_A, 1, json!({})),
            refusal(KEY_B, 2, json!({ "command": "git clean -fd" })),
            json!({ "type": "pass-used", "at": 3, "key": KEY_B }),
        ],
        0.0,
    );
    let row = passable(&rows, KEY_A, false).expect("refused, and nothing pending");
    assert_eq!(row.key, KEY_A);
    assert!(
        passable(&rows, KEY_A, true).is_none(),
        "its pass is being written"
    );
    assert!(
        passable(&rows, KEY_B, false).is_none(),
        "let through already"
    );
    assert!(
        passable(&rows, &"c".repeat(64), false).is_none(),
        "a key no row holds"
    );
    assert!(passable(&rows, "../../evil", false).is_none());
    assert!(passable(&rows, "", false).is_none());

    // A row is a row of a log, which anything may have written to: a key that
    // is not 64 hexadecimal digits never names a file, whatever row holds it.
    let refused = rows
        .iter()
        .find(|row| row.key == KEY_A)
        .expect("a refused row");
    let odd = Row {
        key: "../../evil".into(),
        ..refused.clone()
    };
    assert!(passable(std::slice::from_ref(&odd), "../../evil", false).is_none());
    let plain = Row {
        key: KEY_A.into(),
        ..odd
    };
    assert!(
        passable(&[plain], KEY_A, false).is_some(),
        "the same row, with a key"
    );
}
