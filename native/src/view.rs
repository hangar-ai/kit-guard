//! What the tab draws, in Hangar's own widgets, with no
//! colour of its own. Everything an agent wrote (its name, the command, the
//! folder, the reason) is text: it is never read as anything else.

use hangar_kit_sdk::Handler;
use hangar_kit_sdk::prelude::*;
use hangar_kit_sdk::ui;

use crate::logic::{self, ErrorSummary, PassState, Row};
use crate::strings::why_for;
use crate::{Guard, Message, Phase};

/// How tall a command grows before it scrolls inside its block.
const COMMAND_HEIGHT: f32 = 160.0;

impl Guard {
    pub(crate) fn page(&self) -> Element<Message> {
        let t = &self.t;
        let mut parts: Vec<Element<Message>> = vec![self.header().into()];
        match self.phase {
            Phase::Loading => parts.push(loading(t.t("loading")).into()),
            Phase::Denied => {
                parts.push(
                    denied(t.t("deniedTitle"))
                        .text(t.t("deniedText"))
                        .action(button(t.t("retry")).small().on_press(Message::Retry))
                        .into(),
                );
                parts.push(self.limits());
            }
            Phase::Error => {
                // The one thing to do.
                parts.push(
                    failed(t.t("errorTitle"))
                        .text(t.t("errorText"))
                        .action(
                            button(t.t("retry"))
                                .small()
                                .primary()
                                .on_press(Message::Retry),
                        )
                        .into(),
                );
                parts.push(self.limits());
            }
            Phase::List => {
                if self.stale {
                    parts.push(banner(Notice::Warning, t.t("staleWarning")).into());
                }
                if self.clear_failed {
                    parts.push(banner(Notice::Error, t.t("clearFailed")).into());
                }
                if let Some(errors) = &self.errors {
                    parts.push(self.errors_banner(errors));
                }
                if self.rows.is_empty() {
                    parts.push(empty(t.t("emptyTitle")).text(t.t("emptyText")).into());
                } else {
                    let rows: Element<Message> =
                        list(self.rows.iter().map(|row| self.refusal(row)))
                            .group()
                            .into();
                    parts.push(section(t.t("heading"), [rows]).into());
                }
                parts.push(self.limits());
            }
        }
        page(parts).into()
    }

    fn header(&self) -> ui::Header<Handler<Message>> {
        let t = &self.t;
        let mut head = header::<Message>(t.t("tab"));
        if self.phase == Phase::List {
            let count = logic::refused_count(&self.rows);
            head = head.state(if count > 0 {
                state_label(Status::Failure, t.tn("stateRefused", count as i64, &[]))
            } else {
                state_label(Status::Neutral, t.t("stateClear"))
            });
            if !self.rows.is_empty() {
                head = head.action(
                    button(t.t("clear"))
                        .small()
                        .ghost()
                        .on_press(Message::Clear),
                );
            }
        }
        head
    }

    /// What the guard cannot do, said at the foot of every state.
    fn limits(&self) -> Element<Message> {
        banner(Notice::Info, self.t.t("limits"))
            .title(self.t.t("limitsHeading"))
            .into()
    }

    /// The guard's own failures, and the latest word of them.
    fn errors_banner(&self, errors: &ErrorSummary) -> Element<Message> {
        col![
            banner(Notice::Error, self.t.tn("errors", errors.count as i64, &[])),
            text(errors.last.clone()).code(),
        ]
        .spacing(4.0)
        .into()
    }

    /// The pass of a row: what it was, what is running, or the button.
    fn pass_line(&self, row: &Row) -> Element<Message> {
        let t = &self.t;
        let locale = t.locale();
        match logic::pass_state(row, self.grants.get(&row.key), self.now) {
            PassState::Used { at } => {
                let when = format::since(at as i64, self.now, self.utc_offset, locale);
                state_label(Status::Success, t.tv("passUsed", &[("when", &when)])).into()
            }
            PassState::Active { remaining_ms } => col![
                state_label(
                    Status::Waiting,
                    t.tv(
                        "passActive",
                        &[("remaining", &logic::format_remaining(remaining_ms))]
                    )
                ),
                text(t.tv("passActiveHint", &[("agent", &row.agent)]))
                    .small()
                    .dim(),
            ]
            .spacing(2.0)
            .into(),
            state => {
                let mut parts: Vec<Element<Message>> = Vec::new();
                if state == PassState::Expired {
                    parts.push(text(t.t("passExpired")).small().dim().into());
                }
                if let Some(problem) = self.problems.get(&row.key) {
                    parts.push(
                        banner(
                            Notice::Error,
                            t.tv(
                                &format!("problem.{}", problem.code),
                                &[("detail", &problem.detail)],
                            ),
                        )
                        .into(),
                    );
                }
                // Several rows, several buttons: none is the one primary action.
                // The handler names the row by its key, not its place in the
                // list, so a click on a list drawn before a new refusal came
                // still lets through the command it was drawn beside.
                let allow = if self.pending.contains(&row.key) {
                    button(t.t("allowing")).small()
                } else {
                    button(t.t("allow"))
                        .small()
                        .on_press(Message::Allow(row.key.clone()))
                };
                parts.push(row![allow].into());
                column(parts).spacing(4.0).into()
            }
        }
    }

    fn refusal(&self, row: &Row) -> ui::ListItem<Handler<Message>> {
        let t = &self.t;
        let locale = t.locale();
        let used = row.used_at.is_some();

        let at = row.at as i64;
        let mut facts = format::since(at, self.now, self.utc_offset, locale);
        let folder = logic::folder_name(&row.folder);
        if !folder.is_empty() {
            facts.push_str(" · ");
            facts.push_str(folder);
        }
        if row.count > 1 {
            facts.push_str(" · ");
            facts.push_str(&t.tn("refusedTimes", i64::from(row.count), &[]));
        }

        let mut item = item(row.agent.clone())
            .lead(glyph(
                if used {
                    Status::Success
                } else {
                    Status::Failure
                },
                t.t(if used {
                    "letThroughLabel"
                } else {
                    "refusedLabel"
                }),
            ))
            .sub(facts)
            .tooltip(format::date(at, self.now, self.utc_offset, locale))
            .wrap()
            .body(pre(row.command.clone()).height(COMMAND_HEIGHT));
        if row.length > logic::js_length(&row.command) as f64 {
            item = item.body(
                text(t.tv("truncated", &[("n", &row.length.to_string())]))
                    .small()
                    .dim(),
            );
        }
        item.body(text(why_for(t, &row.rule, &row.kind, &row.subject, &row.why)).small())
            .body(self.pass_line(row))
    }
}
