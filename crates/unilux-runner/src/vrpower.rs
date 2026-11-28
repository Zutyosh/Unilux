//! Putting AMD GPUs in their VR power profile. The sysfs file belongs to
//! root, so the write goes through pkexec and polkit asks for the password.

use std::collections::BTreeMap;
use std::process::Stdio;

use tokio::io::AsyncWriteExt;
use unilux_core::{Error, Result};
use unilux_system::{Host, PowerProfile, amd_power, gpus};

/// AMD GPUs that have a VR profile and are not in it.
pub fn pending(host: &dyn Host) -> Vec<PowerProfile> {
    amd_power(host, &gpus(host))
        .into_iter()
        .filter(PowerProfile::needs_vr)
        .collect()
}

/// A DRM card name (`card0`), so the path handed to root stays under
/// /sys/class/drm.
fn is_card(name: &str) -> bool {
    name.strip_prefix("card")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// tee by absolute path: pkexec would otherwise resolve it through the
/// caller's PATH.
fn tee_path(host: &dyn Host) -> Result<String> {
    ["/usr/bin/tee", "/bin/tee"]
        .iter()
        .find(|p| host.exists(p.as_ref()))
        .map(|p| p.to_string())
        .or_else(|| {
            unilux_system::find_in_path(host, "tee")
                .map(|p| host.canonicalize(&p).display().to_string())
        })
        .ok_or_else(|| Error::Invalid("tee is missing.".into()))
}

/// Switch every pending GPU to its VR profile. Returns how many were
/// switched; nothing is asked when all of them already are.
pub async fn set_vr_power_profile(host: &dyn Host) -> Result<usize> {
    let todo = pending(host);
    if todo.is_empty() {
        return Ok(0);
    }
    if unilux_system::find_in_path(host, "pkexec").is_none() {
        return Err(Error::Invalid(
            "pkexec is missing (polkit), so Unilux cannot change the GPU power profile.".into(),
        ));
    }
    let tee = tee_path(host)?;
    // One prompt per profile index; in practice one in all.
    let mut by_idx: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for p in &todo {
        if !is_card(&p.card) {
            continue;
        }
        if let Some(i) = p.vr_index {
            by_idx
                .entry(i)
                .or_default()
                .push(p.path().display().to_string());
        }
    }
    for (idx, files) in by_idx {
        let mut child = tokio::process::Command::new("pkexec")
            .arg(&tee)
            .args(&files)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::io("pkexec", e))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(format!("{idx}\n").as_bytes())
                .await
                .map_err(|e| Error::io("pkexec", e))?;
        }
        let out = child
            .wait_with_output()
            .await
            .map_err(|e| Error::io("pkexec", e))?;
        match out.status.code() {
            Some(0) => {}
            Some(126) => return Err(Error::Cancelled),
            _ => {
                return Err(Error::Invalid(format!(
                    "Could not set the GPU power profile: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                )));
            }
        }
    }
    let left = pending(host);
    if !left.is_empty() {
        return Err(Error::Invalid(format!(
            "The driver kept the power profile of {}.",
            left.iter()
                .map(|p| p.card.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    Ok(todo.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_drm_cards_reach_root() {
        assert!(is_card("card0"));
        assert!(is_card("card12"));
        for bad in ["card", "renderD128", "card0/../../x", "card1 ", ""] {
            assert!(!is_card(bad), "{bad:?}");
        }
    }
}
