//! `install`: put the built binary where the manifest expects it.
//!
//! The manifest launches `scripts/bin/herdr-splits` with no extension on every
//! platform. That is not cosmetic: on Windows herdr resolves a relative command
//! containing a path separator by joining it onto the plugin root and
//! canonicalising it (yielding a `\\?\` verbatim path). `CreateProcess` then
//! runs that exact path and does *not* append `.PATHEXT` variants for it, while
//! it happily runs an extensionless PE image. Both behaviours are verified, so
//! the destination name has no `.exe` suffix anywhere.

use std::path::{Path, PathBuf};

/// Where the binary goes, relative to the plugin root.
pub const DESTINATION: &str = "scripts/bin/herdr-splits";

/// Replace attempts for a destination another process may be holding open.
const RETRIES: u32 = 5;

/// Wait between attempts. Short: an install runs once, but a keypress may be
/// waiting for the file to be replaced.
const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(60);

/// Everything `install` needs to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOutcome {
    pub source: PathBuf,
    pub destination: PathBuf,
}

/// Locate the plugin root from the running executable's path.
///
/// Candidates, in order:
/// * `--root` (explicit, used by `make` and `plugin link` development),
/// * the checkout the binary was built from: `<root>/rust/target/<profile>/herdr-splits`,
/// * `<root>/scripts/bin/herdr-splits` — the installed location itself.
pub fn guess_root(exe: &Path, explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(root) = explicit {
        return Some(root.to_path_buf());
    }
    let exe = exe.canonicalize().unwrap_or_else(|_| exe.to_path_buf());
    // Walk up looking for the marker file. This is robust to any target dir
    // layout (`target/release`, `target/debug`, a workspace `target/...`).
    let mut current = exe.parent().map(Path::to_path_buf);
    while let Some(directory) = current {
        if directory.join("herdr-plugin.toml").is_file() {
            return Some(directory);
        }
        current = directory.parent().map(Path::to_path_buf);
    }
    None
}

/// Copy the running binary into `<root>/scripts/bin/herdr-splits`.
///
/// On Windows the image is written twice: once extensionless (what herdr's
/// `CreateProcess` call resolves) and once as `herdr-splits.exe` (what shells,
/// `make`, CI and humans resolve — PowerShell and `cmd` refuse to execute an
/// extensionless image). The extensionless copy is the authoritative one; the
/// `.exe` is a convenience mirror of the same bytes, so the two can never be
/// different builds.
pub fn install(exe: &Path, root: &Path) -> std::io::Result<InstallOutcome> {
    let destination = root.join(DESTINATION);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Read the image once, up front. The binary is a few hundred kilobytes, and
    // holding it in memory makes the write below a plain local file write
    // instead of a copy between two paths that may be on different volumes
    // (`CARGO_TARGET_DIR` on another drive is a supported setup).
    let image = std::fs::read(exe)?;
    // Write through a temp file in the destination directory so a concurrent
    // keypress never runs a half-written binary, and so the rename below can
    // never cross a volume boundary.
    let temp = destination.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temp, &image)?;
    // Unix needs the executable bit; Windows has no equivalent and the copy
    // already inherits read/execute for the owner.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755))?;
    }
    // Replacing the destination can fail transiently on Windows: something is
    // holding it open (an action running right now, an editor's file watcher,
    // or a scanner that just saw the copy land). A retry turns those into a
    // successful install instead of a failed `plugin install`.
    let mut last_error = None;
    for attempt in 0..RETRIES {
        match std::fs::rename(&temp, &destination) {
            Ok(()) => {
                last_error = None;
                break;
            }
            Err(err) => {
                // Not every platform lets a rename replace an existing file;
                // rewriting the destination in place is the fallback either
                // way. It still writes bytes we already hold, never re-reads
                // the source.
                match std::fs::write(&destination, &image) {
                    Ok(()) => {
                        last_error = None;
                        break;
                    }
                    Err(write_error) => {
                        last_error = Some(if attempt + 1 == RETRIES {
                            write_error
                        } else {
                            err
                        });
                        std::thread::sleep(RETRY_DELAY);
                    }
                }
            }
        }
    }
    let _ = std::fs::remove_file(&temp);
    if let Some(err) = last_error {
        return Err(err);
    }

    #[cfg(windows)]
    {
        // Refresh the shell-facing mirror from the bytes we just installed, so
        // it tracks the extensionless copy instead of drifting.
        let mirror = destination.with_extension("exe");
        for attempt in 0..RETRIES {
            match std::fs::write(&mirror, &image) {
                Ok(()) => break,
                Err(_) if attempt + 1 < RETRIES => std::thread::sleep(RETRY_DELAY),
                // Not fatal: herdr's own invocation does not need the mirror.
                Err(_) => {
                    let _ = std::fs::remove_file(&mirror);
                }
            }
        }
    }

    Ok(InstallOutcome {
        source: exe.to_path_buf(),
        destination,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        crate::testtmp::dir(tag)
    }

    #[test]
    fn explicit_root_wins() {
        let root = PathBuf::from("/tmp/explicit");
        assert_eq!(
            guess_root(Path::new("/opt/bin/herdr-splits"), Some(&root)),
            Some(root)
        );
    }

    #[test]
    fn finds_the_root_above_a_cargo_target_dir() {
        let root = temp_root("guess");
        std::fs::write(root.join("herdr-plugin.toml"), "id = \"herdr-splits\"").unwrap();
        let exe = root
            .join("rust")
            .join("target")
            .join("release")
            .join("herdr-splits");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"stub").unwrap();

        let found = guess_root(&exe, None);
        assert_eq!(
            found.as_deref().map(|path| path.canonicalize().unwrap()),
            Some(root.canonicalize().unwrap())
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn finds_the_root_below_the_installed_location() {
        let root = temp_root("guess-installed");
        std::fs::write(root.join("herdr-plugin.toml"), "id = \"herdr-splits\"").unwrap();
        let exe = root.join("scripts").join("bin").join("herdr-splits");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"stub").unwrap();

        let found = guess_root(&exe, None);
        assert_eq!(
            found.as_deref().map(|path| path.canonicalize().unwrap()),
            Some(root.canonicalize().unwrap())
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_tree_without_the_marker_has_no_root() {
        // The probe path must not sit under a plugin checkout: `guess_root`
        // walks up from the executable, so a directory inside this repository
        // would legitimately find *our* herdr-plugin.toml and the assertion
        // would test the checkout layout instead of the missing marker.
        let outside = std::env::temp_dir()
            .join("herdr-splits-no-marker-probe")
            .join("herdr-splits");
        assert_eq!(guess_root(&outside, None), None);
    }

    #[test]
    fn the_walk_stops_at_the_nearest_marker() {
        // Nested checkouts: the innermost marker wins, so a vendored copy does
        // not install into its parent.
        let outer = temp_root("nested-outer");
        let inner = outer.join("vendor").join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(outer.join("herdr-plugin.toml"), "id = \"outer\"").unwrap();
        std::fs::write(inner.join("herdr-plugin.toml"), "id = \"inner\"").unwrap();
        let exe = inner.join("target/release/herdr-splits");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"stub").unwrap();

        // `guess_root` canonicalises the executable, so compare canonical
        // paths rather than the raw ones (Windows may differ in case or have a
        // `\\?\` prefix).
        assert_eq!(
            guess_root(&exe, None).map(|path| path.canonicalize().unwrap()),
            Some(inner.canonicalize().unwrap()),
            "the nearest marker must win"
        );
        let _ = std::fs::remove_dir_all(&outer);
    }

    #[test]
    fn install_copies_to_an_extensionless_destination() {
        let root = temp_root("copy");
        let source = root.join("herdr-splits.exe");
        std::fs::write(&source, b"binary").unwrap();

        let outcome = install(&source, &root).unwrap();
        assert_eq!(outcome.destination, root.join(DESTINATION));
        assert_eq!(std::fs::read(&outcome.destination).unwrap(), b"binary");
        // No `.exe` and no leftover temp file.
        assert!(outcome
            .destination
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .eq("herdr-splits"));
        let leftovers: Vec<_> = std::fs::read_dir(root.join("scripts/bin"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("tmp-"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn install_sets_the_executable_bit_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_root("mode");
        let source = root.join("herdr-splits");
        std::fs::write(&source, b"binary").unwrap();
        let outcome = install(&source, &root).unwrap();
        let mode = std::fs::metadata(&outcome.destination)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn install_overwrites_an_older_binary() {
        let root = temp_root("overwrite");
        let source = root.join("herdr-splits");
        std::fs::write(&source, "new").unwrap();
        let destination = root.join(DESTINATION);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(&destination, "old").unwrap();

        install(&source, &root).unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Windows also gets a `.exe` mirror: shells refuse to execute an
    /// extensionless image even though `CreateProcess` (what herdr uses) does.
    /// The mirror must track the installed bytes, replacing any stale copy.
    #[cfg(windows)]
    #[test]
    fn install_writes_an_identical_exe_mirror_on_windows() {
        let root = temp_root("mirror");
        let source = root.join("built.exe");
        std::fs::write(&source, "new").unwrap();
        let destination = root.join(DESTINATION);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let mirror = destination.with_extension("exe");
        std::fs::write(&mirror, "stale").unwrap();

        install(&source, &root).unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"new");
        assert_eq!(std::fs::read(&mirror).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&root);
    }
}
