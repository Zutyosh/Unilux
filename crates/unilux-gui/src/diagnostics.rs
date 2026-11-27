//! Diagnostics: what Unilux checked, what is wrong, and how to fix it.

use freya::prelude::*;
use unilux_core::Severity;
use unilux_diag::checks::CHECKS;
use unilux_diag::{Finding, FixId};

use crate::i18n::{t, t_args, t_named};
use crate::state::{Section, Ui};
use crate::svc;
use crate::theme::space;
use crate::widgets::{
    Tone, banner, body, chip, divided, group, mono_box, muted, row, severity_tone,
};

#[derive(PartialEq)]
pub struct DiagnosticsSection(pub Ui);

impl Component for DiagnosticsSection {
    fn render(&self) -> impl IntoElement {
        view(self.0)
    }
}

async fn run_checks(mut ui: Ui, mut running: State<bool>) {
    running.set(true);
    if let Ok(v) = tokio::spawn(async { svc().doctor(true).await }).await {
        ui.findings.set(Some(v));
    }
    running.set(false);
}

/// What each check looks at, in plain words.
fn check_meta(id: &str) -> (&'static str, &'static str) {
    match id {
        "python" => (t("diag-check-python-name"), t("diag-check-python-what")),
        "gpu" => (t("diag-check-gpu-name"), t("diag-check-gpu-what")),
        "network" => (t("diag-check-network-name"), t("diag-check-network-what")),
        "install" => (t("diag-check-install-name"), t("diag-check-install-what")),
        "patches" => (t("diag-check-patches-name"), t("diag-check-patches-what")),
        "proton" => (t("diag-check-proton-name"), t("diag-check-proton-what")),
        "openxr" => (t("diag-check-openxr-name"), t("diag-check-openxr-what")),
        "session" => (t("diag-check-session-name"), t("diag-check-session-what")),
        "desktop_entry" => (
            t("diag-check-desktop_entry-name"),
            t("diag-check-desktop_entry-what"),
        ),
        "sandbox" => (t("diag-check-sandbox-name"), t("diag-check-sandbox-what")),
        "launch_log" => (
            t("diag-check-launch_log-name"),
            t("diag-check-launch_log-what"),
        ),
        "vr_perf" => (t("diag-check-vr_perf-name"), t("diag-check-vr_perf-what")),
        "memory" => (t("diag-check-memory-name"), t("diag-check-memory-what")),
        "video" => (t("diag-check-video-name"), t("diag-check-video-what")),
        _ => ("?", ""),
    }
}

/// Title and advice in the interface language; the engine's English when no translation exists.
fn localized(f: &Finding) -> (String, Option<String>) {
    let key = format!("finding-{}", f.id);
    match t_named(&format!("{key}-title"), &f.args) {
        Some(title) => (
            title,
            t_named(&format!("{key}-advice"), &f.args).or_else(|| f.advice.clone()),
        ),
        None => (f.title.clone(), f.advice.clone()),
    }
}

fn fix_label(fix: FixId) -> &'static str {
    match fix {
        FixId::Repair => t("diag-repair"),
        FixId::StopSession => t("diag-stop-game"),
        FixId::ClearOpenxrOverride => t("diag-use-automatic-detection"),
        FixId::ClearDownloads => t("diag-clear-downloads"),
    }
}

fn sev_label(s: Severity) -> &'static str {
    match s {
        Severity::Error => t("diag-problem"),
        Severity::Warn => t("diag-worth-checking"),
        Severity::Info => t("diag-info"),
    }
}

/// The VR page owns these: the headset chain and the compositor settings.
fn vr_owned(f: &Finding) -> bool {
    (f.check == "openxr" && f.fix.is_none())
        || (f.check == "vr_perf"
            && (f.title.contains("compositor at realtime")
                || f.title.contains("VR power profile")
                || f.title.starts_with("XR overlay")))
}

/// The raw evidence of a finding (paths, commands), folded until asked for.
#[derive(PartialEq)]
struct Evidence(String);

impl Component for Evidence {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| false);
        let shown = *open.read();
        rect()
            .width(Size::fill())
            .spacing(space::XS)
            .child(
                Button::new()
                    .flat()
                    .compact()
                    .padding((space::XS, 0.))
                    .on_press(move |_| open.set(!shown))
                    .child(
                        label()
                            .text(if shown {
                                t("diag-hide-details")
                            } else {
                                t("diag-details")
                            })
                            .font_size(crate::theme::size::SMALL)
                            .color(crate::theme::T.muted),
                    ),
            )
            .maybe_child(shown.then(|| mono_box(self.0.clone())))
    }
}

fn finding_row(ui: Ui, running: State<bool>, f: &Finding) -> Element {
    let (head, advice) = localized(f);
    let fix = f.fix;
    let side = rect()
        .spacing(space::S)
        .cross_align(Alignment::End)
        .child(chip(sev_label(f.severity), severity_tone(f.severity)))
        .maybe_child(fix.map(|fix| {
            Button::new()
                .on_press(move |_| {
                    ui.run(async move { svc().apply_fix(fix, false).await }, move |o| {
                        ui.notice(Severity::Info, o.message);
                        spawn_in_window(run_checks(ui, running));
                    })
                })
                .child(fix_label(fix))
        }));
    rect()
        .width(Size::fill())
        .horizontal()
        .content(Content::Flex)
        .padding((space::M, space::L))
        .spacing(space::M)
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(space::XS)
                .child(body(head))
                .maybe_child(advice.map(muted))
                .maybe_child((!f.evidence.is_empty()).then(|| {
                    Evidence(
                        f.evidence
                            .iter()
                            .take(4)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                    .into_element()
                })),
        )
        .child(side)
        .into_element()
}

fn view(mut ui: Ui) -> impl IntoElement {
    let running = use_state(|| false);
    use_future(move || async move { run_checks(ui, running).await });
    let busy = *running.read();
    let findings = ui.findings.read().clone();
    let list: Vec<Finding> = findings.clone().unwrap_or_default();
    let shown: Vec<&Finding> = list.iter().filter(|f| !vr_owned(f)).collect();
    let vr_attention = list
        .iter()
        .any(|f| vr_owned(f) && f.severity != Severity::Info);
    let errors = shown
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warns = shown
        .iter()
        .filter(|f| f.severity == Severity::Warn)
        .count()
        + usize::from(vr_attention);

    let rerun = Button::new()
        .enabled(!busy)
        .on_press(move |_| {
            spawn_in_window(run_checks(ui, running));
        })
        .child(if busy {
            t("diag-checking")
        } else {
            t("diag-check-again")
        })
        .into_element();
    let plural = |n: usize, one: &str, many: &str| {
        if n == 1 {
            t(one).to_string()
        } else {
            t_args(many, &[("count", n.to_string())])
        }
    };
    let verdict = if findings.is_none() {
        banner(
            Tone::Neutral,
            t("diag-banner-checking"),
            t("diag-takes-few-seconds"),
            Some(rerun),
        )
    } else if errors > 0 {
        banner(
            Tone::Error,
            plural(errors, "diag-one-problem-fix", "diag-problems-fix"),
            t("diag-start-these-game-probably"),
            Some(rerun),
        )
    } else if warns > 0 {
        banner(
            Tone::Warn,
            plural(
                warns,
                "diag-one-thing-worth-checking",
                "diag-things-worth-checking",
            ),
            t("diag-nothing-broken-but-these"),
            Some(rerun),
        )
    } else {
        banner(
            Tone::Ok,
            t("diag-all-good"),
            t_args(
                "diag-checks-passed-install-system",
                &[("count", CHECKS.len().to_string())],
            ),
            Some(rerun),
        )
    };

    let mut to_fix: Vec<Element> = shown
        .iter()
        .filter(|f| f.severity != Severity::Info)
        .map(|f| finding_row(ui, running, f))
        .collect();
    if vr_attention {
        to_fix.push(row(
            t("diag-vr-headset-needs-attention"),
            Some(t("diag-step-missing-between-headset").into()),
            Some(
                Button::new()
                    .on_press(move |_| ui.section.set(Section::Vr))
                    .child(t("diag-open-vr-page"))
                    .into_element(),
            ),
        ));
    }
    let infos: Vec<Element> = shown
        .iter()
        .filter(|f| f.severity == Severity::Info)
        .map(|f| finding_row(ui, running, f))
        .collect();

    let checked: Vec<Element> = CHECKS
        .iter()
        .map(|c| {
            let (name, what) = check_meta(c.id);
            let chip_el = if findings.is_none() {
                chip("...", Tone::Neutral)
            } else {
                match list
                    .iter()
                    .filter(|f| f.check == c.id)
                    .map(|f| f.severity)
                    .max()
                {
                    Some(Severity::Error) => chip(t("diag-problem"), Tone::Error),
                    Some(Severity::Warn) => chip(t("diag-worth-checking"), Tone::Warn),
                    _ => chip(t("diag-ok"), Tone::Ok),
                }
            };
            row(name, Some(what.to_string()), Some(chip_el.into_element()))
        })
        .collect();

    let help = vec![
        row(
            t("diag-anonymous-bug-report"),
            Some(t("diag-json-file-these-results").into()),
            Some(
                Button::new()
                    .on_press(move |_| {
                        spawn_in_window(async move {
                            if let Ok(r) = tokio::spawn(async { svc().report().await }).await {
                                let p = svc().paths().logs_dir().join("bug-report.json");
                                let _ = std::fs::create_dir_all(svc().paths().logs_dir());
                                match std::fs::write(&p, r) {
                                    Ok(()) => ui.notice(
                                        Severity::Info,
                                        t_args(
                                            "diag-report-saved",
                                            &[("path", p.display().to_string())],
                                        ),
                                    ),
                                    Err(e) => ui.error.set(Some(e.to_string())),
                                }
                            }
                        });
                    })
                    .child(t("diag-save-report"))
                    .into_element(),
            ),
        ),
        row(
            t("diag-launch-logs"),
            Some(t("diag-what-proton-game-printed").into()),
            Some(
                Button::new()
                    .flat()
                    .on_press(move |_| ui.section.set(Section::Logs))
                    .child(t("diag-open-logs"))
                    .into_element(),
            ),
        ),
    ];

    rect()
        .spacing(space::XL)
        .child(crate::widgets::page_header(
            t("diag-diagnostics"),
            t("diag-when-something-does-not"),
        ))
        .child(verdict)
        .maybe_child((!to_fix.is_empty()).then(|| group(t("diag-fix"), divided(to_fix))))
        .maybe_child((!infos.is_empty()).then(|| group(t("diag-information"), divided(infos))))
        .child(group(t("diag-what-was-checked"), divided(checked)))
        .child(group(t("diag-ask-help"), divided(help)))
}
