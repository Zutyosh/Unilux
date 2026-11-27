//! Who the window is to the desktop: app id, icon, menu entry.
//!
//! On Wayland the compositor finds the taskbar and title-bar icon through the
//! window's app id, which must match an installed `unilux.desktop` whose
//! `Icon=` resolves in the icon theme. Packages install both; a bare binary
//! (cargo build, copied file) registers them for the user on first start.

use std::path::{Path, PathBuf};

pub const APP_ID: &str = "unilux";
pub const ICON_256: &[u8] = include_bytes!("../../../assets/unilux-256.png");
const ICON_512: &[u8] = include_bytes!("../../../assets/unilux-512.png");
const DESKTOP: &str = include_str!("../../../packaging/unilux.desktop");

/// Best effort: a missing icon is cosmetic, never a reason not to start.
pub fn register(env: impl Fn(&str) -> Option<String>) {
    let Some(home) = env("HOME").filter(|h| !h.is_empty()).map(PathBuf::from) else {
        return;
    };
    let data_home = env("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"));
    let data_dirs = env("XDG_DATA_DIRS")
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let mut roots = vec![data_home.clone()];
    roots.extend(
        data_dirs
            .split(':')
            .map(PathBuf::from)
            .filter(|p| p.is_absolute()),
    );
    let exe = env("APPIMAGE")
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok());
    for (rel, bytes) in [
        ("icons/hicolor/256x256/apps/unilux.png", ICON_256),
        ("icons/hicolor/512x512/apps/unilux.png", ICON_512),
    ] {
        if !roots.iter().any(|r| r.join(rel).exists()) {
            write(&data_home.join(rel), bytes);
        }
    }
    let entry = data_home.join("applications/unilux.desktop");
    let packaged = roots[1..]
        .iter()
        .any(|r| r.join("applications/unilux.desktop").exists());
    let Some(text) = exe.as_deref().and_then(desktop_entry) else {
        return;
    };
    if packaged {
        return;
    }
    match std::fs::read_to_string(&entry) {
        // Someone else's entry (the user's own, an AppImage integrator): leave it.
        Ok(old) if !old.lines().any(|l| l == MARKER) => {}
        Ok(old) if old == text => {}
        _ => write(&entry, text.as_bytes()),
    }
}

/// Marks the entries Unilux wrote, the only ones it rewrites.
const MARKER: &str = "X-Unilux-Generated=true";

/// The packaged entry, pointed at this binary. None for a path a desktop
/// entry cannot carry (not UTF-8, or with control characters that would
/// start a new line in the file).
fn desktop_entry(exe: &Path) -> Option<String> {
    let exe = exe.to_str().filter(|s| !s.chars().any(char::is_control))?;
    // Exec quoting (desktop entry spec), then the key file's own escaping of
    // backslashes, then % so no part of the path reads as a field code.
    let quoted = exe
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$");
    let exec = format!("\"{quoted}\"")
        .replace('\\', "\\\\")
        .replace('%', "%%");
    let mut text = String::new();
    for line in DESKTOP.lines() {
        if line.starts_with("Exec=") {
            text.push_str(&format!("Exec={exec}\nStartupWMClass={APP_ID}\n{MARKER}\n"));
        } else {
            text.push_str(line);
            text.push('\n');
        }
    }
    Some(text)
}

fn write(path: &Path, bytes: &[u8]) {
    if let Err(e) = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|_| unilux_core::write_atomic(path, bytes).map_err(std::io::Error::other))
    {
        eprintln!("unilux-gui: could not write {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_icon_and_entry_for_a_bare_binary() {
        let home = tempfile::tempdir().unwrap();
        let h = home.path().to_str().unwrap().to_string();
        let sys = home.path().join("sys").to_str().unwrap().to_string();
        register(|k| match k {
            "HOME" => Some(h.clone()),
            "XDG_DATA_DIRS" => Some(sys.clone()),
            "APPIMAGE" => Some("/opt/My Apps/Unilux.AppImage".into()),
            _ => None,
        });
        let share = home.path().join(".local/share");
        assert!(share.join("icons/hicolor/256x256/apps/unilux.png").exists());
        let entry = std::fs::read_to_string(share.join("applications/unilux.desktop")).unwrap();
        assert!(entry.contains("Exec=\"/opt/My Apps/Unilux.AppImage\"\n"));
        assert!(entry.contains("Icon=unilux\n"));
        assert!(entry.contains("StartupWMClass=unilux\n"));
    }

    #[test]
    fn exec_survives_quotes_dollars_percents_and_newlines() {
        let e = desktop_entry(Path::new(r#"/a "b"/$x/50%/c\d"#)).unwrap();
        assert!(e.contains(
            r#"Exec="/a \\"b\\"/\\$x/50%%/c\\\\d"
"#
        ));
        assert_eq!(desktop_entry(Path::new("/a\nExec=evil")), None);
    }

    #[test]
    fn keeps_an_entry_it_did_not_write() {
        let home = tempfile::tempdir().unwrap();
        let apps = home.path().join(".local/share/applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(apps.join("unilux.desktop"), "[Desktop Entry]\nExec=mine\n").unwrap();
        let h = home.path().to_str().unwrap().to_string();
        register(|k| match k {
            "HOME" => Some(h.clone()),
            "XDG_DATA_DIRS" => Some("/nonexistent".into()),
            "APPIMAGE" => Some("/opt/Unilux.AppImage".into()),
            _ => None,
        });
        let kept = std::fs::read_to_string(apps.join("unilux.desktop")).unwrap();
        assert_eq!(kept, "[Desktop Entry]\nExec=mine\n");
    }

    #[test]
    fn window_icon_decodes() {
        let _ = freya::prelude::LaunchConfig::window_icon(ICON_256);
    }

    #[test]
    fn leaves_a_packaged_install_alone() {
        let home = tempfile::tempdir().unwrap();
        let sys = home.path().join("sys");
        std::fs::create_dir_all(sys.join("applications")).unwrap();
        std::fs::create_dir_all(sys.join("icons/hicolor/256x256/apps")).unwrap();
        std::fs::create_dir_all(sys.join("icons/hicolor/512x512/apps")).unwrap();
        std::fs::write(sys.join("applications/unilux.desktop"), "").unwrap();
        std::fs::write(sys.join("icons/hicolor/256x256/apps/unilux.png"), "").unwrap();
        std::fs::write(sys.join("icons/hicolor/512x512/apps/unilux.png"), "").unwrap();
        let h = home.path().to_str().unwrap().to_string();
        let s = sys.to_str().unwrap().to_string();
        register(|k| match k {
            "HOME" => Some(h.clone()),
            "XDG_DATA_DIRS" => Some(s.clone()),
            _ => None,
        });
        assert!(!home.path().join(".local/share").exists());
    }
}
