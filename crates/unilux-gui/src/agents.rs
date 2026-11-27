//! Agents: connect a coding agent, see what it may do and what it did.

use std::time::Duration;

use freya::prelude::*;
use unilux_agent::ToolInfo;

use crate::i18n::{t, t_args};
use crate::state::Ui;
use crate::svc;
use crate::theme::{T, size, space};
use crate::widgets::{Tone, banner, body, chip, divided, group, mono_box, muted, row};
use crate::{approvals, sessions};

#[derive(PartialEq)]
pub struct AgentsSection(pub Ui);

impl Component for AgentsSection {
    fn render(&self) -> impl IntoElement {
        view(self.0)
    }
}

/// Tool, category, what it does in English and in French.
const TOOLS: &[(&str, usize)] = &[
    ("status", 0),
    ("system", 0),
    ("list_processes", 0),
    ("tail_logs", 0),
    ("export_report", 0),
    ("run_doctor", 1),
    ("list_fixes", 1),
    ("apply_fix", 1),
    ("vr_status", 2),
    ("proton_installed", 3),
    ("proton_available", 3),
    ("proton_use", 3),
    ("recipe_list", 4),
    ("recipe_get", 4),
    ("ingredient_list", 4),
    ("recipe_check", 4),
    ("recipe_save", 4),
    ("recipe_build", 4),
    ("recipe_use", 4),
    ("session_list", 5),
    ("recipe_verdict", 5),
    ("recipe_share", 5),
    ("community_list", 5),
    ("community_install", 5),
    ("patch_status", 6),
    ("patch_list", 6),
    ("patch_read", 6),
    ("patch_plan", 6),
    ("patch_build", 6),
    ("patch_apply", 6),
    ("launch", 7),
    ("stop", 7),
    ("update_check", 7),
    ("update_apply", 7),
];

fn category(i: usize) -> (&'static str, &'static str) {
    match i {
        0 => (t("agents-state-logs"), t("agents-read-how-unilux-game")),
        1 => (
            t("agents-checks-fixes"),
            t("agents-run-diagnostics-apply-fixes"),
        ),
        2 => (t("agents-vr-headset"), t("agents-check-headset-ready-game")),
        3 => (t("agents-proton"), t("agents-see-proton-versions-switch")),
        4 => (t("agents-recipes"), t("agents-read-write-build-use")),
        5 => (
            t("agents-sessions-community"),
            t("agents-read-game-sessions-community"),
        ),
        6 => (
            t("agents-wine-patches"),
            t("agents-inspect-compile-install-patches"),
        ),
        _ => (t("agents-game-updates"), t("agents-start-stop-game-update")),
    }
}

fn label_of(name: &str) -> String {
    TOOLS
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|(n, _)| t(&format!("agents-tool-{n}")).to_string())
        .unwrap_or_else(|| name.to_string())
}

fn ago(secs: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let d = now.saturating_sub(secs);
    match d {
        0..=59 => t("agents-just-now").to_string(),
        60..=3599 => t_args("agents-min-ago", &[("count", (d / 60).to_string())]),
        3600..=86_399 => t_args("agents-h-ago", &[("count", (d / 3600).to_string())]),
        _ => t_args("agents-days-ago", &[("count", (d / 86_400).to_string())]),
    }
}

struct Call {
    at: u64,
    tool: String,
    ok: bool,
    what: Option<String>,
}

fn calls() -> Vec<Call> {
    let text = std::fs::read_to_string(svc().paths().audit_log()).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| {
            let args = &v["args"];
            let what = ["id", "name", "provider", "path"]
                .iter()
                .find_map(|k| args.get(*k).and_then(|x| x.as_str()))
                .map(|s| {
                    let s = s.rsplit('/').next().unwrap_or(s);
                    if s.chars().count() > 40 {
                        format!("{}...", s.chars().take(40).collect::<String>())
                    } else {
                        s.to_string()
                    }
                });
            Call {
                at: v["at"].as_u64().unwrap_or(0),
                tool: v["tool"].as_str().unwrap_or("?").to_string(),
                ok: v["ok"].as_bool().unwrap_or(false),
                what,
            }
        })
        .collect()
}

fn access_chip(t: &ToolInfo) -> Rect {
    if t.read_only {
        chip(crate::i18n::t("agents-read-only"), Tone::Ok)
    } else if t.needs_approval {
        chip(crate::i18n::t("agents-needs-approval"), Tone::Warn)
    } else {
        chip(crate::i18n::t("agents-acts-own"), Tone::Accent)
    }
}

fn step(n: usize, head: &str, text: &str, code: Option<&str>) -> Element {
    rect()
        .width(Size::fill())
        .horizontal()
        .content(Content::Flex)
        .padding((space::M, space::L))
        .spacing(space::M)
        .child(
            rect()
                .width(Size::px(26.))
                .height(Size::px(26.))
                .corner_radius(13.)
                .background(T.raised)
                .main_align(Alignment::Center)
                .cross_align(Alignment::Center)
                .child(
                    label()
                        .text(n.to_string())
                        .font_size(size::SMALL)
                        .font_weight(FontWeight::MEDIUM)
                        .color(T.muted),
                ),
        )
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(space::XS)
                .child(body(head.to_string()))
                .child(muted(text.to_string()))
                .maybe_child(code.map(|c| mono_box(c.to_string()))),
        )
        .into_element()
}

fn view(ui: Ui) -> impl IntoElement {
    let mut tick = use_state(|| 0u32);
    let mut open = use_state(|| false);
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let n = *tick.peek();
            tick.set(n.wrapping_add(1));
        }
    });
    let _ = *tick.read();

    let pending = ui.approvals.read().clone();
    let log = calls();
    let total = log.len();
    let failed = log.iter().filter(|c| !c.ok).count();
    let last = log.last().map(|c| c.at);
    let tools = unilux_agent::tools();
    let n_read = tools.iter().filter(|t| t.read_only).count();
    let n_ask = tools.iter().filter(|t| t.needs_approval).count();

    let verdict = if !pending.is_empty() {
        banner(
            Tone::Warn,
            if pending.len() == 1 {
                t("agents-agent-waiting-answer").to_string()
            } else {
                t_args(
                    "agents-requests-waiting-answer",
                    &[("count", pending.len().to_string())],
                )
            },
            t("agents-does-nothing-until-decide"),
            None,
        )
    } else {
        banner(
            Tone::Neutral,
            t("agents-ready-agent"),
            match last {
                Some(at) => t_args("agents-last-call-nothing-changes", &[("ago", ago(at))]),
                None => t("agents-no-agent-has-connected").to_string(),
            },
            None,
        )
    };

    let asks: Vec<Element> = divided(
        pending
            .iter()
            .map(|(id, msg)| {
                let id = *id;
                row(
                    msg.clone(),
                    Some(t("agents-requested-agent").to_string()),
                    Some(
                        rect()
                            .horizontal()
                            .spacing(space::S)
                            .child(
                                Button::new()
                                    .filled()
                                    .on_press(move |_| approvals::answer(id, true))
                                    .child(t("agents-approve")),
                            )
                            .child(
                                Button::new()
                                    .on_press(move |_| approvals::answer(id, false))
                                    .child(t("agents-refuse")),
                            )
                            .into_element(),
                    ),
                )
            })
            .collect(),
    );

    let connect = divided(vec![
        step(
            1,
            t("agents-claude-code"),
            t("agents-run-once-terminal"),
            Some("claude mcp add unilux -- unilux mcp"),
        ),
        step(
            2,
            t("agents-another-agent-cursor-zed"),
            t("agents-add-server-mcp-settings"),
            Some(
                "{ \"mcpServers\": { \"unilux\": { \"command\": \"unilux\", \"args\": [\"mcp\"] } } }",
            ),
        ),
        step(
            3,
            t("agents-keep-unilux-open"),
            t("agents-agent-then-shares-window"),
            None,
        ),
    ]);

    let mut cats: Vec<Element> = Vec::new();
    for c in 0..8 {
        let (name, what) = category(c);
        let mine: Vec<&ToolInfo> = tools
            .iter()
            .filter(|t| {
                TOOLS
                    .iter()
                    .find(|(n, ..)| *n == t.name)
                    .map_or(c == 7, |(_, k)| *k == c)
            })
            .collect();
        if mine.is_empty() {
            continue;
        }
        let reads = mine.iter().filter(|t| t.read_only).count();
        let asks_n = mine.iter().filter(|t| t.needs_approval).count();
        let mut chips = rect().horizontal().spacing(space::S);
        if reads > 0 {
            chips = chips.child(chip(
                t_args("agents-count-read-only", &[("count", reads.to_string())]),
                Tone::Ok,
            ));
        }
        if asks_n > 0 {
            chips = chips.child(chip(
                t_args("agents-approval", &[("count", asks_n.to_string())]),
                Tone::Warn,
            ));
        }
        let own = mine.len() - reads - asks_n;
        if own > 0 {
            chips = chips.child(chip(
                t_args("agents-own", &[("count", own.to_string())]),
                Tone::Accent,
            ));
        }
        cats.push(row(
            name,
            Some(what.to_string()),
            Some(chips.into_element()),
        ));
        if *open.read() {
            for t in mine {
                cats.push(
                    rect()
                        .key(t.name)
                        .width(Size::fill())
                        .horizontal()
                        .content(Content::Flex)
                        .cross_align(Alignment::Center)
                        .padding((space::S, space::L, space::S, space::XXL))
                        .spacing(space::M)
                        .child(
                            rect()
                                .width(Size::flex(1.))
                                .child(body(label_of(t.name)))
                                .child(
                                    label()
                                        .text(t.name)
                                        .font_size(size::SMALL)
                                        .font_family("monospace")
                                        .color(T.muted),
                                ),
                        )
                        .child(access_chip(t))
                        .into_element(),
                );
            }
        }
    }
    let toggle = Button::new()
        .flat()
        .on_press(move |_| {
            let v = *open.peek();
            open.set(!v);
        })
        .child(if *open.read() {
            t("agents-hide-details").to_string()
        } else {
            t_args("agents-show-tools", &[("count", tools.len().to_string())])
        })
        .into_element();

    let recent: Vec<Element> = log
        .iter()
        .rev()
        .take(15)
        .map(|c| {
            row(
                label_of(&c.tool),
                Some(
                    [Some(sessions::when(c.at)), c.what.clone()]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Some(
                    if c.ok {
                        chip(t("agents-done"), Tone::Ok)
                    } else {
                        chip(t("agents-failed-refused"), Tone::Warn)
                    }
                    .into_element(),
                ),
            )
        })
        .collect();
    let recent = if recent.is_empty() {
        vec![row(
            t("agents-nothing-yet"),
            Some(t("agents-every-call-agent-makes").to_string()),
            None,
        )]
    } else {
        divided(recent)
    };

    rect()
        .spacing(space::XL)
        .child(crate::widgets::page_header(
            t("agents-agents"),
            t("agents-coding-agent-like-claude"),
        ))
        .child(verdict)
        .maybe_child((!asks.is_empty()).then(|| group(t("agents-waiting"), asks)))
        .child(group(t("agents-connect-agent"), connect))
        .child(
            rect()
                .width(Size::fill())
                .spacing(space::S)
                .child(group(
                    t_args(
                        "agents-what-agent-do-tools",
                        &[
                            ("tools", tools.len().to_string()),
                            ("reads", n_read.to_string()),
                            ("asks", n_ask.to_string()),
                        ],
                    )
                    .as_str(),
                    cats,
                ))
                .child(toggle),
        )
        .child(sessions::tiles(vec![
            sessions::tile(t("agents-calls"), total.to_string(), None),
            sessions::tile(
                t("agents-last-call"),
                last.map_or("-".to_string(), ago),
                None,
            ),
            sessions::tile(
                t("agents-tile-failed-refused"),
                failed.to_string(),
                (failed > 0).then_some(Tone::Warn),
            ),
        ]))
        .child(group(t("agents-recent-activity"), recent))
        .child(group(
            t("agents-technical-details"),
            divided(vec![
                row(
                    t("agents-socket"),
                    Some(svc().paths().socket().display().to_string()),
                    None,
                ),
                row(
                    t("agents-activity-log"),
                    Some(svc().paths().audit_log().display().to_string()),
                    None,
                ),
            ]),
        ))
}
