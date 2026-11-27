//! The launcher's sky: a faint field of ambient stars, and one blue star
//! per player in Somnium right now. Players fade in and out as they join and
//! leave; the field stretches outward while the game launches; nothing moves
//! while it runs, so the VR session keeps every frame.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use freya::engine::prelude::{
    Canvas, Paint, PaintStyle, SkBlurStyle, SkColor, SkMaskFilter, SkPoint,
};
use freya::prelude::*;

use crate::state::Ui;

/// Ambient stars: always there, they carry no meaning but depth.
const AMBIENT: u32 = 150;
/// More players than this would only fill the sky.
const MAX_PLAYERS: usize = 60;
const FADE: f32 = 1.0;
const FRAME: Duration = Duration::from_millis(33);

/// Player stars: a cold blue, so they read apart from the white field.
const PLAYER: (u8, u8, u8) = (0x9c, 0xc4, 0xff);
const AMBIENT_RGB: (u8, u8, u8) = (0xe8, 0xe6, 0xe3);

#[derive(Clone, Copy, PartialEq)]
struct Frame {
    /// Player stars before the last change, and now.
    from: usize,
    shown: usize,
    changed: Option<Instant>,
    /// 0 at rest, 1 while the game launches.
    warp: f32,
}

#[derive(PartialEq)]
pub struct Sky(pub Ui);

impl Component for Sky {
    fn render(&self) -> impl IntoElement {
        let ui = self.0;
        let mut frame = use_state(|| Frame {
            from: 0,
            shown: 0,
            changed: None,
            warp: 0.,
        });
        use_future(move || async move {
            loop {
                tokio::time::sleep(FRAME).await;
                let st = ui.status.peek().clone();
                if st.session_pid.is_some() {
                    continue;
                }
                let target = ui
                    .online
                    .peek()
                    .as_ref()
                    .map_or(0, |o| (o.current as usize).min(MAX_PLAYERS));
                let mut f = *frame.peek();
                if target != f.shown {
                    f.from = visible(&f);
                    f.shown = target;
                    f.changed = Some(Instant::now());
                }
                let launching = st.operation.as_deref() == Some("launch");
                f.warp += (if launching { 1. } else { 0. } - f.warp) * 0.08;
                frame.set(f);
            }
        });
        let f = *frame.read();
        rect()
            .position(Position::new_absolute().top(0.).left(0.))
            .width(Size::fill())
            .height(Size::fill())
            .background(RenderCallback::new(move |ctx: &mut FillRenderContext| {
                if let RenderTarget::Background { canvas, .. } = &ctx.target {
                    draw(canvas, ctx.size.width, ctx.size.height, &f);
                }
            }))
    }
}

/// How far the last change has gone, 0 to 1.
fn progress(f: &Frame) -> f32 {
    f.changed
        .map_or(1., |c| (c.elapsed().as_secs_f32() / FADE).min(1.))
}

/// Player stars on screen right now, counting a fading one as present.
fn visible(f: &Frame) -> usize {
    if progress(f) >= 1. {
        f.shown
    } else {
        f.from.max(f.shown)
    }
}

/// A stable pseudo-random number in [0, 1) for a seed.
fn rnd(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x & 0x00FF_FFFF) as f32 / 16_777_216.
}

fn sk(rgb: (u8, u8, u8), a: f32) -> SkColor {
    SkColor::from_argb((a.clamp(0., 1.) * 255.) as u8, rgb.0, rgb.1, rgb.2)
}

fn now() -> f32 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

fn draw(canvas: &Canvas, w: f32, h: f32, f: &Frame) {
    let t = now();
    let (cx, cy) = (w * 0.5, h * 0.45);
    let k = f.warp;
    let place = |x: f32, y: f32| {
        (
            cx + (x - cx) * (1. + k * 0.3),
            cy + (y - cy) * (1. + k * 0.3),
        )
    };

    let mut dot = Paint::default();
    dot.set_anti_alias(true);
    dot.set_style(PaintStyle::Fill);
    let mut line = Paint::default();
    line.set_anti_alias(true);
    line.set_style(PaintStyle::Stroke);
    let mut glow = Paint::default();
    glow.set_anti_alias(true);
    glow.set_mask_filter(SkMaskFilter::blur(SkBlurStyle::Normal, 3., false));

    let star = |x: f32, y: f32, r: f32, rgb: (u8, u8, u8), a: f32, glint: bool, haze: bool| {
        let (px, py) = place(x, y);
        if k > 0.02 {
            let mut l = line.clone();
            l.set_stroke_width(r * 1.6);
            l.set_color(sk(rgb, a * 0.6));
            let (sx, sy) = (
                cx + (x - cx) * (1. + k * 0.08),
                cy + (y - cy) * (1. + k * 0.08),
            );
            canvas.draw_line(SkPoint::new(sx, sy), SkPoint::new(px, py), &l);
        }
        if haze {
            let mut g = glow.clone();
            g.set_color(sk(rgb, a * 0.45));
            canvas.draw_circle(SkPoint::new(px, py), r * 2.6, &g);
        }
        if glint {
            // A four-point glint: two hairlines fading out from the core.
            let mut l = line.clone();
            l.set_stroke_width(0.7);
            l.set_color(sk(rgb, a * 0.35));
            let len = r * 7.;
            canvas.draw_line(SkPoint::new(px - len, py), SkPoint::new(px + len, py), &l);
            canvas.draw_line(SkPoint::new(px, py - len), SkPoint::new(px, py + len), &l);
        }
        let mut d = dot.clone();
        d.set_color(sk(rgb, a));
        canvas.draw_circle(SkPoint::new(px, py), r, &d);
    };

    for i in 0..AMBIENT {
        let s = rnd(i * 4 + 2);
        let x = rnd(i * 4) * w;
        let y = rnd(i * 4 + 1) * h;
        let r = 0.4 + s * s * 0.9;
        let twinkle = 0.8 + 0.2 * (t * (0.25 + s * 0.35) + i as f32).sin();
        let a = (0.1 + s * 0.35) * twinkle * (1. + k);
        star(x, y, r, AMBIENT_RGB, a, s > 0.93, false);
    }

    let p = progress(f);
    let n = visible(f);
    for i in 0..n as u32 {
        let fade = if (i as usize) < f.from.min(f.shown) {
            1.
        } else if f.shown > f.from {
            p
        } else {
            1. - p
        };
        let s = rnd(10_007 + i * 4 + 2);
        // Players keep to the upper sky, above the log and the buttons.
        let x = (0.04 + rnd(10_007 + i * 4) * 0.92) * w;
        let y = (0.06 + rnd(10_007 + i * 4 + 1) * 0.62) * h;
        let r = 1.1 + s * 0.8;
        let twinkle = 0.85 + 0.15 * (t * 0.5 + s * 6.3).sin();
        star(x, y, r, PLAYER, fade * twinkle * 0.9, s > 0.7, true);
    }
}
