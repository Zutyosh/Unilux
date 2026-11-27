//! Every check is a pure function of a [DiagContext] snapshot, so each one
//! has fixture tests and the same result in the GUI, the CLI and MCP.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use unilux_components::ProtonId;
use unilux_components::sources::LAUNCHER_REL_PATH;
use unilux_core::{Config, Paths, Severity};
use unilux_i18n::ENGLISH;
use unilux_install::InstallState;
use unilux_patches::community::{DipCause, Perf, Video};
use unilux_patches::{PatchStatus, TargetStatus};
use unilux_system::procs::ProcInfo;
use unilux_system::{SystemReport, VrStatus};

/// Fixes Unilux can apply itself. Anything not listed here is advice only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixId {
    /// Re-run the repair pipeline (prefix and Somnium untouched).
    Repair,
    /// Stop every process in the prefix.
    StopSession,
    /// Remove `openxr_env` from the config, back to automatic detection.
    ClearOpenxrOverride,
    /// Delete partial downloads.
    ClearDownloads,
}

impl FixId {
    pub const ALL: &[FixId] = &[
        Self::Repair,
        Self::StopSession,
        Self::ClearOpenxrOverride,
        Self::ClearDownloads,
    ];

    /// What applying it does, in English: for agents, logs and approvals.
    pub fn describe(self) -> String {
        self.describe_in(ENGLISH)
    }

    /// What applying it does, in the given language.
    pub fn describe_in(self, lang: &str) -> String {
        unilux_i18n::t_in(lang, self.message(), &[])
    }

    fn message(self) -> &'static str {
        match self {
            Self::Repair => "diag-fix-repair-describe",
            Self::StopSession => "diag-fix-stop-session-describe",
            Self::ClearOpenxrOverride => "diag-fix-clear-openxr-override-describe",
            Self::ClearDownloads => "diag-fix-clear-downloads-describe",
        }
    }

    /// Whether applying it can lose user state.
    pub fn destructive(self) -> bool {
        matches!(self, Self::StopSession)
    }

    pub fn parse(s: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(s.into())).ok()
    }
}

/// Every finding id, so front ends can check they translate them all.
pub const FINDING_IDS: &[&str] = &[
    "amd-power-profile",
    "compositor-not-realtime",
    "desktop-entry-missing",
    "dips",
    "dips-gpu-bound",
    "dips-gpu-clock",
    "dips-gpu-waiting",
    "flatpak",
    "freezes",
    "freezes-periodic",
    "game-running",
    "github-unreachable",
    "immutable-distro",
    "log-certificate",
    "log-container",
    "log-openxr",
    "log-tls",
    "log-vulkan",
    "no-gpu",
    "no-patches",
    "no-proton",
    "no-vulkan",
    "openxr-override",
    "patches-intact",
    "patches-none-for-proton",
    "patches-replaced",
    "patches-untested",
    "proton-missing",
    "proton-too-old",
    "python-missing",
    "python-old",
    "somnium-missing",
    "swap-disk",
    "swap-zram",
    "umu-missing",
    "video-domain-blocked",
    "video-extract-failed",
    "vr-not-ready",
    "vr-note",
    "vr-ready",
    "xr-overlay",
    "ytdlp-blocked",
];

/// One conclusion of a check. It holds no title or advice text: front ends render
/// `finding-<id>-title` and `finding-<id>-advice` from `unilux-i18n` with
/// [Finding::args]. Serialised (MCP, `--json`, bug reports) it carries the
/// English title and advice, whatever the interface language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub check: &'static str,
    /// Stable name of this kind of finding; front ends translate by it.
    pub id: &'static str,
    /// The values in the title and advice, by name, for translations.
    pub args: Vec<(&'static str, String)>,
    pub severity: Severity,
    /// The facts the conclusion rests on: paths, versions, log lines.
    pub evidence: Vec<String>,
    advice: Advice,
    pub fix: Option<FixId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Advice {
    /// None for machines; front ends still show `finding-<id>-advice` when
    /// their language has one.
    Unstated,
    /// `finding-<id>-advice`, for everyone.
    Message,
    /// Text another crate wrote (the VR runtime's blockers and notes),
    /// passed on as is.
    Verbatim(String),
}

impl Finding {
    fn new(check: &'static str, id: &'static str, severity: Severity) -> Self {
        Self {
            check,
            id,
            args: Vec::new(),
            severity,
            evidence: Vec::new(),
            advice: Advice::Unstated,
            fix: None,
        }
    }
    fn arg(mut self, name: &'static str, value: impl ToString) -> Self {
        self.args.push((name, value.to_string()));
        self
    }
    fn evidence(mut self, e: impl Into<String>) -> Self {
        self.evidence.push(e.into());
        self
    }
    fn advised(mut self) -> Self {
        self.advice = Advice::Message;
        self
    }
    fn advice_text(mut self, text: impl Into<String>) -> Self {
        self.advice = Advice::Verbatim(text.into());
        self
    }
    fn fix(mut self, f: FixId) -> Self {
        self.fix = Some(f);
        self
    }

    fn key(&self, part: &str) -> String {
        format!("finding-{}-{part}", self.id)
    }

    /// The title in the interface language.
    pub fn title(&self) -> String {
        unilux_i18n::t_named(&self.key("title"), &self.args)
            .unwrap_or_else(|| self.title_in(ENGLISH))
    }

    /// The title in the given language.
    pub fn title_in(&self, lang: &str) -> String {
        unilux_i18n::t_in(lang, &self.key("title"), &self.args)
    }

    /// What to do, in one or two sentences, in the interface language.
    pub fn advice(&self) -> Option<String> {
        match &self.advice {
            Advice::Verbatim(text) => Some(text.clone()),
            _ => unilux_i18n::t_named(&self.key("advice"), &self.args),
        }
    }

    /// The advice the engine states, in the given language: what machine
    /// readers get.
    pub fn advice_in(&self, lang: &str) -> Option<String> {
        match &self.advice {
            Advice::Unstated => None,
            Advice::Message => Some(unilux_i18n::t_in(lang, &self.key("advice"), &self.args)),
            Advice::Verbatim(text) => Some(text.clone()),
        }
    }
}

impl Serialize for Finding {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Finding", 8)?;
        st.serialize_field("check", self.check)?;
        st.serialize_field("id", self.id)?;
        st.serialize_field("args", &self.args)?;
        st.serialize_field("severity", &self.severity)?;
        st.serialize_field("title", &self.title_in(ENGLISH))?;
        st.serialize_field("evidence", &self.evidence)?;
        st.serialize_field("advice", &self.advice_in(ENGLISH))?;
        st.serialize_field("fix", &self.fix)?;
        st.end()
    }
}

/// Everything the checks look at, gathered once.
pub struct DiagContext {
    pub paths: Paths,
    pub config: Config,
    pub state: InstallState,
    pub system: SystemReport,
    pub prefix: PathBuf,
    pub proton_dir: Option<PathBuf>,
    pub proton: Option<ProtonId>,
    /// Installed patch bundle and whether its files are untouched.
    pub patches: Option<PatchStatus>,
    /// Best catalog bundle for this Proton; None when there is none.
    pub patch_match: Option<TargetStatus>,
    pub vr: Option<VrStatus>,
    pub processes: Vec<ProcInfo>,
    /// None when the network was not tested.
    pub github_reachable: Option<bool>,
    /// Tail of launch.log.
    pub launch_log: Vec<String>,
    /// Frame pacing of the last recorded session.
    pub last_perf: Option<Perf>,
    /// Video opens and failures of the last recorded session.
    pub last_video: Option<Video>,
    pub exists: Box<dyn Fn(&std::path::Path) -> bool + Send + Sync>,
}

impl DiagContext {
    fn exists(&self, p: &std::path::Path) -> bool {
        (self.exists)(p)
    }
}

pub struct Check {
    pub id: &'static str,
    pub run: fn(&DiagContext) -> Vec<Finding>,
}

pub const CHECKS: &[Check] = &[
    Check {
        id: "python",
        run: python,
    },
    Check {
        id: "gpu",
        run: gpu,
    },
    Check {
        id: "network",
        run: network,
    },
    Check {
        id: "install",
        run: install,
    },
    Check {
        id: "patches",
        run: patches,
    },
    Check {
        id: "proton",
        run: proton,
    },
    Check {
        id: "openxr",
        run: openxr,
    },
    Check {
        id: "session",
        run: session,
    },
    Check {
        id: "desktop_entry",
        run: desktop_entry,
    },
    Check {
        id: "sandbox",
        run: sandbox,
    },
    Check {
        id: "launch_log",
        run: launch_log,
    },
    Check {
        id: "vr_perf",
        run: vr_perf,
    },
    Check {
        id: "memory",
        run: memory,
    },
    Check {
        id: "video",
        run: video,
    },
];

/// Run every check; the most severe findings first.
pub fn run_all(ctx: &DiagContext) -> Vec<Finding> {
    let mut v: Vec<Finding> = CHECKS.iter().flat_map(|c| (c.run)(ctx)).collect();
    v.sort_by_key(|f| std::cmp::Reverse(f.severity));
    v
}

fn python(ctx: &DiagContext) -> Vec<Finding> {
    let hint = ctx
        .system
        .distro
        .as_ref()
        .and_then(|d| d.install_hint("python3"));
    match ctx.system.python {
        None => vec![
            Finding::new("python", "python-missing", Severity::Error)
                .arg("hint", hint.as_deref().unwrap_or("none"))
                .evidence("python3 --version did not run")
                .advised(),
        ],
        Some(v) if !v.supported() => vec![
            Finding::new("python", "python-old", Severity::Error)
                .arg("version", v)
                .evidence(format!("python3 --version reports {v}"))
                .advised(),
        ],
        Some(_) => vec![],
    }
}

fn gpu(ctx: &DiagContext) -> Vec<Finding> {
    let mut out = Vec::new();
    if ctx.system.vulkan_icds.is_empty() {
        out.push(
            Finding::new("gpu", "no-vulkan", Severity::Error)
                .evidence("no *.json in /usr/share/vulkan/icd.d or /etc/vulkan/icd.d")
                .advised(),
        );
    }
    if ctx.system.gpus.is_empty() {
        out.push(Finding::new("gpu", "no-gpu", Severity::Warn));
    }
    out
}

fn network(ctx: &DiagContext) -> Vec<Finding> {
    match ctx.github_reachable {
        Some(false) => vec![
            Finding::new("network", "github-unreachable", Severity::Error)
                .evidence("request to raw.githubusercontent.com failed")
                .advised(),
        ],
        _ => vec![],
    }
}

fn install(ctx: &DiagContext) -> Vec<Finding> {
    let mut out = Vec::new();
    let launcher = ctx.prefix.join(LAUNCHER_REL_PATH);
    if !ctx.exists(&launcher) {
        out.push(
            Finding::new("install", "somnium-missing", Severity::Error)
                .evidence(format!("missing {}", launcher.display()))
                .advised(),
        );
        return out;
    }
    let umu = unilux_install::umu_run(&ctx.paths);
    if !ctx.exists(&umu) {
        out.push(
            Finding::new("install", "umu-missing", Severity::Error)
                .evidence(format!("missing {}", umu.display()))
                .fix(FixId::Repair),
        );
    }
    match &ctx.proton_dir {
        Some(p) if ctx.exists(&p.join("proton")) => {}
        Some(p) => out.push(
            Finding::new("install", "proton-missing", Severity::Error)
                .evidence(format!("missing {}", p.display()))
                .fix(FixId::Repair),
        ),
        None => out.push(Finding::new("install", "no-proton", Severity::Error).fix(FixId::Repair)),
    }
    out
}

fn patches(ctx: &DiagContext) -> Vec<Finding> {
    let Some(p) = &ctx.proton_dir else {
        return vec![];
    };
    if !ctx.exists(&p.join("proton")) {
        return vec![];
    }
    let mut out = Vec::new();
    match &ctx.patches {
        None | Some(PatchStatus { bundle: None, .. }) => out.push(
            Finding::new("patches", "no-patches", Severity::Error)
                .evidence(format!("no .unilux-patches.json in {}", p.display()))
                .advised()
                .fix(FixId::Repair),
        ),
        Some(PatchStatus {
            intact: false,
            changed,
            bundle,
            ..
        }) => {
            let bundle = bundle.as_deref().unwrap_or("?");
            let mut f = Finding::new("patches", "patches-replaced", Severity::Error)
                .arg("bundle", bundle)
                .advised()
                .fix(FixId::Repair);
            for c in changed.iter().take(6) {
                f = f.evidence(format!("changed: {c}"));
            }
            out.push(f);
        }
        Some(PatchStatus {
            bundle: Some(b), ..
        }) => out.push(Finding::new("patches", "patches-intact", Severity::Info).arg("bundle", b)),
    }
    let id = ctx
        .proton
        .as_ref()
        .map(|i| i.to_string())
        .unwrap_or_default();
    match ctx.patch_match {
        None if ctx.proton.is_some() => out.push(
            Finding::new("patches", "patches-none-for-proton", Severity::Warn)
                .arg("proton", &id)
                .advised(),
        ),
        Some(TargetStatus::Untested) => out.push(
            Finding::new("patches", "patches-untested", Severity::Warn)
                .arg("proton", &id)
                .advised(),
        ),
        _ => {}
    }
    out
}

fn proton(ctx: &DiagContext) -> Vec<Finding> {
    match ctx.proton.as_ref().and_then(|p| p.major().map(|m| (p, m))) {
        Some((p, m)) if m < 9 => vec![
            Finding::new("proton", "proton-too-old", Severity::Error)
                .arg("proton", p)
                .advised(),
        ],
        _ => vec![],
    }
}

fn openxr(ctx: &DiagContext) -> Vec<Finding> {
    if let Some(over) = &ctx.config.openxr_env {
        let mut f = Finding::new("openxr", "openxr-override", Severity::Info)
            .advised()
            .fix(FixId::ClearOpenxrOverride);
        for (k, v) in over {
            f = f.evidence(format!("{k}={v}"));
        }
        return vec![f];
    }
    let Some(vr) = &ctx.vr else { return vec![] };
    let mut out = Vec::new();
    let name = vr
        .active
        .as_ref()
        .map(|r| r.name.clone().unwrap_or_else(|| r.kind.to_string()));
    if vr.ready {
        let mut f = Finding::new("openxr", "vr-ready", Severity::Info)
            .arg("runtime", name.clone().unwrap_or_default());
        if let Some(rt) = &vr.active {
            f = f
                .evidence(format!("manifest {}", rt.manifest.display()))
                .evidence(format!("library {}", rt.library.display()));
        }
        if let Some(o) = &vr.openvr {
            f = f.evidence(format!(
                "OpenVR layer {:?} at {}",
                o.layer,
                o.path.display()
            ));
        }
        out.push(f);
    }
    for b in &vr.blockers {
        let mut f = Finding::new("openxr", "vr-not-ready", Severity::Warn).advice_text(b.clone());
        if let Some(n) = &name {
            f = f.evidence(format!("active runtime: {n}"));
        }
        out.push(f);
    }
    for n in &vr.notes {
        out.push(Finding::new("openxr", "vr-note", Severity::Info).advice_text(n.clone()));
    }
    out
}

fn session(ctx: &DiagContext) -> Vec<Finding> {
    if ctx.processes.is_empty() {
        return vec![];
    }
    let mut f = Finding::new("session", "game-running", Severity::Info)
        .arg("count", ctx.processes.len())
        .fix(FixId::StopSession);
    for p in ctx.processes.iter().take(8) {
        f = f.evidence(format!("{} {}", p.pid, p.cmdline));
    }
    vec![f]
}

fn desktop_entry(ctx: &DiagContext) -> Vec<Finding> {
    if ctx.state.is_installed() && !ctx.exists(&ctx.paths.desktop_entry()) {
        return vec![
            Finding::new("desktop_entry", "desktop-entry-missing", Severity::Warn)
                .evidence(format!("missing {}", ctx.paths.desktop_entry().display()))
                .fix(FixId::Repair),
        ];
    }
    vec![]
}

fn sandbox(ctx: &DiagContext) -> Vec<Finding> {
    let mut out = Vec::new();
    if ctx.system.in_flatpak {
        out.push(Finding::new("sandbox", "flatpak", Severity::Warn).advised());
    }
    if let Some(d) = &ctx.system.distro
        && d.immutable
    {
        out.push(
            Finding::new("sandbox", "immutable-distro", Severity::Info)
                .arg("distro", &d.name)
                .advised(),
        );
    }
    out
}

/// Known failure signatures in Proton and Somnium output.
const LOG_PATTERNS: &[(&str, &str, Severity)] = &[
    ("SEC_E_", "log-tls", Severity::Error),
    (
        "certificate verify failed",
        "log-certificate",
        Severity::Error,
    ),
    ("err:vulkan", "log-vulkan", Severity::Error),
    ("XR_ERROR_RUNTIME_UNAVAILABLE", "log-openxr", Severity::Warn),
    ("pressure-vessel-wrap: E:", "log-container", Severity::Error),
];

fn vr_perf(ctx: &DiagContext) -> Vec<Finding> {
    let mut out = Vec::new();
    let t = &ctx.system.vr_tuning;
    if let Some(c) = &t.compositor
        && c.realtime == Some(false)
    {
        let path = c.path.display();
        out.push(
            Finding::new("vr_perf", "compositor-not-realtime", Severity::Warn)
                .arg("compositor", &c.name)
                .arg("path", &path)
                .evidence(format!("getcap {path}: no cap_sys_nice"))
                .advised(),
        );
    }
    for p in &t.amd_power {
        if let Some(vr) = p.vr_index
            && p.active != "VR"
        {
            let file = format!("/sys/class/drm/{}/device/pp_power_profile_mode", p.card);
            out.push(
                Finding::new("vr_perf", "amd-power-profile", Severity::Warn)
                    .arg("profile", vr)
                    .arg("file", &file)
                    .evidence(format!("{file}: {} active", p.active))
                    .advised(),
            );
        }
    }
    if !t.overlays.is_empty() {
        out.push(
            Finding::new("vr_perf", "xr-overlay", Severity::Info)
                .arg("overlays", t.overlays.join(", "))
                .advised(),
        );
    }
    if let Some(h) = ctx.last_perf.and_then(|p| p.hitches) {
        let ev = format!(
            "last session: {} freezes, the longest {:.0} ms",
            h.count, h.longest_ms
        );
        out.push(match h.period_s {
            Some(p) => Finding::new("vr_perf", "freezes-periodic", Severity::Warn)
                .arg("period", format!("{p:.1}"))
                .evidence(ev)
                .advised(),
            None => Finding::new("vr_perf", "freezes", Severity::Info)
                .evidence(ev)
                .advised(),
        });
    }
    if let Some(d) = ctx.last_perf.and_then(|p| p.dips) {
        let pair = |v: Option<[f32; 2]>, unit: &str| {
            v.map(|[n, d]| format!("{n:.0}{unit} normally, {d:.0}{unit} during them"))
        };
        let (id, sev) = match d.cause {
            DipCause::GpuWaiting => ("dips-gpu-waiting", Severity::Warn),
            DipCause::GpuClock => ("dips-gpu-clock", Severity::Warn),
            DipCause::GpuBound => ("dips-gpu-bound", Severity::Info),
            DipCause::Unknown => ("dips", Severity::Info),
        };
        let mut f = Finding::new("vr_perf", id, sev)
            .evidence(format!(
                "last session: {} slowdowns, {:.0} s in total, {:.0} fps during them",
                d.episodes, d.seconds, d.fps
            ))
            .advised();
        for e in [
            pair(d.gpu_load, " % GPU"),
            pair(d.gpu_clock_mhz, " MHz"),
            pair(d.cpu_load, " % CPU (all cores)"),
        ]
        .into_iter()
        .flatten()
        {
            f = f.evidence(e);
        }
        out.push(f);
    }
    out
}

/// Swap use above this during a session is worth a look.
const SWAP_WARN_GB: f32 = 0.5;

fn memory(ctx: &DiagContext) -> Vec<Finding> {
    let Some(peak) = ctx.last_perf.and_then(|p| p.swap_peak_gb) else {
        return vec![];
    };
    if peak < SWAP_WARN_GB {
        return vec![];
    }
    let m = &ctx.system.vr_tuning.memory;
    let (id, sev) = if m.zram {
        ("swap-zram", Severity::Info)
    } else {
        ("swap-disk", Severity::Warn)
    };
    let mut f = Finding::new("memory", id, sev)
        .arg("gb", format!("{peak:.1}"))
        .evidence(format!("MangoHud swap_used peaked at {peak:.2} GB"));
    if let Some(s) = m.swappiness {
        f = f.evidence(format!("vm.swappiness = {s}"));
    }
    f = f.evidence(if m.swaps.is_empty() {
        "no swap area listed in /proc/swaps".to_string()
    } else {
        format!("swap areas: {}", m.swaps.join(", "))
    });
    vec![f.advised()]
}

fn video(ctx: &DiagContext) -> Vec<Finding> {
    let Some(v) = ctx.last_video else {
        return vec![];
    };
    if v.ytdlp_blocked > 0 {
        return vec![
            Finding::new("video", "ytdlp-blocked", Severity::Error)
                .evidence(format!("last session: {} YouTube extractions refused by PyInstaller (\"Security validation failure: unexpected name of application's home directory\")", v.ytdlp_blocked))
                .advised(),
        ];
    }
    let mut out = Vec::new();
    if v.domain_blocked > 0 {
        out.push(
            Finding::new("video", "video-domain-blocked", Severity::Info)
                .evidence(format!(
                    "last session: {} links refused with \"Domain not allowed\" in Player.log",
                    v.domain_blocked
                ))
                .advised(),
        );
    }
    if v.extract_failed > 0 {
        out.push(
            Finding::new("video", "video-extract-failed", Severity::Info)
                .evidence(format!(
                    "last session: {} failed yt-dlp extractions, {} of {} video opens failed",
                    v.extract_failed,
                    v.failed.min(v.opened),
                    v.opened
                ))
                .advised(),
        );
    }
    out
}

fn launch_log(ctx: &DiagContext) -> Vec<Finding> {
    let mut out = Vec::new();
    for (needle, id, sev) in LOG_PATTERNS {
        let hits: Vec<&String> = ctx
            .launch_log
            .iter()
            .filter(|l| l.contains(needle))
            .collect();
        if let Some(first) = hits.first() {
            let mut f = Finding::new("launch_log", id, *sev)
                .advised()
                .evidence(format!(
                    "{} matching lines, first: {}",
                    hits.len(),
                    first.trim()
                ));
            if needle.starts_with("SEC_E") || needle.starts_with("certificate") {
                f = f.fix(FixId::Repair);
            }
            out.push(f);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;
    use unilux_system::{FakeHost, PythonVersion};

    /// The literal ids passed to `Finding::new` in this file, so a new
    /// finding cannot be added without listing it in FINDING_IDS.
    fn ids_in_source() -> Vec<String> {
        let src = include_str!("checks.rs");
        let code = &src[..src.find("#[cfg(test)]").unwrap_or(src.len())];
        let literal = |s: &str| {
            let s = s.trim_start().strip_prefix('"')?;
            let end = s.find('"')?;
            Some((s[..end].to_string(), s[end + 1..].to_string()))
        };
        code.match_indices("Finding::new(")
            .filter_map(|(at, call)| {
                let (_check, rest) = literal(&code[at + call.len()..])?;
                let rest = rest.trim_start().strip_prefix(',')?;
                literal(rest).map(|(id, _)| id)
            })
            .collect()
    }

    /// Calls that take their id from a table or a condition are not seen
    /// by `ids_in_source`; their ids still have to appear as literals.
    #[test]
    fn finding_ids_lists_every_finding() {
        let used = ids_in_source();
        assert!(used.len() > 25, "found only {} literal ids", used.len());
        let listed: HashSet<_> = FINDING_IDS.iter().copied().collect();
        let missing: Vec<_> = used
            .iter()
            .filter(|id| !listed.contains(id.as_str()))
            .collect();
        assert!(missing.is_empty(), "add these to FINDING_IDS: {missing:?}");

        let src = include_str!("checks.rs");
        let list_end = src.find("pub struct Finding").unwrap();
        let code = &src[list_end..src.find("#[cfg(test)]").unwrap()];
        let stale: Vec<_> = FINDING_IDS
            .iter()
            .filter(|id| !code.contains(&format!("\"{id}\"")))
            .collect();
        assert!(
            stale.is_empty(),
            "FINDING_IDS lists ids no check uses: {stale:?}"
        );
    }

    fn ctx(files: &[&str], f: impl FnOnce(&mut DiagContext)) -> DiagContext {
        let paths = Paths::resolve(|k| (k == "HOME").then(|| "/h".to_string())).unwrap();
        let mut system = unilux_system::probe(&FakeHost::new());
        system.python = Some(PythonVersion {
            major: 3,
            minor: 12,
        });
        system.vulkan_icds = vec!["radeon.json".into()];
        system.gpus = vec![unilux_system::GpuInfo {
            card: "card0".into(),
            vendor: "AMD".into(),
            driver: None,
        }];
        let set: HashSet<PathBuf> = files.iter().map(PathBuf::from).collect();
        let mut c = DiagContext {
            prefix: paths.default_prefix(),
            proton_dir: Some(paths.proton_root().join("ge/GE-Proton11-5")),
            proton: Some(ProtonId::new("ge", "GE-Proton11-5")),
            patches: Some(PatchStatus {
                bundle: Some("b1".into()),
                intact: true,
                changed: vec![],
            }),
            patch_match: Some(TargetStatus::Verified),
            vr: None,
            paths,
            config: Config::default(),
            state: InstallState::default(),
            system,
            processes: vec![],
            github_reachable: Some(true),
            launch_log: vec![],
            last_perf: None,
            last_video: None,
            exists: Box::new(move |p: &Path| set.contains(p)),
        };
        f(&mut c);
        c
    }

    const LAUNCHER: &str = "/h/.local/share/unilux/prefix/drive_c/Program Files/Somnium Space/Somnium Space Launcher.exe";
    const UMU: &str = "/h/.local/share/unilux/components/umu/umu-run";
    const PROTON: &str = "/h/.local/share/unilux/components/proton/ge/GE-Proton11-5/proton";

    fn vr(ready: bool, blockers: &[&str]) -> VrStatus {
        VrStatus {
            active: None,
            installed: vec![],
            service_running: ready,
            openvr: None,
            ready,
            blockers: blockers.iter().map(|b| b.to_string()).collect(),
            notes: vec![],
        }
    }

    fn ids(v: &[Finding]) -> Vec<(&str, Severity)> {
        v.iter().map(|f| (f.check, f.severity)).collect()
    }

    #[test]
    fn healthy_install_has_only_info() {
        let c = ctx(&[LAUNCHER, UMU, PROTON], |c| c.vr = Some(vr(true, &[])));
        let v = run_all(&c);
        assert!(v.iter().all(|f| f.severity == Severity::Info), "{v:#?}");
    }

    #[test]
    fn not_installed() {
        let v = install(&ctx(&[], |_| {}));
        assert_eq!(ids(&v), [("install", Severity::Error)]);
        assert!(v[0].fix.is_none());
    }

    #[test]
    fn missing_components_offer_repair() {
        let v = install(&ctx(&[LAUNCHER], |_| {}));
        assert_eq!(v.len(), 2);
        assert!(v.iter().all(|f| f.fix == Some(FixId::Repair)));
    }

    #[test]
    fn patch_states() {
        let none = patches(&ctx(&[PROTON], |c| c.patches = None));
        assert_eq!(none[0].fix, Some(FixId::Repair));
        let changed = patches(&ctx(&[PROTON], |c| {
            c.patches = Some(PatchStatus {
                bundle: Some("b1".into()),
                intact: false,
                changed: vec!["files/x.dll".into()],
            })
        }));
        assert!(changed[0].title_in(ENGLISH).contains("replaced"));
        let ok = patches(&ctx(&[PROTON], |_| {}));
        assert_eq!(ids(&ok), [("patches", Severity::Info)]);
        let nomatch = patches(&ctx(&[PROTON], |c| c.patch_match = None));
        assert!(nomatch.iter().any(|f| {
            f.advice_in(ENGLISH)
                .is_some_and(|a| a.contains("unilux patch build"))
        }));
        assert!(
            patches(&ctx(&[], |_| {})).is_empty(),
            "no Proton, nothing to say"
        );
    }

    #[test]
    fn old_proton() {
        assert_eq!(
            proton(&ctx(&[], |c| c.proton =
                Some(ProtonId::new("ge", "GE-Proton8-32"))))[0]
                .severity,
            Severity::Error
        );
        assert!(proton(&ctx(&[], |_| {})).is_empty());
    }

    #[test]
    fn python_cases() {
        assert!(python(&ctx(&[], |_| {})).is_empty());
        assert_eq!(python(&ctx(&[], |c| c.system.python = None)).len(), 1);
        let old = python(&ctx(&[], |c| {
            c.system.python = Some(PythonVersion { major: 3, minor: 9 })
        }));
        assert!(old[0].title_in(ENGLISH).contains("3.9"));
    }

    #[test]
    fn vr_blockers_become_findings() {
        let v = openxr(&ctx(&[], |c| {
            c.vr = Some(vr(false, &["Start WiVRn", "Install xrizer"]))
        }));
        assert_eq!(v.len(), 2);
        assert!(v.iter().all(|f| f.severity == Severity::Warn));
        assert_eq!(
            openxr(&ctx(&[], |c| c.vr = Some(vr(true, &[]))))[0].severity,
            Severity::Info
        );
    }

    #[test]
    fn override_and_network_and_session() {
        let v = openxr(&ctx(&[], |c| {
            c.config.openxr_env = Some([("A".into(), "1".into())].into())
        }));
        assert_eq!(v[0].fix, Some(FixId::ClearOpenxrOverride));
        assert_eq!(
            network(&ctx(&[], |c| c.github_reachable = Some(false))).len(),
            1
        );
        assert!(network(&ctx(&[], |c| c.github_reachable = None)).is_empty());
        let v = session(&ctx(&[], |c| {
            c.processes = vec![ProcInfo {
                pid: 7,
                ppid: None,
                name: "wine".into(),
                cmdline: "x".into(),
                wineprefix: None,
            }]
        }));
        assert_eq!(v[0].fix, Some(FixId::StopSession));
    }

    #[test]
    fn gpu_and_desktop_and_sandbox() {
        assert_eq!(gpu(&ctx(&[], |c| c.system.vulkan_icds.clear())).len(), 1);
        assert_eq!(
            desktop_entry(&ctx(&[], |c| c.state.installed_at = Some(1))).len(),
            1
        );
        assert!(desktop_entry(&ctx(&[], |_| {})).is_empty());
        assert_eq!(sandbox(&ctx(&[], |c| c.system.in_flatpak = true)).len(), 1);
    }

    #[test]
    fn log_signatures() {
        let v = launch_log(&ctx(&[], |c| {
            c.launch_log = vec!["ok".into(), "0024:err:secur32: SEC_E_INTERNAL_ERROR".into()]
        }));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].fix, Some(FixId::Repair));
        assert!(launch_log(&ctx(&[], |c| c.launch_log = vec!["fine".into()])).is_empty());
    }

    #[test]
    fn swap_and_video() {
        let perf = |gb| Perf {
            swap_peak_gb: gb,
            ..Default::default()
        };
        assert!(memory(&ctx(&[], |c| c.last_perf = Some(perf(Some(0.1))))).is_empty());
        let v = memory(&ctx(&[], |c| c.last_perf = Some(perf(Some(2.5)))));
        assert_eq!(v[0].severity, Severity::Warn);
        let v = memory(&ctx(&[], |c| {
            c.last_perf = Some(perf(Some(2.5)));
            c.system.vr_tuning.memory = unilux_system::memory(
                Some("60\n"),
                Some("Filename Type\n/dev/zram0 partition 8G 0 100\n"),
            );
        }));
        assert_eq!(v[0].severity, Severity::Info);
        assert!(v[0].evidence.iter().any(|e| e.contains("/dev/zram0")));
        assert!(
            v[0].advice_in(ENGLISH)
                .unwrap()
                .contains("between 100 and 180")
        );

        let blocked = Video {
            opened: 9,
            failed: 7,
            extract_failed: 5,
            ytdlp_blocked: 5,
            domain_blocked: 0,
        };
        let v = video(&ctx(&[], |c| c.last_video = Some(blocked)));
        assert_eq!(v[0].severity, Severity::Error);
        let v = video(&ctx(&[], |c| {
            c.last_video = Some(Video {
                ytdlp_blocked: 0,
                ..blocked
            })
        }));
        assert_eq!(v[0].severity, Severity::Info);
        assert!(video(&ctx(&[], |c| c.last_video = Some(Video::default()))).is_empty());
        let v = video(&ctx(&[], |c| {
            c.last_video = Some(Video {
                domain_blocked: 1,
                ..Video::default()
            })
        }));
        assert_eq!(v[0].title_in(ENGLISH), "Somnium refused some video links");
    }

    fn dips(cause: DipCause) -> unilux_patches::community::Dips {
        unilux_patches::community::Dips {
            episodes: 3,
            seconds: 4.5,
            fps: 38.0,
            gpu_load: Some([60.0, 95.0]),
            gpu_clock_mhz: Some([2400.0, 1200.0]),
            cpu_load: None,
            cause,
        }
    }

    fn hitches(period_s: Option<f32>) -> unilux_patches::community::Hitches {
        unilux_patches::community::Hitches {
            count: 4,
            longest_ms: 310.0,
            period_s,
        }
    }

    /// Contexts that between them raise every finding once.
    fn catalogue() -> Vec<Finding> {
        let video = Video {
            opened: 9,
            failed: 7,
            extract_failed: 5,
            ytdlp_blocked: 5,
            domain_blocked: 2,
        };
        let perf = |dips: DipCause, period: Option<f32>, swap: f32| Perf {
            dips: Some(self::dips(dips)),
            hitches: Some(hitches(period)),
            swap_peak_gb: Some(swap),
            ..Default::default()
        };
        let zram = unilux_system::memory(
            Some("60\n"),
            Some("Filename Type\n/dev/zram0 partition 8G 0 100\n"),
        );
        let scenarios = vec![
            ctx(&[], |c| {
                c.system.python = None;
                c.system.vulkan_icds.clear();
                c.system.gpus.clear();
                c.github_reachable = Some(false);
                c.system.in_flatpak = true;
            }),
            ctx(&[], |c| {
                c.system.python = None;
                c.system.distro = Some(unilux_system::Distro::parse("ID=debian\nNAME=Debian"));
            }),
            ctx(&[], |c| {
                c.system.python = Some(PythonVersion { major: 3, minor: 9 });
                c.system.distro = Some(unilux_system::Distro::parse("ID=bazzite\nNAME=Bazzite"));
            }),
            ctx(&[LAUNCHER], |_| {}),
            ctx(&[LAUNCHER, UMU], |c| c.proton_dir = None),
            ctx(&[PROTON], |c| c.patches = None),
            ctx(&[PROTON], |c| {
                c.patches = Some(PatchStatus {
                    bundle: Some("b1".into()),
                    intact: false,
                    changed: vec!["files/x.dll".into()],
                });
                c.patch_match = None;
            }),
            ctx(&[PROTON], |c| c.patch_match = Some(TargetStatus::Untested)),
            ctx(&[], |c| {
                c.proton = Some(ProtonId::new("ge", "GE-Proton8-32"))
            }),
            ctx(&[], |c| {
                c.config.openxr_env = Some([("A".into(), "1".into())].into())
            }),
            ctx(&[], |c| {
                let mut v = vr(false, &["Start WiVRn and connect the headset."]);
                v.notes = vec!["WiVRn or Monado are recommended over SteamVR.".into()];
                c.vr = Some(v);
            }),
            ctx(&[], |c| c.vr = Some(vr(true, &[]))),
            ctx(&[], |c| {
                c.processes = vec![ProcInfo {
                    pid: 7,
                    ppid: None,
                    name: "wine".into(),
                    cmdline: "x".into(),
                    wineprefix: None,
                }];
                c.state.installed_at = Some(1);
                c.launch_log = LOG_PATTERNS
                    .iter()
                    .map(|(needle, ..)| format!("0024:{needle} here"))
                    .collect();
            }),
            ctx(&[], |c| {
                c.system.vr_tuning.compositor = Some(unilux_system::Compositor {
                    name: "wivrn-server".into(),
                    path: "/usr/bin/wivrn-server".into(),
                    realtime: Some(false),
                });
                c.system.vr_tuning.amd_power = vec![unilux_system::PowerProfile {
                    card: "card0".into(),
                    active: "3D_FULL_SCREEN".into(),
                    vr_index: Some(4),
                }];
                c.system.vr_tuning.overlays = vec!["wayvr".into(), "xrdesktop".into()];
                c.last_perf = Some(perf(DipCause::GpuWaiting, Some(30.0), 2.5));
                c.last_video = Some(video);
            }),
            ctx(&[], |c| {
                c.last_perf = Some(perf(DipCause::GpuClock, None, 0.1));
                c.last_video = Some(Video {
                    ytdlp_blocked: 0,
                    ..video
                });
            }),
            ctx(&[], |c| {
                c.last_perf = Some(perf(DipCause::GpuBound, Some(12.25), 0.1))
            }),
            ctx(&[], |c| {
                c.last_perf = Some(perf(DipCause::Unknown, None, 1.25));
                c.system.vr_tuning.memory = zram.clone();
            }),
        ];
        let mut out: Vec<Finding> = Vec::new();
        for c in &scenarios {
            for f in CHECKS.iter().flat_map(|k| (k.run)(c)) {
                if !out.contains(&f) {
                    out.push(f);
                }
            }
        }
        out
    }

    /// What machine readers (MCP, `--json`, bug reports) get for every
    /// finding. Rerun with UNILUX_BLESS=1 only when a change is intended.
    #[test]
    fn machine_readable_findings_are_stable() {
        let v = catalogue();
        let seen: HashSet<&str> = v.iter().map(|f| f.id).collect();
        let missing: Vec<_> = FINDING_IDS
            .iter()
            .filter(|id| !seen.contains(*id))
            .collect();
        assert!(
            missing.is_empty(),
            "no catalogue context raises {missing:?}"
        );
        let text = serde_json::to_string_pretty(&v).unwrap() + "\n";
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/findings.en.json");
        if std::env::var_os("UNILUX_BLESS").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &text).unwrap();
        }
        let golden = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            golden == text,
            "findings JSON changed, see {}",
            path.display()
        );
    }

    /// Without engine text to fall back on, every finding needs its English
    /// in en.ftl (and so in the template every language follows).
    #[test]
    fn every_finding_has_a_title_and_advice() {
        let mut missing = Vec::new();
        for id in FINDING_IDS {
            let key = format!("finding-{id}-title");
            if !unilux_i18n::has_key(ENGLISH, &key) {
                missing.push(key);
            }
        }
        for f in catalogue() {
            if f.advice == Advice::Message && !unilux_i18n::has_key(ENGLISH, &f.key("advice")) {
                missing.push(f.key("advice"));
            }
        }
        assert!(missing.is_empty(), "en.ftl is missing {missing:?}");
    }

    #[test]
    fn every_fix_has_a_description() {
        for f in FixId::ALL {
            assert!(
                unilux_i18n::has_key(ENGLISH, f.message()),
                "en.ftl is missing {}",
                f.message()
            );
            assert_ne!(f.describe_in("fr"), f.describe(), "{f:?} is not translated");
        }
    }

    /// The text a person reads follows the interface language; what machines
    /// read stays English.
    #[test]
    fn people_get_their_language_machines_get_english() {
        let f = Finding::new("network", "github-unreachable", Severity::Error).advised();
        assert_eq!(f.title_in(ENGLISH), "GitHub is not reachable");
        assert_ne!(f.title_in("fr"), f.title_in(ENGLISH));
        assert_ne!(f.advice_in("fr"), f.advice_in(ENGLISH));
        let json = serde_json::to_value(&f).unwrap();
        assert_eq!(json["title"], "GitHub is not reachable");
        let quiet = Finding::new("install", "umu-missing", Severity::Error);
        assert!(
            quiet.advice_in(ENGLISH).is_none(),
            "machines get only stated advice"
        );
        assert!(
            quiet.advice().is_some(),
            "people still get the translated hint"
        );
    }

    #[test]
    fn fix_ids_round_trip() {
        for f in FixId::ALL {
            let s = serde_json::to_value(f).unwrap();
            assert_eq!(FixId::parse(s.as_str().unwrap()), Some(*f));
        }
        assert_eq!(FixId::parse("rm_rf"), None);
    }
}
