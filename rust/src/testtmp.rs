//! Scratch directories for tests.
//!
//! Tests must not depend on `std::env::temp_dir()`: on Windows that path can be
//! unwritable for a sandboxed or service-account process (and `%TEMP%` is
//! occasionally redirected to a per-session directory that a test binary cannot
//! create in). Writing under the crate's own `target/` is always allowed,
//! always cleaned up by `cargo clean`, and already ignored by git.
//!
//! Every directory is unique per process and per call, so tests can run in
//! parallel without sharing state.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Root under which test scratch directories are created.
///
/// Defaults to `<crate>/target/test-tmp`, which cargo already writes to, is
/// gitignored, and is removed by `cargo clean`. `HERDR_SPLITS_TEST_TMP`
/// overrides it for checkouts whose `target/` is not writable (read-only
/// mounts, sealed sandboxes, CI caches).
pub fn root() -> PathBuf {
    match std::env::var_os("HERDR_SPLITS_TEST_TMP") {
        Some(override_root) if !override_root.is_empty() => PathBuf::from(override_root),
        _ => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp"),
    }
}

/// A fresh, empty directory named `<tag>-<pid>-<serial>`.
pub fn dir(tag: &str) -> PathBuf {
    let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = root().join(format!("{tag}-{}-{serial}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path)
        .unwrap_or_else(|err| panic!("create test dir {}: {err}", path.display()));
    path
}

/// Path to a file inside a fresh scratch directory (the parent is created).
pub fn file(tag: &str, name: &str) -> PathBuf {
    dir(tag).join(name)
}

/// Serialises tests that read or write process environment variables.
///
/// The environment is process-global, so two tests that set `HERDR_*` at the
/// same time see each other's values. Cargo runs tests on threads by default;
/// hold this guard for the whole body of any test that touches the environment.
pub fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
