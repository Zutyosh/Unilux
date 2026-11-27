//! The main screen, after the An Anime Team launchers on a plain dark
//! background: the brand top left, a boot log of what the game starts with
//! bottom left, and one button that does the next right thing beside Settings.

use freya::prelude::*;
use unilux_service::{LaunchOpts, Status};

use crate::i18n::t;
use crate::state::{Screen, Ui};
use crate::theme::{RADIUS_PILL, T, alpha, size, space};
use crate::widgets::{Icon, muted, percent, settings_icon};
use crate::{app, svc};

const MAIN_W: f32 = 240.;

#[derive(PartialEq)]
pub struct LauncherScreen(pub Ui);

impl Component for LauncherScreen {
    fn render(&self) -> impl IntoElement {
        view(self.0)
    }
}

fn view(ui: Ui) -> impl IntoElement {
    let st = ui.status.read().clone();
    rect()
        .expanded()
        .content(Content::Flex)
        .padding(space::XXL)
        .child(
            rect()
                .width(Size::fill())
                .horizontal()
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(space::L)
                .child(app::brand())
                .child(rect().width(Size::flex(1.)))
                .maybe_child((!ui.approvals.read().is_empty()).then(|| {
                    label()
                        .text(t("agents-pending"))
                        .font_size(size::SMALL)
                        .font_weight(FontWeight::MEDIUM)
                        .color(T.warn)
                        .into_element()
                }))
                .maybe_child(update_badge(ui))
                .maybe_child(crate::events::online_badge(ui)),
        )
        .child(
            rect()
                .width(Size::fill())
                .height(Size::flex(1.))
                .horizontal()
                .main_align(Alignment::End)
                .padding((space::XL, 0., space::L, 0.))
                .overflow(Overflow::Clip)
                .child(crate::events::events_panel(ui)),
        )
        .child(
            rect()
                .width(Size::fill())
                .horizontal()
                .content(Content::Flex)
                .cross_align(Alignment::End)
                .spacing(space::XL)
                .child(rect().width(Size::flex(1.)).child(boot_log(ui, &st)))
                .child(actions(ui, &st)),
        )
}

#[derive(Clone, Copy)]
enum Level {
    Ok,
    Warn,
    Fail,
    Info,
    Busy,
}

/// One boot-log line, systemd style: a fixed-width status tag, then text.
fn log_line(level: Level, text: String) -> Element {
    let (tag, color) = match level {
        Level::Ok => ("\u{a0}\u{a0}OK\u{a0}\u{a0}", T.ok),
        Level::Warn => ("\u{a0}WARN\u{a0}", T.warn),
        Level::Fail => ("FAILED", T.error),
        Level::Info => ("\u{a0}INFO\u{a0}", T.muted),
        Level::Busy => ("\u{a0}\u{a0}..\u{a0}\u{a0}", T.text),
    };
    let mono = |s: String, c: (u8, u8, u8)| {
        label()
            .text(s)
            .font_size(size::SMALL)
            .font_family("monospace")
            .color(c)
            .max_lines(1)
            .text_overflow(TextOverflow::Ellipsis)
    };
    rect()
        .horizontal()
        .spacing(space::S)
        .child(
            rect()
                .horizontal()
                .child(mono("[".into(), T.muted))
                .child(mono(tag.into(), color))
                .child(mono("]".into(), T.muted)),
        )
        .child(mono(text, T.text))
        .into_element()
}

/// A raw line from the game's log, under the status lines.
fn raw_line(text: String) -> Element {
    label()
        .text(text)
        .font_size(size::SMALL)
        .font_family("monospace")
        .color(T.faint)
        .max_lines(1)
        .text_overflow(TextOverflow::Ellipsis)
        .into_element()
}

/// Bottom left, straight on the background like CachyOS's game mode boot
/// log: what the game starts with (Proton, the VR runtime, the OpenVR layer,
/// the patches), then the current step, the game's last lines while it
/// starts and runs, and Unilux's notices. Refreshed every second.
fn boot_log(ui: Ui, st: &Status) -> impl IntoElement {
    let mut lines: Vec<Element> = Vec::new();
    lines.push(match &st.proton {
        Some(p) => log_line(
            Level::Ok,
            format!(
                "Proton {}",
                p.split_once(':')
                    .map(|(_, t)| t.to_string())
                    .unwrap_or(p.clone())
            ),
        ),
        None => log_line(Level::Fail, format!("Proton: {}", t("home-log-missing"))),
    });
    lines.push(match &st.vr.active {
        Some(r) => {
            let name = r.name.clone().unwrap_or_else(|| r.kind.to_string());
            if st.vr.ready {
                log_line(Level::Ok, format!("OpenXR {name}: {}", t("home-log-ready")))
            } else if !st.vr.service_running {
                log_line(
                    Level::Warn,
                    format!("OpenXR {name}: {}", t("home-log-service-off")),
                )
            } else {
                log_line(
                    Level::Warn,
                    format!("OpenXR {name}: {}", t("stat-not_ready")),
                )
            }
        }
        None => log_line(Level::Warn, format!("OpenXR: {}", t("home-log-no-runtime"))),
    });
    if st.vr.active.is_some() {
        lines.push(match &st.vr.openvr {
            Some(o) if o.exists => log_line(
                Level::Ok,
                format!("OpenVR {}", format!("{:?}", o.layer).to_lowercase()),
            ),
            _ => log_line(Level::Warn, format!("OpenVR: {}", t("home-log-missing"))),
        });
    }
    let patches = t("stat-patches");
    lines.push(match &st.patches {
        Some(p) if p.bundle.is_some() && p.intact => log_line(
            Level::Ok,
            format!("{patches} {}: {}", st.recipe, t("stat-intact")),
        ),
        Some(p) if p.bundle.is_some() => {
            log_line(Level::Fail, format!("{patches}: {}", t("stat-changed")))
        }
        _ => log_line(Level::Warn, format!("{patches}: {}", t("stat-none"))),
    });
    if let Some(p) = progress(ui) {
        lines.push(p);
    }
    if st.session_pid.is_some() || st.operation.is_some() {
        let log = ui.logs.read().clone();
        let skip = log.len().saturating_sub(6);
        lines.extend(log.into_iter().skip(skip).map(raw_line));
    }
    let notices = ui.notices.read().clone();
    let skip = notices.len().saturating_sub(2);
    for (sev, msg) in notices.into_iter().skip(skip) {
        let level = match sev {
            unilux_core::Severity::Error => Level::Fail,
            unilux_core::Severity::Warn => Level::Warn,
            unilux_core::Severity::Info => Level::Info,
        };
        lines.push(log_line(level, msg));
    }
    rect()
        .width(Size::fill())
        .max_width(Size::px(620.))
        .spacing(4.)
        .children(
            lines
                .into_iter()
                .enumerate()
                .map(|(i, l)| rect().key(i).child(l).into_element())
                .collect::<Vec<_>>(),
        )
}

/// The current step as a log line, with its percentage when known.
fn progress(ui: Ui) -> Option<Element> {
    let step = ui.step.read().clone();
    let progress = ui.progress.read().clone();
    if step.is_none() && progress.is_none() {
        return None;
    }
    let pct = progress.as_ref().and_then(|(_, d, tot)| percent(*d, *tot));
    let name = step
        .or(progress.map(|p| p.0))
        .map(|s| {
            crate::i18n::lookup(&format!("step-{s}"))
                .map(str::to_string)
                .unwrap_or_else(|| s.replace('_', " "))
        })
        .unwrap_or_default();
    Some(log_line(
        Level::Busy,
        match pct {
            Some(p) => format!("{name} {:.0}%", p.floor()),
            None => name,
        },
    ))
}

/// The main button: a wide pill carrying the next right action.
fn main_button(icon: Option<Icon>, text: &str, filled: bool) -> Button {
    let fg = if filled { T.bg } else { T.text };
    let b = Button::new()
        .width(Size::px(MAIN_W))
        .corner_radius(RADIUS_PILL)
        .padding((14., 24.));
    let b = if filled {
        b.filled()
    } else {
        b.background(alpha(T.text, 0.1))
            .hover_background(alpha(T.text, 0.16))
            .border_fill(Color::TRANSPARENT)
    };
    b.child(
        rect()
            .width(Size::fill())
            .horizontal()
            .main_align(Alignment::Center)
            .cross_align(Alignment::Center)
            .spacing(space::M)
            .maybe_child(icon.map(|i| i.view(18., fg)))
            .child(
                label()
                    .text(text.to_string())
                    .font_size(size::BODY + 1.)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .color(fg),
            ),
    )
}

/// A round icon button beside the main one.
fn round_button(icon: impl IntoElement) -> Button {
    Button::new()
        .background(alpha(T.text, 0.1))
        .hover_background(alpha(T.text, 0.16))
        .border_fill(Color::TRANSPARENT)
        .corner_radius(RADIUS_PILL)
        .padding(14.)
        .child(icon)
}

fn actions(mut ui: Ui, st: &Status) -> impl IntoElement {
    let busy = st.operation.is_some();
    let needs_repair = st.proton.is_none() || st.patches.as_ref().is_none_or(|p| !p.intact);
    let ready = st.installed && st.session_pid.is_none() && !busy && !needs_repair;
    let main: Element = if !st.installed {
        main_button(None, t("play-install"), true)
            .on_press(move |_| ui.screen.set(Screen::Wizard))
            .into_element()
    } else if st.session_pid.is_some() {
        main_button(None, t("play-stop"), false)
            .on_press(move |_| ui.run(async { svc().stop().await }, |_| {}))
            .into_element()
    } else if busy {
        main_button(None, t("cancel"), false)
            .on_press(|_| svc().cancel())
            .into_element()
    } else if needs_repair {
        main_button(Some(Icon::Wrench), t("play-repair"), true)
            .on_press(move |_| ui.run(async { svc().repair().await }, |_| {}))
            .into_element()
    } else {
        main_button(Some(Icon::Play), t("play"), true)
            .on_press(move |_| {
                ui.run(
                    async { svc().launch(LaunchOpts { allow_flat: false }).await },
                    |_| {},
                )
            })
            .into_element()
    };
    rect()
        .horizontal()
        .spacing(space::S)
        .cross_align(Alignment::Center)
        .child(main)
        .maybe_child(ready.then(|| {
            round_button(Icon::Monitor.view(20., T.text)).on_press(move |_| {
                ui.run(
                    async { svc().launch(LaunchOpts { allow_flat: true }).await },
                    |_| {},
                )
            })
        }))
        .child(round_button(settings_icon()).on_press(move |_| ui.screen.set(Screen::Settings)))
}

/// Top-bar entry for Unilux's own updates: install, then restart.
fn update_badge(ui: Ui) -> Option<Element> {
    if *ui.restart_ready.read() {
        return Some(
            Button::new()
                .filled()
                .on_press(|_| crate::restart())
                .child(t("update-restart"))
                .into_element(),
        );
    }
    let info = ui.update.read().clone()?;
    let rel = info.available?;
    let text = format!("{} {}", t("update-available"), rel.version);
    let r = rect()
        .horizontal()
        .spacing(space::S)
        .cross_align(Alignment::Center)
        .child(
            label()
                .text(text)
                .font_size(size::SMALL)
                .font_weight(FontWeight::MEDIUM)
                .color(T.text),
        );
    Some(if info.can_apply {
        r.child(
            Button::new()
                .compact()
                .on_press(move |_| ui.update_now())
                .child(t("update-install")),
        )
        .into_element()
    } else {
        r.child(muted(t("update-package"))).into_element()
    })
}
