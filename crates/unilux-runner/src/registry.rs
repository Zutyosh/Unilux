//! Registry settings the prefix needs, written into Wine's `user.reg`
//! before a launch, while nothing runs in the prefix.

use std::path::Path;

use unilux_core::{Result, write_atomic};

const AVALON: &str = r"[Software\\Microsoft\\Avalon.Graphics]";
const SOFTWARE_WPF: &str = "\"DisableHWAcceleration\"=dword:00000001";

/// Somnium's launcher is a WPF window with a transparent background. Drawn
/// through Direct3D (DXVK), Wine loses what is on screen when the window
/// changes focus and WPF only redraws the parts that changed: the window
/// falls apart. WPF's software renderer draws the whole window each time.
/// Only WPF reads this key; the game itself (Unity) is unaffected.
/// Returns true when the file changed.
pub fn apply(prefix: &Path) -> Result<bool> {
    let path = prefix.join("user.reg");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(false);
    };
    match software_wpf(&text) {
        Some(new) => write_atomic(&path, new.as_bytes()).map(|_| true),
        None => Ok(false),
    }
}

/// `text` with WPF software rendering on, or None when it already is.
fn software_wpf(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let header = lines.iter().position(|l| {
        l.get(..AVALON.len())
            .is_some_and(|h| h.eq_ignore_ascii_case(AVALON))
    });
    let Some(header) = header else {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut out = text.trim_end_matches('\n').to_string();
        out.push_str(&format!("\n\n{AVALON} {secs}\n{SOFTWARE_WPF}\n"));
        return Some(out);
    };
    let end = lines[header + 1..]
        .iter()
        .position(|l| l.starts_with('['))
        .map_or(lines.len(), |i| header + 1 + i);
    let value = (header + 1..end).find(|&i| {
        lines[i]
            .to_ascii_lowercase()
            .starts_with("\"disablehwacceleration\"=")
    });
    let mut out: Vec<&str> = lines.clone();
    match value {
        Some(i) if lines[i].eq_ignore_ascii_case(SOFTWARE_WPF) => return None,
        Some(i) => out[i] = SOFTWARE_WPF,
        None => {
            let at = (header + 1..end)
                .find(|&i| !lines[i].starts_with('#'))
                .unwrap_or(end);
            out.insert(at, SOFTWARE_WPF);
        }
    }
    let mut s = out.join("\n");
    s.push('\n');
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "WINE REGISTRY Version 2\n;; All keys relative to \\\\User\\\\S-1-5-21\n\n#arch=win64\n\n[Software\\\\Wine] 1700000000\n#time=1\n\"Version\"=\"win10\"\n";

    #[test]
    fn adds_the_key_once() {
        let once = software_wpf(BASE).unwrap();
        assert!(once.starts_with(BASE));
        assert!(once.contains("[Software\\\\Microsoft\\\\Avalon.Graphics] "));
        assert!(once.ends_with("\n\"DisableHWAcceleration\"=dword:00000001\n"));
        assert_eq!(software_wpf(&once), None);
    }

    #[test]
    fn fixes_an_existing_section() {
        let off = format!(
            "{BASE}\n[Software\\\\Microsoft\\\\Avalon.Graphics] 1\n#time=2\n\"DisableHWAcceleration\"=dword:00000000\n\"Other\"=dword:00000001\n\n[Software\\\\Next] 1\n"
        );
        let on = software_wpf(&off).unwrap();
        assert_eq!(on, off.replace("dword:00000000", "dword:00000001"));
        let bare = format!(
            "{BASE}\n[Software\\\\Microsoft\\\\Avalon.Graphics] 1\n#time=2\n\n[Software\\\\Next] 1\n"
        );
        let on = software_wpf(&bare).unwrap();
        assert!(
            on.contains("#time=2\n\"DisableHWAcceleration\"=dword:00000001\n\n[Software\\\\Next]")
        );
    }

    #[test]
    fn a_prefix_without_registry_is_left_alone() {
        let d = tempfile::tempdir().unwrap();
        assert!(!apply(d.path()).unwrap());
        std::fs::write(d.path().join("user.reg"), BASE).unwrap();
        assert!(apply(d.path()).unwrap());
        assert!(!apply(d.path()).unwrap());
    }
}
