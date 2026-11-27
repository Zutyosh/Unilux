//! VR: whether the chain from headset to game is complete, and what holds frames back.

use std::time::Duration;

use freya::prelude::*;
use unilux_system::{OpenVrLayer, RuntimeKind, VrStatus, VrTuning};

use crate::i18n::{t, t_args};
use crate::state::{Section, Ui};
use crate::svc;
use crate::theme::{T, alpha, size, space};
use crate::widgets::{Tone, banner, body, chip, divided, group, mono_box, muted};

#[derive(PartialEq)]
pub struct VrSection(pub Ui);

impl Component for VrSection {
    fn render(&self) -> impl IntoElement {
        view(self.0)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum St {
    Ok,
    Todo,
    Wait,
}

struct Step {
    name: String,
    what: String,
    value: Option<String>,
    note: Option<&'static str>,
    st: St,
    chip: &'static str,
    todo: Option<String>,
    cmd: Option<String>,
}

impl Step {
    fn new(name: impl Into<String>, what: impl Into<String>, st: St, chip: &'static str) -> Self {
        Self {
            name: name.into(),
            what: what.into(),
            value: None,
            note: None,
            st,
            chip,
            todo: None,
            cmd: None,
        }
    }
    fn value(mut self, v: impl Into<String>) -> Self {
        self.value = Some(v.into());
        self
    }
    fn todo(mut self, v: impl Into<String>) -> Self {
        self.todo = Some(v.into());
        self
    }
    fn cmd(mut self, v: impl Into<String>) -> Self {
        self.cmd = Some(v.into());
        self
    }
    fn note(mut self, v: &'static str) -> Self {
        self.note = Some(v);
        self
    }
}

fn layer_name(l: OpenVrLayer) -> &'static str {
    match l {
        OpenVrLayer::Xrizer => "xrizer",
        OpenVrLayer::OpenComposite => "OpenComposite",
        OpenVrLayer::VapoR => "VapoR",
        OpenVrLayer::SteamVr => "SteamVR",
        OpenVrLayer::Other => "?",
    }
}

/// The chain, in the order a frame travels the other way: runtime, service, bridge.
fn chain(v: &VrStatus) -> Vec<Step> {
    let kind = v.active.as_ref().map(|r| r.kind);
    let name = v
        .active
        .as_ref()
        .map(|r| r.name.clone().unwrap_or_else(|| r.kind.to_string()));
    let installed = v
        .installed
        .iter()
        .map(|k| k.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let broken = v.active.as_ref().is_some_and(|r| !r.library.exists());

    let what1 = t("vr-program-talks-headset-wivrn");
    let mut s1 = match (&name, v.installed.is_empty()) {
        (Some(n), _) if broken => {
            Step::new(t("vr-vr-runtime"), what1, St::Todo, t("vr-runtime-broken"))
                .value(n.clone())
                .todo(t("vr-points-file-no-longer"))
        }
        (Some(n), _) => {
            Step::new(t("vr-vr-runtime"), what1, St::Ok, t("vr-active")).value(n.clone())
        }
        (None, true) => Step::new(t("vr-vr-runtime"), what1, St::Todo, t("vr-runtime-missing"))
            .todo(t("vr-install-wivrn-standalone-headset")),
        (None, false) => Step::new(t("vr-vr-runtime"), what1, St::Todo, t("vr-inactive"))
            .value(installed.clone())
            .todo(t_args(
                "vr-start-then-connect-headset",
                &[("runtime", installed.clone())],
            )),
    };
    if kind == Some(RuntimeKind::SteamVr) {
        s1 = s1.note(t("vr-valve-only-ships-steamvr"));
    }

    let what2 = t("vr-has-running-while-play");
    let s2 = match (&name, v.service_running) {
        (None, _) => Step::new(t("vr-runtime-service"), what2, St::Wait, t("vr-waiting")),
        (Some(_), true) => Step::new(t("vr-runtime-service"), what2, St::Ok, t("vr-running")),
        (Some(n), false) => Step::new(t("vr-runtime-service"), what2, St::Todo, t("vr-stopped"))
            .todo(t_args(
                "vr-start-service-then-connect",
                &[("runtime", n.clone())],
            )),
    };

    let what3 = t("vr-somnium-speaks-openvr-layer");
    let needs = matches!(
        kind,
        Some(RuntimeKind::WiVRn | RuntimeKind::Monado | RuntimeKind::Envision)
    );
    let n3 = t("vr-openvr-bridge");
    let s3 = if kind.is_none() {
        Step::new(n3, what3, St::Wait, t("vr-waiting"))
    } else if !needs {
        Step::new(n3, what3, St::Wait, t("vr-not-needed"))
            .value(t("vr-runtime-speaks-openvr-itself"))
    } else {
        match &v.openvr {
            None => Step::new(n3, what3, St::Todo, t("vr-bridge-missing"))
                .todo(t("vr-install-xrizer-opencomposite-runtime")),
            Some(o) if !o.exists => Step::new(n3, what3, St::Todo, t("vr-bridge-broken"))
                .value(layer_name(o.layer))
                .todo(t_args(
                    "vr-registered-file-does-not",
                    &[("file", o.path.display().to_string())],
                )),
            Some(o) if o.layer == OpenVrLayer::SteamVr => {
                Step::new(n3, what3, St::Todo, t("vr-wrong-target"))
                    .value("SteamVR")
                    .todo(t("vr-openvr-still-points-steamvr"))
            }
            Some(o) => Step::new(n3, what3, St::Ok, t("vr-place")).value(layer_name(o.layer)),
        }
    };
    vec![s1, s2, s3]
}

/// Settings outside Unilux that decide whether frames arrive on time.
fn tuning(t: &VrTuning) -> Vec<Step> {
    let mut out = Vec::new();
    if let Some(c) = &t.compositor {
        let name = crate::i18n::t("vr-compositor-priority");
        let what = t_args("vr-when-cpu-busy-has", &[("compositor", c.name.clone())]);
        out.push(match c.realtime {
            Some(true) => Step::new(name, what, St::Ok, crate::i18n::t("vr-ok")),
            Some(false) => Step::new(name, what, St::Todo, crate::i18n::t("vr-fix"))
                .todo(crate::i18n::t("vr-allow-once-then-restart"))
                .cmd(format!("sudo setcap CAP_SYS_NICE=+ep {}", c.path.display())),
            None => Step::new(name, what, St::Wait, crate::i18n::t("vr-unknown"))
                .todo(crate::i18n::t("vr-getcap-not-installed-so")),
        });
    }
    for p in &t.amd_power {
        let Some(idx) = p.vr_index else { continue };
        let name = crate::i18n::t("vr-amd-gpu-power-profile");
        let what = crate::i18n::t("vr-vr-load-comes-short");
        out.push(if p.active == "VR" {
            Step::new(name, what, St::Ok, crate::i18n::t("vr-vr-profile"))
        } else {
            Step::new(name, what, St::Todo, crate::i18n::t("vr-fix"))
                .value(t_args(
                    "vr-active-profile",
                    &[("profile", p.active.clone())],
                ))
                .todo(crate::i18n::t("vr-lasts-until-reboot-keep"))
                .cmd(format!(
                    "echo {idx} | sudo tee /sys/class/drm/{}/device/pp_power_profile_mode",
                    p.card
                ))
        });
    }
    if !t.overlays.is_empty() {
        out.push(
            Step::new(
                crate::i18n::t("vr-xr-overlay"),
                crate::i18n::t("vr-overlay-second-client-compositor"),
                St::Wait,
                crate::i18n::t("vr-overlay-running"),
            )
            .value(t.overlays.join(", ")),
        );
    }
    out
}

fn tone(s: St) -> Tone {
    match s {
        St::Ok => Tone::Ok,
        St::Todo => Tone::Warn,
        St::Wait => Tone::Neutral,
    }
}

fn step_row(num: Option<usize>, s: &Step) -> Element {
    let tone = tone(s.st);
    let badge = num.map(|n| {
        rect()
            .width(Size::px(26.))
            .height(Size::px(26.))
            .corner_radius(13.)
            .background(alpha(tone.color(), 0.16))
            .main_align(Alignment::Center)
            .cross_align(Alignment::Center)
            .child(
                label()
                    .text(n.to_string())
                    .font_size(size::SMALL)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .color(tone.color()),
            )
    });
    rect()
        .width(Size::fill())
        .horizontal()
        .content(Content::Flex)
        .padding((space::M, space::L))
        .spacing(space::M)
        .maybe_child(badge)
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(space::XS)
                .child(body(s.name.clone()))
                .child(muted(s.what.clone()))
                .maybe_child(s.value.clone().map(|v| {
                    label()
                        .text(v)
                        .font_size(size::SMALL)
                        .font_weight(FontWeight::MEDIUM)
                        .color(T.text)
                }))
                .maybe_child(s.note.map(muted))
                .maybe_child(
                    s.todo
                        .clone()
                        .map(|t| label().text(t).font_size(size::SMALL).color(T.text)),
                )
                .maybe_child(s.cmd.clone().map(mono_box)),
        )
        .child(chip(s.chip, tone))
        .into_element()
}

fn view(mut ui: Ui) -> impl IntoElement {
    let mut tick = use_state(|| 0u32);
    let mut tun = use_state(|| svc().system().vr_tuning);
    let mut wait = use_state(|| svc().config().wait_for_headset);
    let wait_setter = WritableSetter::new(wait, move |v: bool| {
        let mut cfg = svc().config();
        cfg.wait_for_headset = v;
        match svc().set_config(cfg) {
            Ok(()) => wait.set(v),
            Err(e) => ui.error.set(Some(e.to_string())),
        }
    });
    let mut perf = use_state(|| svc().config().performance);
    let perf_setter = WritableSetter::new(perf, move |v: bool| {
        let mut cfg = svc().config();
        cfg.performance = v;
        match svc().set_config(cfg) {
            Ok(()) => perf.set(v),
            Err(e) => ui.error.set(Some(e.to_string())),
        }
    });
    let perf_sub = if svc().performance_wrapper().is_some() {
        t("vr-cachyos-s-game-performance")
    } else {
        t("vr-shader-cache-grows")
    };
    // The runtime comes and goes while the user starts WiVRn: follow it.
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let n = *tick.peek();
            tick.set(n.wrapping_add(1));
        }
    });
    let _ = *tick.read();

    let v = svc().vr_status();
    let steps = chain(&v);
    let todo = steps.iter().filter(|s| s.st == St::Todo).count();
    let ready = v.ready && todo == 0;
    let name = v
        .active
        .as_ref()
        .map(|r| r.name.clone().unwrap_or_else(|| r.kind.to_string()))
        .unwrap_or_default();

    let recheck = Button::new()
        .flat()
        .on_press(move |_| {
            tun.set(svc().system().vr_tuning);
            let n = *tick.peek();
            tick.set(n.wrapping_add(1));
        })
        .child(t("vr-check-again"))
        .into_element();
    let verdict = if ready {
        banner(
            Tone::Ok,
            t("vr-ready-vr"),
            t_args("vr-running-game-will-find", &[("runtime", name)]),
            Some(recheck),
        )
    } else {
        let head = if todo == 1 {
            t("vr-one-step-fix-before").to_string()
        } else {
            t_args(
                "vr-steps-fix-before-vr",
                &[("count", todo.max(1).to_string())],
            )
        };
        banner(
            Tone::Warn,
            head,
            t("vr-until-then-game-starts"),
            Some(recheck),
        )
    };

    let chain_rows = divided(
        steps
            .iter()
            .enumerate()
            .map(|(i, s)| step_row(Some(i + 1), s))
            .collect(),
    );

    let t = tun.read().clone();
    let mut tune_steps = tuning(&t);
    if tune_steps.is_empty() {
        tune_steps.push(Step::new(
            crate::i18n::t("vr-nothing-report"),
            crate::i18n::t("vr-no-compositor-amd-power"),
            St::Ok,
            crate::i18n::t("vr-ok"),
        ));
    }
    let tune_rows = divided(tune_steps.iter().map(|s| step_row(None, s)).collect());

    let mut details = Vec::new();
    if let Some(r) = &v.active {
        details.push(crate::widgets::row(
            crate::i18n::t("vr-runtime-manifest"),
            Some(r.manifest.display().to_string()),
            None,
        ));
        details.push(crate::widgets::row(
            crate::i18n::t("vr-runtime-library"),
            Some(r.library.display().to_string()),
            None,
        ));
    }
    if let Some(o) = &v.openvr {
        details.push(crate::widgets::row(
            crate::i18n::t("vr-openvr-registration"),
            Some(o.path.display().to_string()),
            None,
        ));
    }

    rect()
        .spacing(space::XL)
        .child(crate::widgets::page_header(
            crate::i18n::t("vr-vr-headset"),
            crate::i18n::t("vr-somnium-space-played-vr"),
        ))
        .child(verdict)
        .child(group(crate::i18n::t("vr-headset-game"), chain_rows))
        .child(group(
            crate::i18n::t("vr-launch"),
            vec![
                crate::widgets::row(
                    crate::i18n::t("vr-wait-headset"),
                    Some(crate::i18n::t("vr-if-runtime-not-ready").into()),
                    Some(Switch::new(wait_setter).into_element()),
                ),
                crate::widgets::row(
                    crate::i18n::t("vr-performance-mode"),
                    Some(perf_sub.into()),
                    Some(Switch::new(perf_setter).into_element()),
                ),
            ],
        ))
        .child(group(crate::i18n::t("vr-smooth-frames"), tune_rows))
        .child(group(
            crate::i18n::t("vr-what-happened-game"),
            vec![crate::widgets::row(
                crate::i18n::t("vr-frame-rate-sessions"),
                Some(crate::i18n::t("vr-each-game-measured-mangohud").into()),
                Some(
                    Button::new()
                        .on_press(move |_| ui.section.set(Section::Recipes))
                        .child(crate::i18n::t("vr-open-sessions"))
                        .into_element(),
                ),
            )],
        ))
        .maybe_child(
            (!details.is_empty())
                .then(|| group(crate::i18n::t("vr-technical-details"), divided(details))),
        )
}
