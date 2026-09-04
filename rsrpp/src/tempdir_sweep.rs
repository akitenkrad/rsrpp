//! Recovery of temporary directories that a previous run could not remove itself.
//!
//! `config::TempDir` removes a parse's working directory when the last handle to
//! it is dropped, which covers normal returns, `?` and panics. It cannot cover `SIGKILL`
//! or [`std::process::exit`]: no destructor runs there, so the directory survives its
//! process. That path is taken routinely, not exceptionally — the A/B harness kills a
//! runaway document with `kill -9`, and corpus runs are interrupted the same way.
//!
//! So the directories have to be collected later, by whichever run comes next. The only
//! thing a later process can go on is the directory's own name, which carries the pid of
//! the process that made it (`pdf_<pid>_<seq>_<rand>`, see `ParserConfig::new`).
//!
//! Two properties matter more here than reclaiming every byte:
//!
//! * **A live parse's directory is never touched.** The reports campaign runs 8–12 rsrpp
//!   processes at once against one temp directory; deleting a sibling's files mid-parse
//!   would corrupt a run that was going to succeed. Removal therefore requires positive
//!   evidence that the owning pid is *gone* — "cannot tell" counts as alive.
//! * **Nothing outside rsrpp's own naming scheme is touched.** The system temp directory
//!   belongs to every application on the machine, so the name is matched in full
//!   (`pdf_` + three all-digit fields), not by prefix.
//!
//! The cost of those two rules is that a pid reused by an unrelated live process leaves
//! its directory behind. That is the safe direction to fail in: the directory is picked
//! up by a later run, once that pid is free again.

use std::path::{Path, PathBuf};
use std::sync::Once;

/// What one sweep did. Returned by the internal entry point so the behaviour can be
/// asserted in tests and summarized in one log line; callers of the public API do not
/// see it, because nothing they do should depend on how much was reclaimed.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct SweepReport {
    /// Directories of dead processes that were removed.
    pub(crate) removed: usize,
    /// Directories left alone because their owning process is, or might be, still running.
    pub(crate) skipped_live: usize,
    /// Directories of dead processes that could not be removed. See [`sweep_dir`].
    pub(crate) failed: usize,
}

/// Extracts the pid from a working-directory name, or `None` if the name is not one of
/// ours.
///
/// The shape must match `ParserConfig::new`'s `format!("pdf_{}_{}_{}", pid, seq, rand)`
/// exactly: prefix `pdf_`, then three non-empty all-ASCII-digit fields and nothing else.
/// Anything looser would let this delete another application's files — `pdf_backups` and
/// `pdf_1_2_3_scratch` are not ours, and a prefix match would claim both.
///
/// pid 0 is rejected as well. `std::process::id()` never returns it, so such a name did
/// not come from here, and `kill(0, …)` addresses the caller's whole process group rather
/// than one process.
fn owner_pid(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("pdf_")?;
    let mut fields = rest.split('_');
    let pid = fields.next()?;
    let seq = fields.next()?;
    let rand = fields.next()?;
    if fields.next().is_some() {
        return None;
    }
    for field in [pid, seq, rand] {
        if field.is_empty() || !field.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    match pid.parse::<u32>() {
        Ok(0) => None,
        Ok(pid) => Some(pid),
        Err(_) => None,
    }
}

/// Whether a process with this pid exists.
///
/// Signal 0 performs the permission and existence checks without delivering anything.
/// Only `ESRCH` — "no such process" — proves the owner is gone; every other outcome,
/// `EPERM` for a process belonging to another user included, is reported as alive so that
/// an uncertain answer can never authorize a deletion.
#[cfg(unix)]
fn is_pid_alive(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        // Larger than any pid this platform can produce, so the name is not ours to act on.
        return true;
    };
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

/// Without a way to ask whether a pid is alive there is no evidence a directory is
/// abandoned, so nothing is ever swept. rsrpp drives poppler's command line tools and is
/// exercised on Unix only; this keeps the crate compiling elsewhere without guessing.
#[cfg(not(unix))]
fn is_pid_alive(_pid: u32) -> bool {
    true
}

/// Removes the abandoned working directories directly under `root`.
///
/// `is_alive` is injected so the decision this function exists to make can be tested
/// without arranging for real processes to be alive or dead at the right moment.
///
/// A removal that fails is counted and otherwise ignored. The usual reason is the one
/// observed on this bug: killing rsrpp does not kill the poppler child it spawned, so an
/// orphaned `pdftocairo` keeps writing into the directory and `remove_dir_all` races it
/// (`Directory not empty`). The tempting fix — find the process holding the path and kill
/// it — is not one a library may take: matching processes by path means killing something
/// this process never started and cannot vouch for. Leaving it is harmless, because the
/// orphan finishes on its own and the next run sweeps the directory then.
pub(crate) fn sweep_dir(root: &Path, is_alive: &dyn Fn(u32) -> bool) -> SweepReport {
    let mut report = SweepReport::default();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::debug!(
                "Skipping cleanup of abandoned temporary directories in {}: {}",
                root.display(),
                e
            );
            return report;
        }
    };

    for entry in entries.flatten() {
        let path: PathBuf = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(pid) = owner_pid(name) else {
            continue;
        };
        // `symlink_metadata` does not follow links, so a symlink pointing at a real
        // directory is not a directory here and is left alone. Following one would make
        // the deletion land somewhere this module never inspected.
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => {}
            _ => continue,
        }
        if is_alive(pid) {
            report.skipped_live += 1;
            continue;
        }
        match std::fs::remove_dir_all(&path) {
            Ok(()) => {
                report.removed += 1;
                tracing::debug!(
                    "Removed temporary directory abandoned by process {}: {}",
                    pid,
                    path.display()
                );
            }
            Err(e) => {
                report.failed += 1;
                tracing::debug!(
                    "Left temporary directory {} for a later run: {}",
                    path.display(),
                    e
                );
            }
        }
    }
    report
}

/// Reclaims working directories left behind by earlier rsrpp processes that were killed
/// before they could clean up after themselves.
///
/// Runs at most once per process: the scan reads the whole system temp directory, and
/// repeating it for every `ParserConfig` would cost every parse in a batch the same walk
/// to find what the first one already removed.
///
/// Never fails and never reports. It is a background chore attached to a parse, not a
/// step of it — a temp directory that cannot be read, or a leftover that cannot be
/// removed, must not turn a parse that would have succeeded into a failure. Outcomes go
/// to the log at debug level.
///
/// `ParserConfig::new` calls this, so every user of the crate gets the recovery without
/// asking; `paper-export` and the other library callers take the same `SIGKILL` as the
/// CLI does. It is public so a caller that never builds a `ParserConfig`, or one that
/// wants the space back at startup rather than at first parse, can ask for it directly.
pub fn sweep_abandoned_temp_dirs() {
    static SWEPT: Once = Once::new();
    SWEPT.call_once(|| {
        let root = std::env::temp_dir();
        let report = sweep_dir(&root, &is_pid_alive);
        if report.removed > 0 || report.failed > 0 {
            tracing::info!(
                "Reclaimed {} of {} temporary directories abandoned by earlier runs in {} \
                 ({} still in use, {} to retry later)",
                report.removed,
                report.removed + report.failed,
                root.display(),
                report.skipped_live,
                report.failed
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;

    /// A sweep root of its own, so a test never reads or deletes anything in the real
    /// system temp directory alongside live parses.
    fn scratch_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "rsrpp_sweep_test_{}_{}_{}",
            std::process::id(),
            label,
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn make_dir(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("leftover.png"), b"x").unwrap();
        path
    }

    fn nothing_is_alive(_pid: u32) -> bool {
        false
    }

    fn everything_is_alive(_pid: u32) -> bool {
        true
    }

    #[test]
    fn test_a_dead_process_directory_is_removed() {
        let root = scratch_root("dead");
        let dir = make_dir(&root, "pdf_424242_0_83457");

        let report = sweep_dir(&root, &nothing_is_alive);

        assert!(!dir.exists(), "the abandoned directory was not reclaimed");
        assert_eq!(report.removed, 1);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn test_a_live_process_directory_is_left_alone() {
        // The reports campaign runs 8-12 parses at once; a sibling's working directory
        // is not this process's to delete.
        let root = scratch_root("live");
        let dir = make_dir(&root, "pdf_424242_0_83457");

        let report = sweep_dir(&root, &everything_is_alive);

        assert!(dir.is_dir(), "a running parse lost its working directory");
        assert_eq!(report.removed, 0);
        assert_eq!(report.skipped_live, 1);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn test_names_that_are_not_ours_are_never_touched() {
        // The system temp directory holds other applications' files, so anything that
        // does not match the format in full has to survive even when nothing is alive.
        let root = scratch_root("names");
        let strangers = [
            "pdf_424242_0",           // too few fields
            "pdf_424242_0_83457_tmp", // too many
            "pdf_abc_0_83457",        // pid is not a number
            "pdf_424242__83457",      // empty field
            "pdf_0_0_83457",          // pid 0 is never one of ours
            "pdf_-1_0_83457",         // sign is not a digit
            "pdf_424242_0_83457x",    // trailing text
            "pdf-424242_0_83457",     // wrong separator
            "pdf_",                   // prefix alone
            "pdfs",                   // unrelated neighbour
        ];
        for name in strangers {
            make_dir(&root, name);
        }
        // A *file* whose name does match: only directories are ours.
        std::fs::write(root.join("pdf_424242_1_11111"), b"x").unwrap();

        let report = sweep_dir(&root, &nothing_is_alive);

        assert_eq!(report, SweepReport::default(), "swept something not ours");
        for name in strangers {
            assert!(root.join(name).is_dir(), "{} was removed", name);
        }
        assert!(root.join("pdf_424242_1_11111").is_file());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn test_owner_pid_reads_a_real_config_directory() {
        // Pins this module to `ParserConfig::new`'s naming. If the format there changes,
        // the sweep silently stops recognizing its own directories, and this is the only
        // place that notices.
        let config = ParserConfig::new();
        let dir = Path::new(&config.pdf_path).parent().unwrap().to_path_buf();
        let name = dir.file_name().unwrap().to_str().unwrap();
        assert_eq!(owner_pid(name), Some(std::process::id()));
    }

    #[test]
    fn test_a_removal_that_fails_is_not_an_error() {
        // An orphaned poppler still writing into the directory makes `remove_dir_all`
        // fail; the sweep records it and moves on rather than propagating. A directory
        // that cannot be emptied stands in for that here.
        let root = scratch_root("failure");
        let dir = make_dir(&root, "pdf_424242_0_83457");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        }

        let report = sweep_dir(&root, &nothing_is_alive);

        if dir.exists() {
            assert_eq!(report.failed, 1, "a failed removal was not recorded");
            assert_eq!(report.removed, 0);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
        } else {
            // Running with enough privilege to ignore the mode: the removal simply worked.
            assert_eq!(report.removed, 1);
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn test_a_missing_root_is_not_an_error() {
        let root = scratch_root("missing");
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(sweep_dir(&root, &nothing_is_alive), SweepReport::default());
    }

    #[test]
    fn test_the_public_sweep_can_be_called_repeatedly() {
        // Guarded by `Once`, so the second call is a no-op rather than a second walk.
        sweep_abandoned_temp_dirs();
        sweep_abandoned_temp_dirs();
    }

    #[cfg(unix)]
    #[test]
    fn test_this_process_is_reported_as_alive() {
        assert!(is_pid_alive(std::process::id()));
    }
}
