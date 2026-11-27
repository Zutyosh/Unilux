//! The launcher's background: a 360° night panorama, seen through a
//! slowly turning camera, like Minecraft's title screen. The picture keeps
//! its full resolution; a shader projects it onto the window at the display's
//! own density. Nothing moves while the game runs, so VR keeps every frame.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use freya::engine::prelude::{
    Data, FilterMode, Image, MipmapMode, Paint, RuntimeEffect, SamplingOptions, SkRect, TileMode,
};
use freya::prelude::*;

use crate::state::Ui;

/// Equirectangular, 2:1, 7680×3840.
const PANORAMA: &[u8] = include_bytes!("../assets/panorama.jpg");
/// The same picture at 2048×1024: decoded in a few milliseconds, it fills
/// the first frames while the full one decodes, so the launcher never opens
/// on an empty background.
const PREVIEW: &[u8] = include_bytes!("../assets/panorama-preview.jpg");
/// One full turn every four minutes.
const TURN_SECS: f32 = 240.;
/// Slightly down, so the ground and the lights sit in the middle.
const PITCH: f32 = -0.08;
/// A gentle nod around that pitch, radians.
const NOD: f32 = 0.035;
/// Vertical field of view, degrees.
const FOV: f32 = 80.;
/// Where the camera starts: the lit house and its fountain left of centre,
/// the big tree on the right, almost all the way round the picture.
const START: f32 = 0.494 * std::f32::consts::TAU;
/// The camera holds that first view this long before it starts turning.
const HOLD: Duration = Duration::from_secs(7);
const FRAME: Duration = Duration::from_millis(16);
const IDLE: Duration = Duration::from_millis(100);
/// Seconds the camera has turned, kept across screens so coming back from
/// Settings picks up where it left off.
static CLOCK: AtomicU32 = AtomicU32::new(0);
/// When Unilux started, for the hold.
static STARTED: OnceLock<Instant> = OnceLock::new();

/// Projects the panorama for a camera turned by `yaw` and tilted by
/// `pitch`, then darkens the top and the bottom under the text.
const SKSL: &str = r"
uniform shader pano;
uniform float2 size;
uniform float2 tex;
uniform float yaw;
uniform float pitch;
uniform float fov;

half4 main(float2 p) {
    float2 ndc = (p - size * 0.5) / (size.y * 0.5);
    float3 d = normalize(float3(ndc.x * fov, -ndc.y * fov, 1.0));
    float cp = cos(pitch);
    float sp = sin(pitch);
    d = float3(d.x, d.y * cp + d.z * sp, d.z * cp - d.y * sp);
    float lon = atan(d.x, d.z) + yaw;
    float lat = asin(clamp(d.y, -1.0, 1.0));
    float2 uv = float2(lon / 6.2831853 + 0.5, 0.5 - lat / 3.1415927);
    half4 c = pano.eval(uv * tex);
    float y = p.y / size.y;
    float shade = 1.0 - 0.45 * smoothstep(0.18, 0.0, y) - 0.6 * smoothstep(0.45, 1.0, y);
    return half4(c.rgb * half(shade), 1.0);
}
";

/// The decoded pictures: the full one prepared off the UI thread at start.
static IMAGE: OnceLock<Option<Image>> = OnceLock::new();
static SMALL: OnceLock<Option<Image>> = OnceLock::new();

/// Decode the preview now and the full panorama in the background, before
/// the window opens.
pub fn preload() {
    STARTED.get_or_init(Instant::now);
    std::thread::spawn(|| {
        IMAGE.get_or_init(|| decode(PANORAMA));
    });
    SMALL.get_or_init(|| decode(PREVIEW));
}

/// Decode now, on this thread: tests render the launcher right away.
#[cfg(test)]
pub fn load() {
    IMAGE.get_or_init(|| decode(PANORAMA));
}

fn decode(bytes: &'static [u8]) -> Option<Image> {
    let image = Image::from_encoded(Data::new_copy(bytes))?;
    image.make_raster_image(None, None).or(Some(image))
}

/// The best picture ready right now.
fn picture() -> Option<&'static Image> {
    match IMAGE.get() {
        Some(Some(full)) => Some(full),
        _ => SMALL.get_or_init(|| decode(PREVIEW)).as_ref(),
    }
}

thread_local! {
    /// Compiled once per thread; only the UI thread draws.
    static EFFECT: Option<RuntimeEffect> = RuntimeEffect::make_for_shader(SKSL, None)
        .inspect_err(|e| eprintln!("unilux-gui: panorama shader: {e}"))
        .ok();
}

#[derive(PartialEq)]
pub struct Panorama(pub Ui);

impl Component for Panorama {
    fn render(&self) -> impl IntoElement {
        let ui = self.0;
        let mut clock = use_state(|| f32::from_bits(CLOCK.load(Ordering::Relaxed)));
        let mut ready = use_state(|| IMAGE.get().is_some());
        use_future(move || async move {
            let mut last = Instant::now();
            let mut still = false;
            loop {
                // Standing still, a few checks a second are enough.
                tokio::time::sleep(if still { IDLE } else { FRAME }).await;
                let now = Instant::now();
                let dt = now.duration_since(last).as_secs_f32().min(0.1);
                last = now;
                if !*ready.peek() && IMAGE.get().is_some() {
                    ready.set(true);
                }
                let held = STARTED.get_or_init(Instant::now).elapsed() < HOLD;
                let st = ui.status.peek();
                let playing = st.session_pid.is_some() || st.operation.as_deref() == Some("launch");
                drop(st);
                still = held || playing;
                if still {
                    continue;
                }
                let t = *clock.peek() + dt;
                CLOCK.store(t.to_bits(), Ordering::Relaxed);
                clock.set(t);
            }
        });
        let t = *clock.read();
        let _ = *ready.read();
        rect()
            .position(Position::new_absolute().top(0.).left(0.))
            .width(Size::fill())
            .height(Size::fill())
            .background(RenderCallback::new(move |ctx: &mut FillRenderContext| {
                if let RenderTarget::Background { canvas, .. } = &ctx.target {
                    draw(canvas, ctx.size.width, ctx.size.height, t);
                }
            }))
    }
}

fn draw(canvas: &freya::engine::prelude::Canvas, w: f32, h: f32, t: f32) {
    EFFECT.with(|effect| {
        if let (Some(image), Some(effect)) = (picture(), effect) {
            draw_with(canvas, effect, image, w, h, t);
        }
    });
}

fn draw_with(
    canvas: &freya::engine::prelude::Canvas,
    effect: &RuntimeEffect,
    image: &Image,
    w: f32,
    h: f32,
    t: f32,
) {
    let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear);
    let Some(pano) = image.to_shader((TileMode::Repeat, TileMode::Clamp), sampling, None) else {
        return;
    };
    let yaw = START + t / TURN_SECS * std::f32::consts::TAU;
    let pitch = PITCH + NOD * (t * std::f32::consts::TAU / 60.).sin();
    let fov = (FOV.to_radians() * 0.5).tan();
    let uniforms: Vec<u8> = [
        w,
        h,
        image.width() as f32,
        image.height() as f32,
        yaw,
        pitch,
        fov,
    ]
    .iter()
    .flat_map(|f| f.to_ne_bytes())
    .collect();
    if uniforms.len() != effect.uniform_size() {
        return;
    }
    let Some(shader) = effect.make_shader(Data::new_copy(&uniforms), &[pano.into()], None) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_shader(shader);
    canvas.draw_rect(SkRect::from_wh(w, h), &paint);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_compiles_and_matches_its_uniforms() {
        let size = EFFECT.with(|e| e.as_ref().map(RuntimeEffect::uniform_size));
        assert_eq!(size, Some(7 * 4), "panorama shader compiles");
    }

    #[test]
    fn picture_is_a_full_resolution_panorama() {
        let image = decode(PANORAMA).expect("panorama decodes");
        assert_eq!(image.width(), 2 * image.height());
        assert!(image.width() >= 7680);
        let small = decode(PREVIEW).expect("preview decodes");
        assert_eq!(small.width(), 2 * small.height());
    }
}
