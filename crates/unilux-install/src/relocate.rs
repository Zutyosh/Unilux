//! Moving the Wine prefix to another folder or drive.

use std::fs::{self, File};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use unilux_core::{Error, Event, Events, IoContext, Result, Severity};

pub const TASK: &str = "move_prefix";

/// Where the prefix lands when the user picks `chosen`: the folder itself
/// when it is empty or missing, otherwise a new folder inside it.
pub fn target_for(chosen: &Path) -> PathBuf {
    if is_empty_or_missing(chosen) {
        chosen.to_path_buf()
    } else {
        chosen.join("unilux-prefix")
    }
}

/// Refuse targets that would lose data or nest the prefix in itself.
pub fn check_target(from: &Path, to: &Path) -> Result<()> {
    if !to.is_absolute() {
        return Err(Error::Invalid(format!(
            "{} is not an absolute path",
            to.display()
        )));
    }
    let (from_r, to_r) = (resolve(from), resolve(to));
    if from_r == to_r {
        return Err(Error::Invalid(format!(
            "the prefix is already in {}",
            to.display()
        )));
    }
    if to_r.starts_with(&from_r) {
        return Err(Error::Invalid(format!(
            "{} is inside the current prefix",
            to.display()
        )));
    }
    if !is_empty_or_missing(to) {
        return Err(Error::Invalid(format!("{} is not empty", to.display())));
    }
    Ok(())
}

/// Move `from` to `to`: a rename on the same filesystem, otherwise a copy
/// with progress events. `commit` runs once the prefix is complete at `to`
/// and before the old one goes; when it fails the move is undone.
pub fn move_prefix(
    from: &Path,
    to: &Path,
    events: &Events,
    cancel: &AtomicBool,
    commit: impl FnOnce() -> Result<()>,
) -> Result<()> {
    check_target(from, to)?;
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).ctx(|| format!("creating {}", parent.display()))?;
    }
    if to.exists() {
        fs::remove_dir(to).ctx(|| format!("replacing {}", to.display()))?;
    }
    match fs::rename(from, to) {
        Ok(()) => {
            return commit().inspect_err(|_| {
                let _ = fs::rename(to, from);
            });
        }
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {}
        Err(e) => {
            return Err(Error::io(
                format!("moving {} to {}", from.display(), to.display()),
                e,
            ));
        }
    }
    let total = tree_size(from)?;
    check_space(to, total)?;
    let copied = copy_tree(from, to, total, events, cancel).and_then(|()| commit());
    if let Err(e) = copied {
        let _ = fs::remove_dir_all(to);
        return Err(e);
    }
    if let Err(e) = fs::remove_dir_all(from) {
        let _ = events.send(Event::Notice {
            severity: Severity::Warn,
            message: format!(
                "The prefix now lives in {}, but the old copy in {} could not be fully removed: {e}",
                to.display(),
                from.display()
            ),
        });
    }
    Ok(())
}

/// Copy a prefix tree, keeping symlinks, permissions and file times.
/// Absolute links into the old prefix are pointed at the new one.
pub fn copy_tree(
    from: &Path,
    to: &Path,
    total: u64,
    events: &Events,
    cancel: &AtomicBool,
) -> Result<()> {
    let mut c = Copier {
        root_from: from,
        root_to: to,
        events,
        cancel,
        done: 0,
        reported: 0,
        total,
    };
    c.entry(from, to)?;
    c.report();
    Ok(())
}

struct Copier<'a> {
    root_from: &'a Path,
    root_to: &'a Path,
    events: &'a Events,
    cancel: &'a AtomicBool,
    done: u64,
    reported: u64,
    total: u64,
}

impl Copier<'_> {
    fn entry(&mut self, src: &Path, dst: &Path) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let meta = fs::symlink_metadata(src).ctx(|| format!("reading {}", src.display()))?;
        let kind = meta.file_type();
        if kind.is_symlink() {
            let mut target = fs::read_link(src).ctx(|| format!("reading {}", src.display()))?;
            if let Ok(rest) = target.strip_prefix(self.root_from) {
                target = self.root_to.join(rest);
            }
            std::os::unix::fs::symlink(&target, dst)
                .ctx(|| format!("linking {}", dst.display()))?;
        } else if kind.is_dir() {
            fs::create_dir(dst).ctx(|| format!("creating {}", dst.display()))?;
            for e in fs::read_dir(src).ctx(|| format!("listing {}", src.display()))? {
                let e = e.ctx(|| format!("listing {}", src.display()))?;
                self.entry(&e.path(), &dst.join(e.file_name()))?;
            }
            fs::set_permissions(dst, meta.permissions())
                .ctx(|| format!("setting permissions on {}", dst.display()))?;
        } else if kind.is_file() {
            self.done += fs::copy(src, dst)
                .ctx(|| format!("copying {} to {}", src.display(), dst.display()))?;
            if let Ok(modified) = meta.modified() {
                let _ = File::options()
                    .write(true)
                    .open(dst)
                    .and_then(|f| f.set_modified(modified));
            }
            if self.done - self.reported >= (self.total / 200).max(1 << 26) {
                self.report();
            }
        }
        Ok(())
    }

    fn report(&mut self) {
        self.reported = self.done;
        let _ = self.events.send(Event::Progress {
            task: TASK.into(),
            done: self.done,
            total: Some(self.total),
        });
    }
}

/// Bytes in regular files under `p`, symlinks not followed.
pub fn tree_size(p: &Path) -> Result<u64> {
    let meta = fs::symlink_metadata(p).ctx(|| format!("reading {}", p.display()))?;
    if !meta.is_dir() {
        return Ok(if meta.is_file() { meta.len() } else { 0 });
    }
    let mut n = 0;
    for e in fs::read_dir(p).ctx(|| format!("listing {}", p.display()))? {
        n += tree_size(&e.ctx(|| format!("listing {}", p.display()))?.path())?;
    }
    Ok(n)
}

fn check_space(to: &Path, needed: u64) -> Result<()> {
    let Some(dir) = to.ancestors().find(|a| a.exists()) else {
        return Ok(());
    };
    let Ok(st) = nix::sys::statvfs::statvfs(dir) else {
        return Ok(());
    };
    let free = st.blocks_available() * st.fragment_size();
    if free < needed {
        return Err(Error::Invalid(format!(
            "{} has {:.1} GB free; the prefix needs {:.1} GB",
            dir.display(),
            free as f64 / 1e9,
            needed as f64 / 1e9
        )));
    }
    Ok(())
}

fn is_empty_or_missing(p: &Path) -> bool {
    match fs::read_dir(p) {
        Ok(mut d) => d.next().is_none(),
        Err(_) => fs::symlink_metadata(p).is_err(),
    }
}

/// `p` with symlinks resolved as far as it exists.
fn resolve(p: &Path) -> PathBuf {
    for a in p.ancestors() {
        if let Ok(real) = fs::canonicalize(a) {
            return match p.strip_prefix(a) {
                Ok(rest) if !rest.as_os_str().is_empty() => real.join(rest),
                _ => real,
            };
        }
    }
    p.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefix(root: &Path) -> PathBuf {
        let p = root.join("prefix");
        fs::create_dir_all(p.join("drive_c/windows")).unwrap();
        fs::create_dir_all(p.join("dosdevices")).unwrap();
        fs::write(p.join("system.reg"), "reg").unwrap();
        fs::write(p.join("drive_c/windows/a.dll"), vec![7u8; 4096]).unwrap();
        std::os::unix::fs::symlink("../drive_c", p.join("dosdevices/c:")).unwrap();
        std::os::unix::fs::symlink(p.join("drive_c/windows"), p.join("winlink")).unwrap();
        std::os::unix::fs::symlink("/", p.join("dosdevices/z:")).unwrap();
        p
    }

    #[test]
    fn picks_a_subfolder_of_a_busy_folder() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(target_for(d.path()), d.path());
        assert_eq!(target_for(&d.path().join("new")), d.path().join("new"));
        fs::write(d.path().join("game.iso"), "x").unwrap();
        assert_eq!(target_for(d.path()), d.path().join("unilux-prefix"));
    }

    #[test]
    fn refuses_unsafe_targets() {
        let d = tempfile::tempdir().unwrap();
        let p = prefix(d.path());
        assert!(check_target(&p, &p).is_err());
        assert!(check_target(&p, &p.join("drive_c/sub")).is_err());
        assert!(check_target(&p, Path::new("relative")).is_err());
        assert!(check_target(&p, d.path()).is_err());
        assert!(check_target(&p, &d.path().join("elsewhere")).is_ok());
    }

    #[test]
    fn renames_and_commits() {
        let d = tempfile::tempdir().unwrap();
        let p = prefix(d.path());
        let to = d.path().join("games/somnium");
        let ev = unilux_core::event_bus();
        move_prefix(&p, &to, &ev, &AtomicBool::new(false), || Ok(())).unwrap();
        assert!(!p.exists());
        assert_eq!(fs::read_to_string(to.join("system.reg")).unwrap(), "reg");
    }

    #[test]
    fn undoes_the_move_when_commit_fails() {
        let d = tempfile::tempdir().unwrap();
        let p = prefix(d.path());
        let to = d.path().join("other");
        let ev = unilux_core::event_bus();
        let r = move_prefix(&p, &to, &ev, &AtomicBool::new(false), || {
            Err(Error::Invalid("disk full".into()))
        });
        assert!(r.is_err());
        assert!(p.join("system.reg").exists());
        assert!(!to.exists());
    }

    #[test]
    fn copies_links_permissions_and_times() {
        let d = tempfile::tempdir().unwrap();
        let p = prefix(d.path());
        let dll = p.join("drive_c/windows/a.dll");
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        File::options()
            .write(true)
            .open(&dll)
            .unwrap()
            .set_modified(old)
            .unwrap();
        let to = d.path().join("copy");
        let ev = unilux_core::event_bus();
        let mut rx = ev.subscribe();
        let total = tree_size(&p).unwrap();
        assert_eq!(total, 3 + 4096);
        copy_tree(&p, &to, total, &ev, &AtomicBool::new(false)).unwrap();
        assert_eq!(
            fs::read_link(to.join("dosdevices/c:")).unwrap(),
            Path::new("../drive_c")
        );
        assert_eq!(
            fs::read_link(to.join("dosdevices/z:")).unwrap(),
            Path::new("/")
        );
        assert_eq!(
            fs::read_link(to.join("winlink")).unwrap(),
            to.join("drive_c/windows")
        );
        assert_eq!(
            fs::read(to.join("drive_c/windows/a.dll")).unwrap().len(),
            4096
        );
        assert_eq!(
            fs::metadata(to.join("drive_c/windows/a.dll"))
                .unwrap()
                .modified()
                .unwrap(),
            old
        );
        let mut last = None;
        while let Ok(e) = rx.try_recv() {
            last = Some(e);
        }
        assert_eq!(
            last,
            Some(Event::Progress {
                task: TASK.into(),
                done: total,
                total: Some(total)
            })
        );
    }

    #[test]
    fn stops_when_cancelled() {
        let d = tempfile::tempdir().unwrap();
        let p = prefix(d.path());
        let ev = unilux_core::event_bus();
        let r = copy_tree(&p, &d.path().join("c"), 1, &ev, &AtomicBool::new(true));
        assert!(matches!(r, Err(Error::Cancelled)));
    }
}
