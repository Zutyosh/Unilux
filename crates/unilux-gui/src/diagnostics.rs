//! Diagnostics: what Unilux checked, what is wrong, and how to fix it.

use freya::prelude::*;
use unilux_core::Severity;
use unilux_diag::checks::CHECKS;
use unilux_diag::{Finding, FixId};

use crate::i18n::{Lang, current, t, t_with};
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

/// French text for the engine's English findings: (title, title in French, advice in French).
const FR: &[(&str, &str, &str)] = &[
    (
        "python3 is not installed",
        "python3 n'est pas installé",
        "umu-run a besoin de Python 3.10 ou plus récent. Installez-le avec votre gestionnaire de paquets.",
    ),
    (
        "No Vulkan driver found",
        "Aucun pilote Vulkan trouvé",
        "Proton dessine le jeu avec Vulkan. Installez le pilote Vulkan de votre carte graphique (mesa-vulkan-drivers, vulkan-radeon, nvidia-utils...).",
    ),
    (
        "No GPU found under /sys/class/drm",
        "Aucune carte graphique détectée",
        "",
    ),
    (
        "GitHub is not reachable",
        "GitHub est injoignable",
        "Les installations et les réparations se téléchargent depuis GitHub. Vérifiez votre connexion, votre proxy ou votre pare-feu.",
    ),
    (
        "Somnium Space is not installed in the prefix",
        "Somnium Space n'est pas installé",
        "Lancez l'installation avec l'installeur Windows de Somnium, sur somniumspace.com.",
    ),
    (
        "umu-run is missing",
        "umu-run est absent",
        "Une réparation le télécharge à nouveau.",
    ),
    (
        "Proton is missing",
        "Proton est absent",
        "Une réparation le télécharge à nouveau.",
    ),
    (
        "No Proton version recorded",
        "Aucune version de Proton enregistrée",
        "Une réparation en choisit une.",
    ),
    (
        "No Somnium patch bundle is installed in this Proton",
        "Les correctifs Somnium ne sont pas dans ce Proton",
        "Sans eux, le launcher ne peut pas se connecter : Wine d'origine ne gère pas les certificats clients TLS dont Somnium a besoin.",
    ),
    (
        "OpenXR detection is replaced by manual settings",
        "La détection VR est remplacée par des réglages manuels",
        "Ces réglages remplacent la détection automatique. Si vous en avez besoin, signalez votre runtime pour que la détection le gère.",
    ),
    (
        "Unilux is running inside a Flatpak sandbox",
        "Unilux tourne dans un bac à sable Flatpak",
        "Le conteneur de Proton ne peut pas démarrer depuis Flatpak sans accès à l'hôte. Utilisez l'AppImage ou le paquet natif.",
    ),
    (
        "The start-menu entry is missing",
        "Le raccourci du menu est absent",
        "Une réparation le recrée.",
    ),
    (
        "TLS errors in the launch log",
        "Erreurs TLS dans le journal de lancement",
        "Les bibliothèques secur32 et crypt32 corrigées ne sont probablement pas utilisées. Lancez une réparation.",
    ),
    (
        "Certificate errors in the launch log",
        "Erreurs de certificat dans le journal de lancement",
        "Les bibliothèques corrigées ne sont probablement pas utilisées. Lancez une réparation.",
    ),
    (
        "Vulkan errors in the launch log",
        "Erreurs Vulkan dans le journal de lancement",
        "Vérifiez le pilote de la carte graphique et que Vulkan fonctionne (vulkaninfo).",
    ),
    (
        "The game could not reach the OpenXR runtime",
        "Le jeu n'a pas trouvé le runtime VR",
        "Branchez le casque avant de lancer, puis vérifiez que le runtime le voit.",
    ),
    (
        "Proton's container failed to start",
        "Le conteneur de Proton n'a pas démarré",
        "La ligne en détail nomme souvent un chemin absent ou une permission manquante.",
    ),
    (
        "yt-dlp cannot start under this Proton",
        "yt-dlp ne démarre pas avec ce Proton",
        "Le yt-dlp.exe de Somnium vérifie le nom de son dossier temporaire, que Wine raccourcit : aucune vidéo YouTube ne charge. Utilisez une recette avec le correctif pyinstaller-tempnam (la recette recommandée l'a) : unilux recipe use recommended.",
    ),
    (
        "Somnium refused some video links",
        "Somnium a refusé des liens vidéo",
        "Somnium ne lit que les liens des domaines de sa propre liste ; le lecteur reste vide sans erreur. C'est une règle du jeu, pas de Wine : utilisez un lien YouTube ou un hébergeur autorisé par le créateur du monde.",
    ),
    (
        "Some YouTube links could not be extracted",
        "Certains liens YouTube n'ont pas pu être extraits",
        "yt-dlp a démarré mais n'a pas pu lire ces liens : vidéos privées, avec limite d'âge, ou YouTube a changé. Essayez une autre vidéo ; si toutes échouent, Somnium doit mettre à jour yt-dlp.",
    ),
    (
        "Freezes in the last session",
        "Gels pendant la dernière partie",
        "Des gels irréguliers coïncident souvent avec des chargements : mondes, arrivée d'avatars, vidéos.",
    ),
    (
        "The GPU idles during the slowdowns",
        "Le GPU est au repos pendant les ralentissements",
        "Les images sont en retard alors que le GPU ne travaille pas : le thread principal du jeu, le compositeur ou un chargement (avatars, mondes, vidéo) les retient. Beaucoup d'avatars sur un processeur à 6 cœurs, c'est la limite habituelle.",
    ),
    (
        "The GPU clock drops during the slowdowns",
        "La fréquence du GPU chute pendant les ralentissements",
        "La gestion d'énergie baisse la fréquence au moment où il faut des images. Utilisez le profil d'alimentation VR (page VR) ou un niveau de performance fixe.",
    ),
    (
        "The GPU is saturated during the slowdowns",
        "Le GPU est saturé pendant les ralentissements",
        "Baissez la résolution dans WiVRn ou les graphismes du jeu, ou masquez les avatars lointains.",
    ),
    (
        "Slowdowns in the last session",
        "Ralentissements pendant la dernière partie",
        "Ni la charge ni la fréquence du GPU ne les expliquent, ou MangoHud ne les a pas mesurées. Ils viennent en général des chargements sur le thread principal du jeu : mondes, arrivée d'avatars, vidéos.",
    ),
];

/// Titles with a value inside: (prefix, suffix, French title with {}, French advice with {}).
const FR_DYN: &[(&str, &str, &str, &str)] = &[
    (
        "Patched files from ",
        " were replaced",
        "Les fichiers corrigés de {} ont été remplacés",
        "Une mise à jour de Proton ou une copie manuelle les a écrasés. Une réparation les remet.",
    ),
    (
        "Patch bundle ",
        " installed and intact",
        "Correctifs {} installés et intacts",
        "",
    ),
    (
        "The catalog has no patch bundle for ",
        "",
        "Le catalogue n'a pas de correctifs pour {}",
        "Passez à un Proton vérifié, ou construisez les correctifs vous-même (Recettes) et testez-les.",
    ),
    (
        "The patch bundle for ",
        " is untested",
        "Les correctifs pour {} ne sont pas testés",
        "Ils ont été construits pour un Proton voisin. Si la connexion ou la vidéo échoue, passez à un Proton vérifié et signalez le résultat.",
    ),
    (
        "python3 is ",
        ", older than 3.10",
        "python3 est en version {}, antérieure à 3.10",
        "umu-run a besoin de la 3.10 ou plus comme python3 dans le PATH. Sur une LTS ancienne, cela passe souvent par une mise à jour de la distribution.",
    ),
    (
        "Somnium Space is running (",
        " processes)",
        "Somnium Space est en cours d'exécution ({} processus)",
        "",
    ),
    (
        "The last session pushed ",
        " GB to swap",
        "La dernière partie a envoyé {} Go dans le swap",
        "Les pages que le jeu réutilise reviennent du disque et l'image les attend : gels au chargement des mondes et des avatars. Activez zram (zram-generator ou le paquet zram de votre distribution) pour garder le swap en RAM compressée, et fermez navigateur et applications lourdes avant de jouer.",
    ),
    (
        "The last session pushed ",
        " GB to zram",
        "La dernière partie a envoyé {} Go dans zram",
        "zram garde le swap peu coûteux, mais le noyau a quand même évincé de la mémoire du jeu. Fermez les applications lourdes avant de jouer ; avec zram, un vm.swappiness entre 100 et 180 évince les pages inactives avant celles du jeu.",
    ),
    (
        "The game freezes every ",
        " s",
        "Le jeu se fige toutes les {} s",
        "Des gels à intervalle fixe viennent d'un minuteur dans le jeu (nettoyage périodique, sondage, tâche réseau ou voix), pas du GPU ni du compositeur. Notez ce que vous faisiez et comparez avec le micro et le chat vocal coupés.",
    ),
];

/// Title and advice in the interface language; the engine's English when no translation exists.
fn localized(f: &Finding) -> (String, Option<String>) {
    if current() == Lang::En {
        return (f.title.clone(), f.advice.clone());
    }
    let keep = |fr: &str| {
        (!fr.is_empty())
            .then(|| fr.to_string())
            .or_else(|| f.advice.clone())
    };
    if let Some((_, t, a)) = FR.iter().find(|(en, _, _)| *en == f.title) {
        return (t.to_string(), keep(a));
    }
    for (pre, suf, t, a) in FR_DYN {
        if let Some(mid) = f.title.strip_prefix(pre).and_then(|r| r.strip_suffix(suf)) {
            return (t.replacen("{}", mid, 1), keep(&a.replacen("{}", mid, 1)));
        }
    }
    if let Some(p) = f.title.strip_suffix(" is older than Proton 9") {
        return (
            format!("{p} est plus ancien que Proton 9"),
            Some("Proton 8 et antérieurs ne savent pas initialiser la VR. Choisissez Proton 9 ou plus récent.".into()),
        );
    }
    if let Some(d) = f.title.strip_suffix(" is an immutable distribution") {
        return (
            format!("{d} est une distribution immuable"),
            Some("Unilux n'écrit que dans votre dossier personnel, cela devrait fonctionner. Les retours sont bienvenus.".into()),
        );
    }
    (f.title.clone(), f.advice.clone())
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
            t_with(many, &[n.to_string()])
        }
    };
    let verdict = if findings.is_none() {
        banner(
            Tone::Neutral,
            t("diag-checking-2"),
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
            t_with(
                "diag-checks-passed-install-system",
                &[CHECKS.len().to_string()],
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
                                        t_with("diag-report-saved", &[p.display().to_string()]),
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
