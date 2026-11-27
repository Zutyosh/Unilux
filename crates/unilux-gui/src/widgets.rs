//! Building blocks bound to the tokens.

use freya::prelude::*;

use crate::theme::{T, alpha, size, space};

pub fn title(text: impl Into<String>) -> Label {
    label()
        .text(text.into())
        .font_size(size::TITLE)
        .font_weight(FontWeight::MEDIUM)
        .color(T.text)
}

pub fn display(text: impl Into<String>) -> Label {
    label()
        .text(text.into())
        .font_size(size::DISPLAY)
        .font_weight(FontWeight::MEDIUM)
        .color(T.text)
}

pub fn body(text: impl Into<String>) -> Label {
    label()
        .text(text.into())
        .font_size(size::BODY)
        .color(T.text)
}

pub fn muted(text: impl Into<String>) -> Label {
    label()
        .text(text.into())
        .font_size(size::SMALL)
        .color(T.muted)
}

/// Frosted panel over the key art.
/// Panel over the key art: near-opaque, so text reads without a blur halo.
/// The main call to action: a filled pill.
pub fn pill(text: impl Into<String>) -> Button {
    Button::new().filled().child(
        label()
            .text(text.into())
            .font_size(size::LEAD)
            .font_weight(FontWeight::SEMI_BOLD)
            .color(T.accent_text),
    )
}

/// A quiet text button for secondary paths.
pub fn link(text: impl Into<String>) -> Button {
    Button::new().flat().compact().child(
        label()
            .text(text.into())
            .font_size(size::SMALL)
            .color(T.muted),
    )
}

/// Line icons, one set (lucide shapes), 1.5 stroke on a 24 grid.
#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Sliders,
    Back,
    Play,
    Wrench,
    Monitor,
}

impl Icon {
    fn paths(self) -> &'static str {
        match self {
            Icon::Sliders => {
                r#"<path d="M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1"/><circle cx="15" cy="6" r="2"/><circle cx="9" cy="12" r="2"/><circle cx="17" cy="18" r="2"/>"#
            }
            Icon::Back => r#"<path d="M19 12H5M12 19l-7-7 7-7"/>"#,
            Icon::Play => {
                r#"<path d="M7 4.5v15a1 1 0 0 0 1.5.86l12.5-7.5a1 1 0 0 0 0-1.72L8.5 3.64A1 1 0 0 0 7 4.5z"/>"#
            }
            Icon::Wrench => {
                r#"<path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>"#
            }
            Icon::Monitor => {
                r#"<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>"#
            }
        }
    }

    /// The icon in one colour at one size.
    pub fn view(self, px: f32, color: (u8, u8, u8)) -> impl IntoElement {
        let svg = format!(
            r#"<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" fill="none" stroke="white" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#,
            self.paths()
        );
        SvgViewer::new(Bytes::from(svg.into_bytes()))
            .stroke(color)
            .width(Size::px(px))
            .height(Size::px(px))
    }
}

/// Sliders icon for Settings.
pub fn settings_icon() -> impl IntoElement {
    Icon::Sliders.view(20., T.text)
}

/// A group of preference rows, libadwaita-style.
/// A group of rows: a small heading, then rows separated by hairlines. No
/// panel: separation comes from lines and space, never a box in a box.
pub fn group(heading_text: &str, rows: Vec<Element>) -> Rect {
    rect()
        .width(Size::fill())
        .spacing(space::XS)
        .child(
            label()
                .text(heading_text.to_string())
                .font_size(size::SMALL)
                .font_weight(FontWeight::MEDIUM)
                .color(T.muted),
        )
        .child(
            rect()
                .width(Size::fill())
                .height(Size::px(1.))
                .background(hairline()),
        )
        .child(rect().width(Size::fill()).children(rows))
}

/// One row: title, optional subtitle, and a trailing control.
/// One row: title, optional subtitle, and a trailing control.
pub fn row(
    title_text: impl Into<String>,
    subtitle: Option<String>,
    trailing: Option<Element>,
) -> Element {
    rect()
        .width(Size::fill())
        .horizontal()
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .padding((space::M, 0.))
        .spacing(space::L)
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(body(title_text))
                .maybe_child(subtitle.map(|s| muted(s).font_size(size::SMALL))),
        )
        .maybe_child(trailing)
        .into_element()
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    Ok,
    Warn,
    Error,
    Neutral,
    Accent,
}

impl Tone {
    pub fn color(self) -> (u8, u8, u8) {
        match self {
            Tone::Ok => T.ok,
            Tone::Warn => T.warn,
            Tone::Error => T.error,
            Tone::Neutral => T.muted,
            Tone::Accent => T.accent,
        }
    }
}

/// Small rounded status label.
/// Status as words: coloured text, no capsule. Neutral reads as muted.
pub fn chip(text: impl Into<String>, tone: Tone) -> Rect {
    rect().child(
        label()
            .text(text.into())
            .font_size(size::SMALL)
            .font_weight(FontWeight::MEDIUM)
            .color(tone.color())
            .max_lines(1),
    )
}

/// A coloured dot, for compact status.
/// A small coloured dot, for compact status.
pub fn dot(tone: Tone) -> Rect {
    rect()
        .width(Size::px(6.))
        .height(Size::px(6.))
        .corner_radius(3.)
        .background(tone.color())
}

pub fn severity_tone(s: unilux_core::Severity) -> Tone {
    match s {
        unilux_core::Severity::Error => Tone::Error,
        unilux_core::Severity::Warn => Tone::Warn,
        unilux_core::Severity::Info => Tone::Neutral,
    }
}

/// The native folder picker; None when cancelled.
pub async fn pick_folder(title: &str) -> Option<std::path::PathBuf> {
    let title = title.to_string();
    tokio::spawn(async move {
        rfd::AsyncFileDialog::new()
            .set_title(title)
            .pick_folder()
            .await
            .map(|h| h.path().to_path_buf())
    })
    .await
    .ok()
    .flatten()
}

/// Percentage of a progress event, if the total is known.
pub fn percent(done: u64, total: Option<u64>) -> Option<f32> {
    total
        .filter(|t| *t > 0)
        .map(|t| (done as f32 * 100. / t as f32).clamp(0., 100.))
}

/// A slim bar with its percentage beside it: the 6px track is too thin to
/// hold the label freya draws inside the fill.
pub fn progress_bar(pct: f32) -> Rect {
    let pct = pct.clamp(0., 100.);
    rect()
        .width(Size::fill())
        .horizontal()
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(space::M)
        .child(
            rect().width(Size::flex(1.)).child(
                ProgressBar::new(pct)
                    .show_progress(false)
                    .width(Size::fill()),
            ),
        )
        .child(
            muted(format!("{:.0}%", pct.floor()))
                .width(Size::px(40.))
                .text_align(TextAlign::End)
                .max_lines(1),
        )
}

/// A verdict strip: a tone, a headline, one sentence and an optional action.
/// The tone lives in a dot; the surface stays neutral like every other group.
/// A verdict line: the headline carries the tone in its colour, then one
/// sentence and an optional action. No surface.
pub fn banner(
    tone: Tone,
    head: impl Into<String>,
    text: impl Into<String>,
    action: Option<Element>,
) -> Element {
    let head_color = match tone {
        Tone::Warn | Tone::Error | Tone::Ok => tone.color(),
        _ => T.text,
    };
    rect()
        .width(Size::fill())
        .horizontal()
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(space::L)
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(
                    label()
                        .text(head.into())
                        .font_size(size::BODY)
                        .font_weight(FontWeight::MEDIUM)
                        .color(head_color),
                )
                .child(muted(text)),
        )
        .maybe_child(action)
        .into_element()
}

/// A page's title and the one sentence that says what it is for.
/// A page's title and the one sentence that says what it is for.
pub fn page_header(title_text: impl Into<String>, lead: impl Into<String>) -> Rect {
    rect()
        .width(Size::fill())
        .spacing(space::XS)
        .child(title(title_text).font_weight(FontWeight::SEMI_BOLD))
        .child(muted(lead).font_size(size::BODY))
}

/// Separator and outline colour for groups: the text at low opacity.
pub fn hairline() -> Color {
    alpha(T.text, 0.08)
}

/// Rows with a hairline between them.
pub fn divided(rows: Vec<Element>) -> Vec<Element> {
    let mut out = Vec::with_capacity(rows.len() * 2);
    for (i, r) in rows.into_iter().enumerate() {
        if i > 0 {
            out.push(
                rect()
                    .key(format!("sep{i}"))
                    .width(Size::fill())
                    .height(Size::px(1.))
                    .background(alpha(T.text, 0.06))
                    .into_element(),
            );
        }
        out.push(r);
    }
    out
}

/// A command or a path, in a small monospace box.
/// A command or a path, in a small monospace box.
pub fn mono_box(text: impl Into<String>) -> Rect {
    rect()
        .width(Size::fill())
        .padding((space::S, space::M))
        .corner_radius(6.)
        .background(T.surface)
        .child(
            label()
                .text(text.into())
                .font_size(size::SMALL)
                .font_family("monospace")
                .color(T.muted),
        )
}

/// A Proton's tag and the title of its provider.
pub fn proton_name(id: &unilux_components::ProtonId) -> (String, String) {
    let provider = unilux_components::proton::provider(&id.provider)
        .map(|p| p.title.to_string())
        .unwrap_or_else(|| id.provider.clone());
    (id.tag.clone(), provider)
}

/// [`proton_name`] for an id saved as text; an unreadable id is shown as is.
pub fn proton_label(id: &str) -> (String, String) {
    match unilux_components::ProtonId::parse(id) {
        Ok(p) => proton_name(&p),
        Err(_) => (id.to_string(), String::new()),
    }
}
