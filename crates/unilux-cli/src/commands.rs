//! `unilux proton ...`, `unilux patch ...` and `unilux recipe ...`.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Subcommand;
use unilux_service::Service;

use unilux_i18n::{t, t_args};

use crate::output::Out;

#[derive(Subcommand)]
pub enum ProtonCmd {
    /// Installed Proton builds.
    List,
    /// Releases available upstream for a provider (ge, cachyos).
    Available {
        #[arg(default_value = "ge")]
        provider: String,
        #[arg(long)]
        refresh: bool,
    },
    /// Download and install a build, e.g. cachyos:cachyos-11.0-20261015-slr.
    Install { id: String },
    /// Make Somnium use this build (downloads and patches it).
    Use {
        id: String,
        /// Use it even without a patch bundle.
        #[arg(long)]
        force: bool,
    },
    /// Delete an installed build.
    Remove { id: String },
}

#[derive(Subcommand)]
pub enum PrefixCmd {
    /// Where the Wine prefix lives.
    Show,
    /// Move the prefix into a folder, e.g. on a game drive. A non-empty
    /// folder gets a new unilux-prefix folder inside it. Stops anything
    /// still running in the prefix.
    Move { dir: PathBuf },
}

#[derive(Subcommand)]
pub enum PatchCmd {
    /// Patch bundle installed in the current Proton, and whether it is intact.
    Status,
    /// Patch files of the ingredient library.
    List,
    /// Bundles in the catalog for a Proton build.
    Catalog { id: Option<String> },
    /// What building the recipe in use would do (Wine commit, trees, container command).
    Plan { id: String },
    /// Rebuild the recipe in use for a Proton build in a container (slow).
    Build {
        id: String,
        /// Install the result into the current Proton when done.
        #[arg(long)]
        apply: bool,
    },
    /// Install a bundle file into the current Proton.
    Apply { bundle: PathBuf },
    /// Put Proton's stock files back.
    Restore,
}

#[derive(Subcommand)]
pub enum RecipeCmd {
    /// Built-in and your own recipes; * marks the one in use.
    List,
    /// Print a recipe as TOML.
    Show { id: String },
    /// The ingredients recipes pick from.
    Ingredients,
    /// Print a self-contained copy (every patch inlined) to share.
    Export { id: String },
    /// Check a recipe file without saving it.
    Check {
        file: PathBuf,
        /// Print it in canonical form (library ingredients by reference).
        #[arg(long)]
        print: bool,
    },
    /// Save a recipe file as one of yours.
    Import { file: PathBuf },
    /// Delete one of your recipes.
    Remove { id: String },
    /// Build a recipe's bundle (default: the one in use, on its Proton).
    Build {
        id: Option<String>,
        /// Proton to build for (provider:tag); default: the recipe's.
        #[arg(long)]
        proton: Option<String>,
        /// Rebuild even when a bundle for it is cached.
        #[arg(long)]
        force: bool,
    },
    /// Make Somnium use a recipe (builds it when nothing fits).
    Use { id: String },
    /// How a recipe stands on your own session reports (same rules as CI).
    Verdict { id: String },
    /// The key a recipe file has on a Proton: session reports carry it.
    Key { file: PathBuf, proton: String },
    /// Submit a recipe with its session reports to the community (GitHub form).
    Share {
        id: String,
        /// Only these reports (ids from `unilux session list`); default: all of this recipe.
        #[arg(long = "report")]
        reports: Vec<String>,
        /// Open the form in the browser.
        #[arg(long)]
        open: bool,
    },
}

#[derive(Subcommand)]
pub enum SessionCmd {
    /// Saved session reports, newest first.
    List,
    /// One report as JSON.
    Show { id: String },
    /// Make the report of the last session (after `unilux launch --detach`).
    Record,
}

#[derive(Subcommand)]
pub enum CommunityCmd {
    /// Community recipes and their standing.
    List,
    /// Save a community recipe as one of yours.
    Install { id: String },
    /// Judge one community recipe directory (recipe.toml + reports/*.json). For CI.
    Verify { dir: PathBuf },
    /// Rebuild the index of a community directory (recipes/*). For CI.
    Index {
        root: PathBuf,
        /// Write root/index.json instead of printing it.
        #[arg(long)]
        write: bool,
        /// Fail when a recipe directory does not check (CI).
        #[arg(long)]
        strict: bool,
    },
}

pub fn parse_id(s: &str) -> Result<unilux_components::ProtonId, unilux_core::Error> {
    unilux_components::ProtonId::parse(s)
}

fn joined<T>(v: &[T], f: impl Fn(&T) -> String) -> String {
    v.iter().map(f).collect::<Vec<_>>().join("\n")
}

pub async fn prefix(out: &Out, svc: &Service, cmd: PrefixCmd) -> ExitCode {
    match cmd {
        PrefixCmd::Show => out.ok(&svc.status().prefix, |p| p.display().to_string()),
        PrefixCmd::Move { dir } => {
            let dir = std::path::absolute(&dir).unwrap_or(dir);
            out.result(svc.move_prefix(dir).await, |p| {
                t_args("cli-prefix-moved", &[("path", p.display().to_string())])
            })
        }
    }
}

pub async fn proton(out: &Out, svc: &Service, cmd: ProtonCmd) -> ExitCode {
    match cmd {
        ProtonCmd::List => out.ok(&svc.proton_installed(), |v| {
            if v.is_empty() {
                return t("cli-proton-none").into();
            }
            joined(v, |p| {
                let patches = match (&p.patches.bundle, p.patches.intact) {
                    (Some(b), true) => b.clone(),
                    (Some(b), false) => t_args("cli-proton-changed", &[("bundle", b.clone())]),
                    (None, _) => t("cli-proton-no-patches").into(),
                };
                format!(
                    "{}{}  {}  {patches}",
                    if p.in_use { "* " } else { "  " },
                    p.proton.id,
                    p.proton.variant
                )
            })
        }),
        ProtonCmd::Available { provider, refresh } => {
            out.result(svc.proton_releases(&provider, refresh).await, |v| {
                joined(v, |r| {
                    let p = match r.patches {
                        Some(unilux_patches::TargetStatus::Verified) => {
                            t("cli-proton-patches-verified")
                        }
                        Some(unilux_patches::TargetStatus::Untested) => {
                            t("cli-proton-patches-untested")
                        }
                        _ => t("cli-proton-no-patches"),
                    };
                    let date = r
                        .published_at
                        .as_deref()
                        .and_then(|d| d.get(..10))
                        .unwrap_or("");
                    let installed = if r.installed {
                        format!("  {}", t("cli-proton-installed-tag"))
                    } else {
                        String::new()
                    };
                    format!(
                        "{}{}  {date}  {p}{installed}",
                        if r.in_use { "* " } else { "  " },
                        r.id,
                    )
                })
            })
        }
        ProtonCmd::Install { id } => match parse_id(&id) {
            Ok(id) => out.result(svc.proton_install(id).await, |p| {
                t_args(
                    "cli-proton-installed-in",
                    &[("path", p.display().to_string())],
                )
            }),
            Err(e) => out.error(&e),
        },
        ProtonCmd::Use { id, force } => match parse_id(&id) {
            Ok(id) => out.result(svc.proton_use(id.clone(), force).await, |_| {
                t_args("cli-proton-used", &[("proton", id.to_string())])
            }),
            Err(e) => out.error(&e),
        },
        ProtonCmd::Remove { id } => match parse_id(&id) {
            Ok(id) => out.result(svc.proton_remove(&id), |_| {
                t_args("cli-removed", &[("id", id.to_string())])
            }),
            Err(e) => out.error(&e),
        },
    }
}

pub async fn patch(out: &Out, svc: &Service, cmd: PatchCmd) -> ExitCode {
    match cmd {
        PatchCmd::Status => out.ok(&svc.status().patches, |p| match p {
            None => t("cli-patch-no-proton").into(),
            Some(s) => match (&s.bundle, s.intact) {
                (None, _) => t("cli-patch-none").into(),
                (Some(b), true) => t_args("cli-patch-intact", &[("bundle", b.clone())]),
                (Some(b), false) => t_args(
                    "cli-patch-changed",
                    &[("bundle", b.clone()), ("files", s.changed.join(", "))],
                ),
            },
        }),
        PatchCmd::List => out.result(svc.patch_list(), |v| {
            joined(v, |p| {
                format!("{:22} {}  {}", p.ingredient, p.name, p.subject)
            })
        }),
        PatchCmd::Catalog { id } => {
            let cat = svc.catalog(true).await;
            match id
                .and_then(|s| parse_id(&s).ok())
                .or_else(|| svc.current_proton())
            {
                Some(id) => out.ok(&cat.resolve(&id), |v| {
                    if v.is_empty() {
                        t_args("cli-patch-no-bundle-for", &[("proton", id.to_string())])
                    } else {
                        joined(v, |m| format!("{} ({:?})", m.bundle.id, m.status))
                    }
                }),
                None => out.ok(&cat, |c| {
                    let bundles = c
                        .bundles
                        .iter()
                        .map(|b| b.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(
                        "{}\n{}",
                        t_args(
                            "cli-patch-recommended",
                            &[("proton", c.recommended_proton.to_string())]
                        ),
                        t_args("cli-patch-bundles", &[("bundles", bundles)])
                    )
                }),
            }
        }
        PatchCmd::Plan { id } => match parse_id(&id) {
            Ok(id) => out.result(svc.patch_plan(&id).await, |p| {
                serde_json::to_string_pretty(p).unwrap_or_default()
            }),
            Err(e) => out.error(&e),
        },
        PatchCmd::Build { id, apply } => match parse_id(&id) {
            Ok(id) => {
                let r = svc.patch_build(id).await;
                if let (Ok(p), true) = (&r, apply)
                    && let Err(e) = svc.patch_apply(p.clone()).await
                {
                    return out.error(&e);
                }
                out.result(r, |p| {
                    t_args("cli-bundle-path", &[("path", p.display().to_string())])
                })
            }
            Err(e) => out.error(&e),
        },
        PatchCmd::Apply { bundle } => out.result(svc.patch_apply(bundle).await, |_| {
            t("cli-bundle-installed").into()
        }),
        PatchCmd::Restore => out.result(svc.patch_restore(), |n| {
            t_args("cli-patch-restored", &[("count", n.to_string())])
        }),
    }
}

fn read_file(p: &std::path::Path) -> Result<String, unilux_core::Error> {
    std::fs::read_to_string(p)
        .map_err(|e| unilux_core::Error::io(format!("reading {}", p.display()), e))
}

fn recipe_line(r: &unilux_service::RecipeInfo) -> String {
    let mut tags = Vec::new();
    if r.builtin {
        tags.push(t("cli-recipe-builtin"));
    }
    if r.custom_code {
        tags.push(t("cli-recipe-custom"));
    }
    if !r.needs_build {
        tags.push(t("cli-recipe-no-build"));
    }
    format!(
        "{}{}  {}  {}  [{}]{}",
        if r.active { "* " } else { "  " },
        r.id,
        r.title,
        match &r.proton {
            Some(p) => t_args("cli-recipe-proton", &[("proton", p.clone())]),
            None => t("cli-recipe-proton-any").into(),
        },
        r.ingredients.join(", "),
        if tags.is_empty() {
            String::new()
        } else {
            format!("  ({})", tags.join(", "))
        }
    )
}

pub async fn recipe(out: &Out, svc: &Service, cmd: RecipeCmd) -> ExitCode {
    match cmd {
        RecipeCmd::List => out.result(svc.recipes(), |v| joined(v, recipe_line)),
        RecipeCmd::Show { id } => out.result(svc.recipe(&id).and_then(|r| r.to_toml()), |t| {
            t.trim_end().to_string()
        }),
        RecipeCmd::Ingredients => out.result(svc.library().map(|l| l.ingredients), |v| {
            joined(v, |i| format!("{:22} {:?}  {}", i.id, i.kind, i.title))
        }),
        RecipeCmd::Export { id } => {
            out.result(svc.recipe_export(&id), |t| t.trim_end().to_string())
        }
        RecipeCmd::Check { file, print: false } => out.result(
            read_file(&file).and_then(|t| svc.recipe_check(&t)),
            recipe_line,
        ),
        RecipeCmd::Check { file, print: true } => out.result(
            read_file(&file).and_then(|t| svc.library()?.import(&t)?.to_toml()),
            |t| t.trim_end().to_string(),
        ),
        RecipeCmd::Import { file } => {
            out.result(read_file(&file).and_then(|t| svc.recipe_import(&t)), |r| {
                let values = [("id", r.id.clone())];
                if r.custom_code {
                    t_args("cli-recipe-saved-custom", &values)
                } else {
                    t_args("cli-recipe-saved", &values)
                }
            })
        }
        RecipeCmd::Remove { id } => out.result(svc.recipe_delete(&id), |_| {
            t_args("cli-removed", &[("id", id.clone())])
        }),
        RecipeCmd::Build { id, proton, force } => {
            let proton = match proton.map(|p| parse_id(&p)).transpose() {
                Ok(p) => p,
                Err(e) => return out.error(&e),
            };
            out.result(svc.recipe_build(id.as_deref(), proton, force).await, |p| {
                t_args("cli-bundle-path", &[("path", p.display().to_string())])
            })
        }
        RecipeCmd::Use { id } => out.result(svc.recipe_use(&id).await, |_| {
            t_args("cli-recipe-used", &[("id", id.clone())])
        }),
        RecipeCmd::Key { file, proton } => {
            let r = parse_id(&proton).and_then(|p| {
                let lib = svc.library()?;
                let recipe = lib.import(&read_file(&file)?)?;
                Ok(lib.plan(&recipe)?.bundle_key(&p))
            });
            out.result(r, |k| k.clone())
        }
        cmd @ (RecipeCmd::Verdict { .. } | RecipeCmd::Share { .. }) => {
            recipe_evidence(out, svc, cmd).await
        }
    }
}

fn verdict_text(v: &unilux_patches::community::Verdict) -> String {
    let pct = |x: Option<f32>| {
        x.map(|s| format!("{:.0} %", s * 100.0))
            .unwrap_or_else(|| t("cli-na").into())
    };
    let mut s = t_args(
        "cli-verdict",
        &[
            ("status", format!("{:?}", v.status)),
            ("reports", v.reports.to_string()),
            ("configs", v.configs.to_string()),
            ("hours", format!("{:.1}", v.hours)),
            ("worlds", v.worlds.to_string()),
            ("crashes", v.crashes_in_game.to_string()),
            ("exits", v.crashes_on_exit.to_string()),
            ("videos", pct(v.video_success)),
            ("pacing", pct(v.pacing)),
        ],
    );
    if let Some(fps) = v.avg_fps {
        s.push_str(&t_args("cli-verdict-fps", &[("fps", format!("{fps:.0}"))]));
    }
    for p in &v.problems {
        s.push_str(&format!("\n  - {p}"));
    }
    s
}

async fn recipe_evidence(out: &Out, svc: &Service, cmd: RecipeCmd) -> ExitCode {
    match cmd {
        RecipeCmd::Verdict { id } => out.result(svc.recipe_verdict(&id), verdict_text),
        RecipeCmd::Share { id, reports, open } => {
            let r = svc.recipe_share(&id, &reports);
            if let (Ok(s), true) = (&r, open) {
                let _ = std::process::Command::new("xdg-open").arg(&s.url).spawn();
            }
            out.result(r, |s| {
                if s.prefilled {
                    format!("{}\n{}", t("cli-share-open"), s.url)
                } else {
                    format!(
                        "{}\n{}\n{}\n\n{}\n{}\n\n{}",
                        t("cli-share-too-long"),
                        s.url,
                        t("cli-share-paste-recipe"),
                        s.recipe,
                        t("cli-share-paste-reports"),
                        s.reports
                    )
                }
            })
        }
        _ => unreachable!("recipe() routes only Verdict and Share here"),
    }
}

pub async fn session(out: &Out, svc: &Service, cmd: SessionCmd) -> ExitCode {
    match cmd {
        SessionCmd::List => out.ok(&svc.sessions(), |v| {
            if v.is_empty() {
                return t("cli-session-none").into();
            }
            joined(v, |s| {
                let r = &s.report;
                let perf = r
                    .perf
                    .map(|p| {
                        t_args(
                            "cli-session-fps",
                            &[
                                ("fps", format!("{:.0}", p.avg_fps)),
                                ("low", format!("{:.0}", p.low1_fps)),
                            ],
                        )
                    })
                    .unwrap_or_else(|| t("cli-session-no-fps").into());
                let mut line = t_args(
                    "cli-session-line",
                    &[
                        ("id", s.id.clone()),
                        ("proton", r.recipe.proton.to_string()),
                        ("minutes", (r.duration_s / 60).to_string()),
                        ("worlds", r.worlds.to_string()),
                        (
                            "played",
                            (r.video.opened - r.video.failed.min(r.video.opened)).to_string(),
                        ),
                        ("opened", r.video.opened.to_string()),
                        ("perf", perf),
                    ],
                );
                let tags = [
                    match r.crash {
                        Some(unilux_patches::community::Crash::InGame) => {
                            Some(t("cli-session-crash"))
                        }
                        Some(unilux_patches::community::Crash::OnExit) => {
                            Some(t("cli-session-crash-exit"))
                        }
                        None => None,
                    },
                    r.flat.then(|| t("cli-session-desktop")),
                    (r.video.ytdlp_blocked > 0).then(|| t("cli-session-ytdlp")),
                ];
                for tag in tags.into_iter().flatten() {
                    line.push_str("  ");
                    line.push_str(tag);
                }
                line
            })
        }),
        SessionCmd::Show { id } => match svc.sessions().into_iter().find(|s| s.id == id) {
            Some(s) => out.ok(&s.report, |r| {
                serde_json::to_string_pretty(r).unwrap_or_default()
            }),
            None => out.error(&unilux_core::Error::Invalid(format!("no report {id}"))),
        },
        SessionCmd::Record => out.result(svc.record_session(None), |s| {
            t_args("cli-session-saved", &[("id", s.id.clone())])
        }),
    }
}

pub async fn community(out: &Out, svc: &Service, cmd: CommunityCmd) -> ExitCode {
    match cmd {
        CommunityCmd::List => out.result(svc.community().await, |i| {
            if i.recipes.is_empty() {
                return t("cli-community-none").into();
            }
            joined(&i.recipes, |e| {
                format!(
                    "{:10} {}  {}  [{}]{}",
                    format!("{:?}", e.verdict.status),
                    e.id,
                    e.title,
                    e.ingredients.join(", "),
                    if e.custom_code {
                        format!("  ({})", t("cli-recipe-custom"))
                    } else {
                        String::new()
                    }
                )
            })
        }),
        CommunityCmd::Install { id } => out.result(svc.community_install(&id).await, |r| {
            t_args("cli-community-saved", &[("id", r.id.clone())])
        }),
        CommunityCmd::Verify { dir } => {
            let r = svc
                .library()
                .and_then(|lib| unilux_patches::community::judge_dir(&lib, &dir));
            out.result(r.map(|(_, e)| e), |e| {
                format!("{} ({})\n{}", e.id, e.title, verdict_text(&e.verdict))
            })
        }
        CommunityCmd::Index {
            root,
            write,
            strict,
        } => {
            let r = svc.library().and_then(|lib| {
                let (idx, errors) = unilux_patches::community::index(&lib, &root);
                for e in &errors {
                    eprintln!("warning: {e}");
                }
                if strict && !errors.is_empty() {
                    return Err(unilux_core::Error::Invalid(format!(
                        "{} community recipe(s) do not check",
                        errors.len()
                    )));
                }
                if write {
                    let text = serde_json::to_string_pretty(&idx).unwrap_or_default();
                    std::fs::write(root.join("index.json"), text + "\n")
                        .map_err(|e| unilux_core::Error::io("writing index.json", e))?;
                }
                Ok(idx)
            });
            out.result(r, |i| {
                t_args(
                    "cli-community-indexed",
                    &[("count", i.recipes.len().to_string())],
                )
            })
        }
    }
}
