//! Root: the key art behind everything, then the launcher, the first-run
//! wizard or the settings.

use freya::prelude::*;

use crate::state::{Screen, use_ui};
use crate::theme::{RADIUS_PANEL, T, alpha, size, space, unilux_theme};
use crate::{launcher, settings, widgets, wizard};

pub const LOGO: &[u8] = include_bytes!("../assets/logo.png");

pub fn app() -> impl IntoElement {
    use_init_theme(unilux_theme);
    let ui = use_ui();
    let screen = *ui.screen.read();
    let body: Element = match screen {
        Screen::Launcher => launcher::LauncherScreen(ui).into_element(),
        Screen::Wizard => wizard::WizardScreen(ui).into_element(),
        Screen::Settings => settings::SettingsScreen(ui).into_element(),
    };
    let lang = *ui.lang.read();
    let body: Element = rect().expanded().key(lang).child(body).into_element();
    rect()
        .expanded()
        .background(T.bg)
        .color(T.text)
        .font_size(size::BODY)
        .maybe_child((screen == Screen::Launcher).then_some(crate::sky::Sky(ui)))
        .child(body)
        .maybe_child(crate::state::building(ui).then_some(crate::build_screen::BuildScreen(ui)))
        .maybe_child(ui.error.read().clone().map(move |msg| error_toast(ui, msg)))
}

pub fn logo(px: f32) -> impl IntoElement {
    ImageViewer::new(LOGO)
        .width(Size::px(px))
        .height(Size::px(px))
}

pub fn brand() -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(space::M)
        .child(logo(32.))
        .child(
            label()
                .text("Unilux")
                .font_size(size::LEAD)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(T.text),
        )
}

fn error_toast(mut ui: crate::state::Ui, msg: String) -> impl IntoElement {
    rect()
        .position(Position::new_absolute().top(space::L).left(0.))
        .width(Size::fill())
        .center()
        .child(
            rect()
                .max_width(Size::px(560.))
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(space::M)
                .padding((space::M, space::L))
                .corner_radius(RADIUS_PANEL)
                .background(T.raised)
                .border(Border::new().fill(alpha(T.error, 0.6)).width(1.))
                .child(widgets::dot(widgets::Tone::Error))
                .child(
                    rect()
                        .max_width(Size::px(440.))
                        .child(label().text(msg).color(T.text)),
                )
                .child(
                    Button::new()
                        .flat()
                        .compact()
                        .on_press(move |_| ui.error.set(None))
                        .child(crate::i18n::t("dismiss")),
                ),
        )
}
