//! Settings: a sidebar of sections, each a set of grouped rows.

use freya::prelude::*;
use unilux_service::LaunchOpts;

use crate::i18n::t;
use crate::state::{Screen, Section, Ui};

use crate::svc;
use crate::theme::{T, size, space};
use crate::widgets::{Icon, Tone, dot, group, muted, percent, pick_folder, row, title};

const SECTIONS: &[(Section, &str)] = &[
    (Section::Game, "nav-game"),
    (Section::Vr, "prefs-vr"),
    (Section::Performance, "nav-performance"),
    (Section::Health, "nav-health"),
    (Section::Agents, "prefs-agents"),
    (Section::Preferences, "nav-preferences"),
];

#[derive(PartialEq)]
pub struct SettingsScreen(pub Ui);

impl Component for SettingsScreen {
    fn render(&self) -> impl IntoElement {
        view(self.0)
    }
}

#[derive(PartialEq)]
struct GeneralSection(Ui);

impl Component for GeneralSection {
    fn render(&self) -> impl IntoElement {
        general(self.0)
    }
}

fn view(mut ui: Ui) -> impl IntoElement {
    let section = *ui.section.read();
    let content: Element = match section {
        Section::Game => crate::recipes::RecipesSection(ui).into_element(),
        Section::Vr => crate::vr::VrSection(ui).into_element(),
        Section::Performance => rect()
            .spacing(space::XL)
            .child(crate::widgets::page_header(
                t("nav-performance"),
                t("perf-lead"),
            ))
            .child(crate::sessions::SessionsTab(ui))
            .into_element(),
        Section::Health => rect()
            .spacing(space::XL)
            .child(crate::diagnostics::DiagnosticsSection(ui))
            .maybe_child(ui.logs_open.read().then(|| logs(ui)))
            .into_element(),
        Section::Agents => crate::agents::AgentsSection(ui).into_element(),
        Section::Preferences => GeneralSection(ui).into_element(),
    };
    let st = ui.status.read().clone();
    let findings = ui.findings.read().clone();
    let pending = ui.approvals.read().len();
    // Only what needs attention is marked: a count, or a warning dot.
    let mark = move |s: Section| -> Option<Element> {
        let count = |n: usize, tone: Tone| {
            label()
                .text(n.to_string())
                .font_size(size::SMALL)
                .font_weight(FontWeight::MEDIUM)
                .color(tone.color())
                .into_element()
        };
        match s {
            Section::Vr => (!st.vr.ready).then(|| dot(Tone::Warn).into_element()),
            Section::Game => (st.proton.is_none()
                || !st.patches.as_ref().is_some_and(|p| p.intact))
            .then(|| dot(Tone::Warn).into_element()),
            Section::Health => findings.as_ref().and_then(|f| {
                let errors = f
                    .iter()
                    .filter(|f| f.severity == unilux_core::Severity::Error)
                    .count();
                let warns = f
                    .iter()
                    .filter(|f| f.severity == unilux_core::Severity::Warn)
                    .count();
                match (errors, warns) {
                    (0, 0) => None,
                    (0, w) => Some(count(w, Tone::Warn)),
                    (e, _) => Some(count(e, Tone::Error)),
                }
            }),
            Section::Agents => (pending > 0).then(|| count(pending, Tone::Neutral)),
            _ => None,
        }
    };
    let nav_row = |text: String, selected: bool, mark: Option<Element>| {
        rect()
            .width(Size::fill())
            .height(Size::px(34.))
            .horizontal()
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(space::M)
            .child(
                label()
                    .text(text)
                    .width(Size::flex(1.))
                    .color(if selected { T.text } else { T.muted })
                    .font_weight(if selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    }),
            )
            .maybe_child(mark)
    };
    rect()
        .expanded()
        .horizontal()
        .content(Content::Flex)
        .child(
            rect()
                .width(Size::px(220.))
                .height(Size::fill())
                .content(Content::Flex)
                .padding((space::XL, space::M))
                .spacing(2.)
                .background(T.surface)
                .border(
                    Border::new()
                        .fill(crate::widgets::hairline())
                        .width(BorderWidth {
                            right: 1.,
                            ..Default::default()
                        })
                        .alignment(BorderAlignment::Inner),
                )
                .child(
                    rect()
                        .padding((0., space::M, space::XL, space::M))
                        .child(crate::app::brand()),
                )
                .children(
                    SECTIONS
                        .iter()
                        .map(|(s, key)| {
                            let s = *s;
                            let selected = s == section;
                            Button::new()
                                .key(*key)
                                .width(Size::fill())
                                .padding((0., space::M))
                                .flat()
                                .maybe(selected, |b| b.background(T.raised))
                                .on_press(move |_| ui.section.set(s))
                                .child(nav_row(t(key).to_string(), selected, mark(s)))
                                .into_element()
                        })
                        .collect::<Vec<_>>(),
                )
                .child(rect().height(Size::flex(1.)))
                .child(
                    Button::new()
                        .width(Size::fill())
                        .padding((0., space::M))
                        .flat()
                        .on_press(move |_| ui.screen.set(Screen::Launcher))
                        .child(
                            rect()
                                .width(Size::fill())
                                .height(Size::px(34.))
                                .horizontal()
                                .cross_align(Alignment::Center)
                                .spacing(space::S)
                                .child(Icon::Back.view(16., T.muted))
                                .child(label().text(t("nav-back")).color(T.muted)),
                        ),
                )
                .child(rect().padding((space::S, space::M, 0., space::M)).child(
                    muted(format!("Unilux {}", unilux_update::Version::current())).color(T.faint),
                )),
        )
        .child(
            ScrollView::new()
                .width(Size::flex(1.))
                .height(Size::fill())
                .child(
                    rect()
                        .width(Size::fill())
                        .max_width(Size::px(760.))
                        .padding((space::XXXL, space::XXXL))
                        .spacing(space::XL)
                        .child(content),
                ),
        )
}

fn move_prefix_button(ui: Ui, st: &unilux_service::Status) -> Element {
    let moving = st.operation.as_deref() == Some("move_prefix");
    let pct = ui
        .progress
        .read()
        .as_ref()
        .filter(|(task, _, _)| task == "move_prefix")
        .and_then(|(_, d, tot)| percent(*d, *tot));
    let button = Button::new()
        .enabled(st.operation.is_none() && st.session_pid.is_none())
        .on_press(move |_| {
            spawn_in_window(async move {
                if let Some(dir) = pick_folder(t("prefs-prefix-pick")).await {
                    ui.run(async move { svc().move_prefix(dir).await }, move |p| {
                        ui.notice(
                            unilux_core::Severity::Info,
                            crate::i18n::t_named(
                                "prefs-prefix-moved",
                                &[("path", p.display().to_string())],
                            )
                            .unwrap_or_default(),
                        )
                    });
                }
            });
        })
        .child(t(if moving {
            "prefs-prefix-moving"
        } else {
            "prefs-prefix-move"
        }));
    rect()
        .width(Size::px(160.))
        .cross_align(Alignment::End)
        .spacing(space::XS)
        .child(button)
        .maybe_child(
            pct.filter(|_| moving)
                .map(|p| ProgressBar::new(p).show_progress(false).width(Size::fill())),
        )
        .into_element()
}

fn general(mut ui: Ui) -> impl IntoElement {
    let st = ui.status.read().clone();
    let busy = st.operation.is_some();
    let mut auto = use_state(|| svc().config().check_updates);
    let auto_setter = WritableSetter::new(auto, move |v: bool| {
        let mut cfg = svc().config();
        cfg.check_updates = v;
        match svc().set_config(cfg) {
            Ok(()) => auto.set(v),
            Err(e) => ui.error.set(Some(e.to_string())),
        }
    });
    let mut beta = use_state(|| svc().config().update_channel == "beta");
    let beta_setter = WritableSetter::new(beta, move |v: bool| {
        let mut cfg = svc().config();
        cfg.update_channel = if v { "beta" } else { "stable" }.into();
        match svc().set_config(cfg) {
            Ok(()) => beta.set(v),
            Err(e) => ui.error.set(Some(e.to_string())),
        }
    });
    let cur_locale = svc().config().locale;
    let lang_btn = |label: String, val: Option<&'static str>| {
        let active = cur_locale.as_deref() == val;
        let b = Button::new();
        let b = if active { b.filled() } else { b.flat() };
        b.on_press(move |_| {
            if active {
                return;
            }
            let mut cfg = svc().config();
            cfg.locale = val.map(String::from);
            match svc().set_config(cfg) {
                Ok(()) => {
                    crate::i18n::set(crate::i18n::detect());
                    ui.lang.set(crate::i18n::current());
                }
                Err(e) => ui.error.set(Some(e.to_string())),
            }
        })
        .child(label)
        .into_element()
    };
    let lang_row = rect()
        .horizontal()
        .spacing(space::S)
        .child(lang_btn(t("prefs-lang-auto").to_string(), None))
        .children(
            crate::i18n::languages()
                .into_iter()
                .map(|(code, name)| lang_btn(name, Some(code))),
        )
        .into_element();
    let current = unilux_update::Version::current().to_string();
    let update_line = match ui.update.read().as_ref() {
        Some(u) => match &u.available {
            Some(r) => format!("{current} → {}", r.version),
            None => format!("{current} · {}", t("prefs-updates-uptodate")),
        },
        None => current,
    };
    let can_install = ui
        .update
        .read()
        .as_ref()
        .is_some_and(|u| u.available.is_some() && u.can_apply);
    let restart = *ui.restart_ready.read();
    let version_action = if restart {
        Button::new()
            .filled()
            .on_press(|_| crate::restart())
            .child(t("update-restart"))
            .into_element()
    } else if can_install {
        Button::new()
            .filled()
            .enabled(!busy)
            .on_press(move |_| ui.update_now())
            .child(t("update-install"))
            .into_element()
    } else {
        Button::new()
            .flat()
            .on_press(move |_| {
                ui.run(async { svc().update_check().await }, move |i| {
                    ui.update.set(Some(i))
                })
            })
            .child(t("prefs-updates-check"))
            .into_element()
    };
    rect()
        .spacing(space::XL)
        .child(title(t("nav-preferences")))
        .child(group(
            crate::i18n::t("prefs-somnium-space-account"),
            vec![crate::events::AccountForm(ui).into_element()],
        ))
        .child(group(
            t("prefs-updates"),
            vec![
                row(
                    format!("Unilux {}", t("prefs-updates-version")),
                    Some(update_line),
                    Some(version_action),
                ),
                row(
                    t("prefs-updates-auto"),
                    None,
                    Some(Switch::new(auto_setter).into_element()),
                ),
                row(
                    t("prefs-updates-beta"),
                    None,
                    Some(Switch::new(beta_setter).into_element()),
                ),
            ],
        ))
        .child(group(
            t("prefs-general"),
            vec![
                row(
                    t("prefs-lang"),
                    Some(t("prefs-lang-sub").into()),
                    Some(lang_row),
                ),
                row(
                    t("prefs-prefix"),
                    Some(st.prefix.display().to_string()),
                    Some(move_prefix_button(ui, &st)),
                ),
                row(
                    t("play-flat"),
                    None,
                    Some(
                        Button::new()
                            .flat()
                            .enabled(st.installed && !busy)
                            .on_press(move |_| {
                                ui.run(
                                    async { svc().launch(LaunchOpts { allow_flat: true }).await },
                                    |_| {},
                                )
                            })
                            .child(t("play"))
                            .into_element(),
                    ),
                ),
                row(
                    t("prefs-repair"),
                    Some(t("prefs-repair-sub").into()),
                    Some(
                        Button::new()
                            .enabled(st.installed && !busy)
                            .on_press(move |_| ui.run(async { svc().repair().await }, |_| {}))
                            .child(t("play-repair"))
                            .into_element(),
                    ),
                ),
            ],
        ))
}

/// The launch log, unfolded under Health: the last lines, newest last.
fn logs(ui: Ui) -> impl IntoElement {
    let lines = ui.logs.read().clone();
    let skip = lines.len().saturating_sub(120);
    rect()
        .width(Size::fill())
        .spacing(space::S)
        .child(
            label()
                .text(t("prefs-logs"))
                .font_size(size::SMALL)
                .font_weight(FontWeight::MEDIUM)
                .color(T.muted),
        )
        .child(muted(svc().paths().launch_log().display().to_string()).color(T.faint))
        .child(
            rect()
                .width(Size::fill())
                .padding(space::L)
                .spacing(2.)
                .background(T.surface)
                .corner_radius(10.)
                .children(
                    lines
                        .into_iter()
                        .skip(skip)
                        .enumerate()
                        .map(|(i, l)| {
                            label()
                                .key(i)
                                .text(l)
                                .font_size(size::SMALL)
                                .font_family("monospace")
                                .color(T.muted)
                        })
                        .collect::<Vec<_>>(),
                ),
        )
}
