# Unilux design system

The app has one job per screen. Home has one primary action: Play. Everything else (components, settings, diagnostics, logs, agents) lives one click away and never competes with it.

## Layout

Three screens, modelled on the An Anime Team launchers:

- **Launcher**: after the An Anime Team launchers, on plain `bg` (no picture). Brand top left, players online top right. Bottom left, directly on the background with no panel, a systemd-style boot log in monospace: `[  OK  ]`, `[ WARN ]`, `[FAILED]`, `[ INFO ]` tags in `ok`/`warn`/`error`/`muted`, then plain text. Lines: Proton, the OpenXR runtime and its state, the OpenVR layer, the patches; then the current step, the game's last log lines (in `faint`) while it starts and runs, and Unilux notices. Only system state belongs there: no events, no account. Behind it, the sky (`src/sky.rs`): ~150 faint white ambient stars (a few with a four-point glint) and one cold-blue star (#9cc4ff, soft glow) per player online in Somnium, capped at 60; players fade in and out over 1 s as they join and leave, the field stretches outward while the game launches, and nothing moves while the game runs. Bottom right, one wide pill that does the next right thing (Install, Repair, Play, Stop, Cancel), a round Desktop mode button when ready, and a round Settings button.
- **First run**: the plain `bg` background, logo, heading, one sentence and one pill, centred like a libadwaita status page. Pages: welcome (system problems listed inline), Proton (verified builds only), installer source (download by default; drop zone and file picker as the quiet alternative), loading screen (breathing logo inside a spinner, the current step, a rotating line, one overall progress bar), done.
- **Settings**: an opaque, text-only sidebar of six sections: Game (Proton and compatibility recipes: setup, my recipes, community), VR, Performance (recorded sessions), Health (diagnostics, with the launch log unfolded on demand), Agents, Preferences. A section is marked only when it needs attention: a warning dot (Game, VR) or a count (Health, Agents); healthy sections carry nothing. Each section is grouped rows with a title, a subtitle and one trailing control. A build covers everything with its own screen until it ends or goes to the background.

## Tone

Utilitarian, calm, exact. The app reads like a good instrument panel: state first, then the action that changes it.

## Tokens

All colours and type go through `crates/unilux-gui/src/theme.rs`. No literal colour appears in a component.

| Token | Value | Use |
|---|---|---|
| `bg` | #0d0e10 | window background |
| `surface` | #141518 | sidebar, code and log boxes |
| `raised` | #1d1e22 | selected item, toasts |
| `border` | #2a2b30 | input outlines |
| `text` | #e8e6e3 | body text, primary buttons, toggles, progress |
| `muted` | #8b8b92 | secondary text, group headings |
| `faint` | #5d5d64 | tertiary text: timestamps, paths, version |
| `accent` | = `text` | no hue: the interface has no accent colour |
| `ok` | #86c79a | healthy state |
| `warn` | #e8b25a | needs attention |
| `error` | #ef7f74 | failure |

Dark only. **No accent colour**: selection, toggles, progress and focus are off-white `text`. Colour in the interface means state (`ok`, `warn`, `error`) and nothing else. The pink-to-violet of the mark lives only in the logo. Why: a violet accent on graphite read as generic AI-app styling (user decision, redesign of v0.3).

Type: one family (Inter, system sans fallback). Sizes 13 / 15 / 18 / 26 / 36. Body never below 15, except the launcher's monospace boot log at 13. Two weights: 400 and 500; hierarchy comes from size and colour. Group headings are small muted labels.

Icons: one line set (lucide shapes, `widgets::Icon`), 1.5 stroke, coloured like their text, used only where a word is not enough (Play, Back, Repair, Desktop mode, Settings). The settings sidebar is text only.

Status is words, not capsules: `widgets::chip` renders coloured text with no background, and a verdict (`widgets::banner`) is a headline in the tone's colour with no surface. Never stack a dot, a chip and a coloured headline for the same fact.

Surfaces: groups are separated by a hairline and space, not boxes; never a panel inside a panel. Boxes are reserved for code, logs and the wizard card. The app background is plain `bg` everywhere: no key art, no picture, no drawn decoration (user decision).

Spacing scale: 4, 8, 12, 16, 24, 32, 48. Radius: 10 for controls, 16 for floating panels, pills for the launcher actions.

App icon: `assets/unilux-512.png` (and 256), the user's mark: three orbits in a pink-to-violet gradient (#ff1f6a to #8f1fd0) around Somnium's cube, on transparency. `crates/unilux-gui/assets/logo.png` is the same mark. The orbit gradient is the only saturated colour in the app outside state colours and it lives only in the mark.

## Controls

Every control has these states: default, hover, pressed, focused, disabled, loading, success, error. Focus shows a 2 px `text` ring right away, with no fade. Disabled buttons say why in a tooltip ("Connect your headset first").

## Motion

Only opacity and transform, 120 to 200 ms. When the desktop asks for reduced motion, no animation runs. Progress bars move. The launcher sky is the one ambient motion: a slow twinkle (cycles of several seconds), frozen while the game runs so VR keeps every frame.

## Copy rules

- Labels in sentence case, never all caps, never tracked.
- State the fact, then the fix: "No OpenXR runtime found. Start SteamVR or WiVRn, then connect the headset."
- No invented numbers, quotes or user counts.
- No exclamation marks in errors. No "Oops".
- Every string goes through `crates/unilux-i18n` (English, French and Scots), shared with the CLI; machine output (MCP, `--json`, reports) is always English.
