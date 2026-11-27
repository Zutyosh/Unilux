use std::process::ExitCode;

use serde::Serialize;
use unilux_core::Severity;
use unilux_diag::Finding;
use unilux_i18n::{t, t_args};
use unilux_service::Status;

pub const SCHEMA: u32 = 1;

pub struct Out {
    json: bool,
}

pub fn envelope_ok<T: Serialize>(data: &T) -> serde_json::Value {
    serde_json::json!({ "schema": SCHEMA, "ok": true, "data": data })
}

pub fn envelope_err(e: &unilux_core::Error) -> serde_json::Value {
    let kind = match e {
        unilux_core::Error::Io { .. } => "io",
        unilux_core::Error::Parse { .. } => "parse",
        unilux_core::Error::Network(_) => "network",
        unilux_core::Error::Checksum { .. } => "checksum",
        unilux_core::Error::Invalid(_) => "invalid",
        unilux_core::Error::Busy(_) => "busy",
        unilux_core::Error::Cancelled => "cancelled",
    };
    serde_json::json!({ "schema": SCHEMA, "ok": false, "error": { "kind": kind, "message": e.to_string() } })
}

impl Out {
    pub fn new(json: bool) -> Self {
        Self { json }
    }

    pub fn ok<T: Serialize>(&self, data: &T, text: impl FnOnce(&T) -> String) -> ExitCode {
        if self.json {
            println!("{}", envelope_ok(data));
        } else {
            println!("{}", text(data));
        }
        ExitCode::SUCCESS
    }

    pub fn error(&self, e: &unilux_core::Error) -> ExitCode {
        if self.json {
            println!("{}", envelope_err(e));
        } else {
            eprintln!("error: {e}");
        }
        ExitCode::FAILURE
    }

    pub fn result<T: Serialize>(
        &self,
        r: unilux_core::Result<T>,
        text: impl FnOnce(&T) -> String,
    ) -> ExitCode {
        match r {
            Ok(v) => self.ok(&v, text),
            Err(e) => self.error(&e),
        }
    }
}

pub fn status(s: &Status) -> String {
    let patches = match &s.patches {
        Some(p) if p.bundle.is_some() => {
            let bundle = p.bundle.clone().unwrap_or_default();
            if p.intact {
                t_args("cli-status-patches", &[("bundle", bundle)])
            } else {
                t_args("cli-status-patches-changed", &[("bundle", bundle)])
            }
        }
        _ => t("cli-status-patches-none").to_string(),
    };
    let mut out = vec![
        format!("Unilux {}", s.unilux_version),
        if s.installed {
            t("cli-status-installed-yes")
        } else {
            t("cli-status-installed-no")
        }
        .to_string(),
        t_args(
            "cli-status-prefix",
            &[("path", s.prefix.display().to_string())],
        ),
        t_args("cli-status-recipe", &[("recipe", s.recipe.to_string())]),
        match &s.proton {
            Some(p) => t_args("cli-status-proton", &[("proton", p.clone())]),
            None => t("cli-status-proton-none").into(),
        },
        patches,
        if s.vr.ready {
            t("cli-status-vr-ready")
        } else {
            t("cli-status-vr-not-ready")
        }
        .to_string(),
        match &s.runtime {
            Some(r) => t_args(
                "cli-status-runtime",
                &[(
                    "runtime",
                    r.name.clone().unwrap_or_else(|| r.kind.to_string()),
                )],
            ),
            None => t("cli-status-runtime-none").into(),
        },
    ];
    if let Some(pid) = s.session_pid {
        out.push(t_args("cli-status-running", &[("pid", pid.to_string())]));
    }
    out.join("\n")
}

pub fn vr(v: &unilux_system::VrStatus) -> String {
    let mut out = vec![match &v.active {
        Some(r) => t_args(
            "cli-vr-runtime",
            &[
                (
                    "runtime",
                    r.name.clone().unwrap_or_else(|| r.kind.to_string()),
                ),
                ("path", r.manifest.display().to_string()),
            ],
        ),
        None => t("cli-vr-runtime-none").into(),
    }];
    if !v.installed.is_empty() {
        let runtimes = v
            .installed
            .iter()
            .map(|k| k.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        out.push(t_args("cli-vr-installed", &[("runtimes", runtimes)]));
    }
    if let Some(o) = &v.openvr {
        out.push(t_args(
            "cli-vr-openvr",
            &[
                ("layer", format!("{:?}", o.layer)),
                ("path", o.path.display().to_string()),
            ],
        ));
    }
    out.push(
        if v.ready {
            t("cli-vr-ready")
        } else {
            t("cli-vr-not-ready")
        }
        .into(),
    );
    out.extend(v.blockers.iter().map(|b| format!("  - {b}")));
    out.extend(
        v.notes
            .iter()
            .map(|n| format!("  {}", t_args("cli-vr-note", &[("note", n.clone())]))),
    );
    out.join("\n")
}

/// Severity tags padded to one width, so the titles line up in any language.
fn severity_tag(s: Severity) -> String {
    let tags = [
        t("cli-severity-error"),
        t("cli-severity-warn"),
        t("cli-severity-info"),
    ];
    let width = tags.iter().map(|t| t.chars().count()).max().unwrap_or(0);
    let tag = match s {
        Severity::Error => tags[0],
        Severity::Warn => tags[1],
        Severity::Info => tags[2],
    };
    format!("{tag:<width$}")
}

pub fn findings(v: &[Finding]) -> String {
    if v.is_empty() {
        return t("cli-doctor-none").into();
    }
    v.iter()
        .map(|f| {
            let mut s = format!("[{}] {}", severity_tag(f.severity), f.title());
            for e in &f.evidence {
                s.push_str(&format!("\n        {e}"));
            }
            if let Some(a) = f.advice() {
                s.push_str(&format!("\n        -> {a}"));
            }
            if let Some(fix) = f.fix {
                let id = serde_json::to_value(fix)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                s.push_str(&format!(
                    "\n        {}",
                    t_args("cli-doctor-fix", &[("id", id)])
                ));
            }
            s
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelopes_are_stable() {
        assert_eq!(
            envelope_ok(&42).to_string(),
            r#"{"data":42,"ok":true,"schema":1}"#
        );
        let e = envelope_err(&unilux_core::Error::Busy("install".into()));
        assert_eq!(e["error"]["kind"], "busy");
        assert_eq!(e["ok"], false);
    }
}
