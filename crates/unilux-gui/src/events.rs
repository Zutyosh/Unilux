//! Somnium Space's live side on the main screen: players online, the next
//! events, and the optional sign-in that unlocks them.

use freya::prelude::*;
use unilux_service::{Account, LoginStep, TwoFactor, TwoFactorMethod};

use crate::i18n::{t, t_args};
use crate::state::Ui;
use crate::svc;
use crate::theme::{RADIUS_CONTROL, RADIUS_PILL, T, alpha, size, space};
use crate::widgets::{Tone, body, dot, link, muted, pill};

/// Top bar: how many people are in Somnium right now.
pub fn online_badge(ui: Ui) -> Option<Element> {
    let o = ui.online.read().clone()?;
    Some(
        rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(space::S)
            .child(dot(if o.current > 0 {
                Tone::Ok
            } else {
                Tone::Neutral
            }))
            .child(
                label()
                    .text(t_args("events-online", &[("count", o.current.to_string())]))
                    .font_size(size::SMALL)
                    .font_weight(FontWeight::MEDIUM)
                    .color(T.muted),
            )
            .into_element(),
    )
}

#[cfg(test)]
thread_local! {
    /// Snapshot test: open the form on the code step.
    pub static TEST_CHALLENGE: std::cell::RefCell<Option<TwoFactor>> = const { std::cell::RefCell::new(None) };
}

fn test_challenge() -> Option<TwoFactor> {
    #[cfg(test)]
    {
        return TEST_CHALLENGE.with(|c| c.borrow().clone());
    }
    #[allow(unreachable_code)]
    None
}

/// Sign in to Somnium Space, or the signed-in account with a way out.
#[derive(PartialEq)]
pub struct AccountForm(pub Ui);

impl Component for AccountForm {
    fn render(&self) -> impl IntoElement {
        let mut ui = self.0;
        let login = use_state(String::new);
        let mut password = use_state(String::new);
        let mut code = use_state(String::new);
        let mut challenge = use_state(test_challenge);
        let mut method = use_state(|| TwoFactorMethod::Authenticator);
        let mut sms_sent = use_state(|| false);
        let mut busy = use_state(|| false);
        let mut error = use_state(|| None::<String>);
        let mut signed_in = move |a: Account| {
            password.set(String::new());
            code.set(String::new());
            challenge.set(None);
            ui.account.set(Some(a));
            ui.refresh_events();
        };
        let mut submit = move || {
            let (l, p) = (login.peek().clone(), password.peek().clone());
            if *busy.peek() || l.trim().is_empty() || p.is_empty() {
                return;
            }
            busy.set(true);
            error.set(None);
            spawn_in_window(async move {
                let r = tokio::spawn(async move { svc().somnium_login(&l, &p).await }).await;
                busy.set(false);
                match r {
                    Ok(Ok(LoginStep::Done(a))) => signed_in(a),
                    Ok(Ok(LoginStep::Code(c))) => {
                        password.set(String::new());
                        method.set(if c.authenticator {
                            TwoFactorMethod::Authenticator
                        } else {
                            TwoFactorMethod::Sms
                        });
                        sms_sent.set(false);
                        challenge.set(Some(c));
                    }
                    Ok(Err(e)) => error.set(Some(e.to_string())),
                    Err(e) => error.set(Some(e.to_string())),
                }
            });
        };
        let mut verify = move || {
            let (c, m) = (code.peek().clone(), *method.peek());
            if *busy.peek() || c.trim().is_empty() {
                return;
            }
            busy.set(true);
            error.set(None);
            spawn_in_window(async move {
                let r = tokio::spawn(async move { svc().somnium_verify(&c, m).await }).await;
                busy.set(false);
                match r {
                    Ok(Ok(a)) => signed_in(a),
                    Ok(Err(e)) => {
                        code.set(String::new());
                        error.set(Some(e.to_string()))
                    }
                    Err(e) => error.set(Some(e.to_string())),
                }
            });
        };
        let mut send_sms = move || {
            if *busy.peek() {
                return;
            }
            busy.set(true);
            error.set(None);
            spawn_in_window(async move {
                let r = tokio::spawn(async { svc().somnium_send_sms().await }).await;
                busy.set(false);
                match r {
                    Ok(Ok(())) => sms_sent.set(true),
                    Ok(Err(e)) => error.set(Some(e.to_string())),
                    Err(e) => error.set(Some(e.to_string())),
                }
            });
        };
        if let Some(a) = ui.account.read().clone() {
            return rect()
                .width(Size::fill())
                .horizontal()
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(space::M)
                .padding((space::M, space::L))
                .child(dot(Tone::Ok))
                .child(
                    rect()
                        .width(Size::flex(1.))
                        .spacing(2.)
                        .child(body(a.name))
                        .child(muted(t("events-signed-unilux-keeps-only"))),
                )
                .child(
                    Button::new()
                        .compact()
                        .on_press(move |_| match svc().somnium_logout() {
                            Ok(()) => {
                                ui.account.set(None);
                                ui.events.set(Vec::new());
                            }
                            Err(e) => ui.error.set(Some(e.to_string())),
                        })
                        .child(t("events-sign-out")),
                )
                .into_element();
        }
        let is_busy = *busy.read();
        let err = error.read().clone().map(|e| body(e).color(T.error));
        if let Some(c) = challenge.read().clone() {
            let m = *method.read();
            let hint = match m {
                TwoFactorMethod::Authenticator => t("events-enter-6-digit-code").to_string(),
                TwoFactorMethod::Sms => match (&c.phone, *sms_sent.read()) {
                    (Some(p), true) => t_args("events-code-sent", &[("phone", p.clone())]),
                    (_, true) => t("events-code-sent-sms").to_string(),
                    (Some(p), false) => t_args("events-get-code-sms", &[("phone", p.clone())]),
                    (None, false) => t("events-get-code-sms-no-number").to_string(),
                },
                TwoFactorMethod::BackupCode => t("events-enter-one-backup-codes").to_string(),
            };
            let mut options = Vec::new();
            if c.authenticator {
                options.push((TwoFactorMethod::Authenticator, t("events-app")));
            }
            if c.sms {
                options.push((TwoFactorMethod::Sms, "SMS"));
            }
            options.push((TwoFactorMethod::BackupCode, t("events-backup-code")));
            let ready = !code.read().trim().is_empty() && !is_busy;
            return rect()
                .width(Size::fill())
                .spacing(space::S)
                .padding((space::M, space::L))
                .child(body(t("events-verification-code")).font_weight(FontWeight::SEMI_BOLD))
                .child(
                    rect().horizontal().spacing(space::XS).children(
                        options
                            .into_iter()
                            .map(|(opt, name)| {
                                let b = Button::new().compact().corner_radius(RADIUS_PILL);
                                let b = if opt == m { b.filled() } else { b.flat() };
                                b.key(name)
                                    .on_press(move |_| {
                                        method.set(opt);
                                        error.set(None)
                                    })
                                    .child(name)
                                    .into_element()
                            })
                            .collect::<Vec<_>>(),
                    ),
                )
                .child(muted(hint))
                .maybe_child((m == TwoFactorMethod::Sms).then(|| {
                    Button::new()
                        .compact()
                        .corner_radius(RADIUS_PILL)
                        .enabled(!is_busy)
                        .on_press(move |_| send_sms())
                        .child(if *sms_sent.read() {
                            t("events-send-again")
                        } else {
                            t("events-send-sms")
                        })
                }))
                .child(
                    Input::new(code)
                        .placeholder(t("events-code"))
                        .auto_focus(true)
                        .width(Size::fill())
                        .on_submit(move |_| verify()),
                )
                .maybe_child(err)
                .child(
                    rect()
                        .width(Size::fill())
                        .horizontal()
                        .cross_align(Alignment::Center)
                        .spacing(space::M)
                        .padding((space::XS, 0., 0., 0.))
                        .child(
                            pill(if is_busy {
                                t("events-checking")
                            } else {
                                t("events-confirm")
                            })
                            .corner_radius(RADIUS_PILL)
                            .padding((10., 28.))
                            .enabled(ready)
                            .on_press(move |_| verify()),
                        )
                        .child(link(t("cancel")).on_press(move |_| {
                            challenge.set(None);
                            code.set(String::new());
                            error.set(None)
                        })),
                )
                .into_element();
        }
        let field = |s: State<String>, ph: &'static str, hidden: bool| {
            let mut i = Input::new(s).placeholder(ph).width(Size::fill());
            if hidden {
                i = i.mode(InputMode::new_password());
            }
            i.on_submit(move |_| submit())
        };
        let ready = !login.read().trim().is_empty() && !password.read().is_empty();
        rect()
            .width(Size::fill())
            .spacing(space::S)
            .padding((space::M, space::L))
            .child(field(login, t("events-e-mail-user-name"), false))
            .child(field(password, t("events-password"), true))
            .maybe_child(err)
            .child(
                rect()
                    .width(Size::fill())
                    .horizontal()
                    .cross_align(Alignment::Center)
                    .spacing(space::M)
                    .padding((space::XS, 0., 0., 0.))
                    .child(
                        pill(if is_busy {
                            t("events-signing")
                        } else {
                            t("events-sign")
                        })
                        .corner_radius(RADIUS_PILL)
                        .padding((10., 28.))
                        .enabled(ready && !is_busy)
                        .on_press(move |_| submit()),
                    ),
            )
            .child(muted(t("events-optional-unlocks-events-main")).color(alpha(T.text, 0.6)))
            .corner_radius(RADIUS_CONTROL)
            .into_element()
    }
}
