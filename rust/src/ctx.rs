//! Environment and path resolution.
//!
//! Every function here mirrors one in `scripts/herdr_common.py`; the shared
//! config file and the MRU state must live where the Neovim side and the bash
//! fallback expect them, or the three halves stop agreeing.

use std::env;
use std::path::{Path, PathBuf};

/// Resolve the herdr control socket/pipe path.
///
/// 1. `HERDR_SOCKET_PATH` / `HERDR_SOCKET` (injected into panes and plugin
///    actions; authoritative when present)
/// 2. Windows: `%APPDATA%\herdr\herdr.sock` (native Windows herdr home), but
///    only when that file exists
/// 3. `XDG_CONFIG_HOME` / `~/.config/herdr/herdr.sock` (unix layout)
pub fn socket_path() -> String {
    if let Some(explicit) = env_path(&["HERDR_SOCKET_PATH", "HERDR_SOCKET"]) {
        return explicit;
    }
    if cfg!(windows) {
        if let Some(appdata) = env_path(&["APPDATA"]) {
            let candidate = Path::new(&appdata).join("herdr").join("herdr.sock");
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    let base = match env_path(&["XDG_CONFIG_HOME"]) {
        Some(xdg) => PathBuf::from(xdg),
        None => home_dir().join(".config"),
    };
    base.join("herdr")
        .join("herdr.sock")
        .to_string_lossy()
        .into_owned()
}

/// Resolve the shared `herdr-splits.conf`, in the same order as the Neovim side
/// (`lua/herdr-splits/conf.lua`) and the bash fallback:
/// `HERDR_SPLITS_CONFIG`, then `HERDR_PLUGIN_CONFIG_DIR`, then the platform
/// default.
pub fn config_path() -> PathBuf {
    if let Some(explicit) = env_path(&["HERDR_SPLITS_CONFIG"]) {
        return PathBuf::from(explicit);
    }
    if let Some(plugin_dir) = env_path(&["HERDR_PLUGIN_CONFIG_DIR"]) {
        return Path::new(&plugin_dir).join("herdr-splits.conf");
    }
    let config_root = if cfg!(windows) {
        match env_path(&["APPDATA"]) {
            Some(appdata) => Path::new(&appdata).join("herdr"),
            None => home_dir().join(".config").join("herdr"),
        }
    } else {
        home_dir().join(".config").join("herdr")
    };
    config_root
        .join("plugins")
        .join("config")
        .join("herdr-splits")
        .join("herdr-splits.conf")
}

/// `$HOME`, or a best-effort fallback so a stripped-down environment still
/// produces a usable path instead of panicking.
fn home_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(profile) = env_path(&["USERPROFILE"]) {
            return PathBuf::from(profile);
        }
        if let (Some(drive), Some(path)) = (env_path(&["HOMEDRIVE"]), env_path(&["HOMEPATH"])) {
            return PathBuf::from(format!("{drive}{path}"));
        }
    }
    env_path(&["HOME"]).map_or_else(|| PathBuf::from("."), PathBuf::from)
}

/// First non-empty variable from `names`.
fn env_path(names: &[&str]) -> Option<String> {
    for name in names {
        if let Ok(value) = env::var(name) {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

/// The current pane id: herdr injects `HERDR_PANE_ID` into plugin actions (and
/// panes); the RPC fallback only runs when it is missing (manual invocation).
pub fn env_pane_id() -> Option<String> {
    env_path(&["HERDR_PANE_ID"])
}

/// `\\.\pipe\<path>`, unless the caller already supplied the prefix.
pub fn windows_pipe_name(path: &str) -> String {
    const PREFIX: &str = r"\\.\pipe\";
    if path.starts_with(PREFIX) {
        path.to_string()
    } else {
        format!("{PREFIX}{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The environment is process-global; serialise the tests that touch it.
    // Shared with every other test module that sets `HERDR_*` variables.
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::testtmp::env_lock()
    }

    struct EnvGuard {
        saved: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn set(vars: &[(&'static str, &str)]) -> EnvGuard {
            const NAMES: &[&str] = &[
                "HERDR_SOCKET_PATH",
                "HERDR_SOCKET",
                "HERDR_SPLITS_CONFIG",
                "HERDR_PLUGIN_CONFIG_DIR",
                "XDG_CONFIG_HOME",
                "APPDATA",
                "HERDR_PANE_ID",
            ];
            let saved = NAMES
                .iter()
                .map(|name| (*name, env::var(name).ok()))
                .collect();
            for name in NAMES {
                env::remove_var(name);
            }
            for (name, value) in vars {
                env::set_var(name, value);
            }
            EnvGuard { saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.saved {
                match value {
                    Some(value) => env::set_var(name, value),
                    None => env::remove_var(name),
                }
            }
        }
    }

    #[test]
    fn explicit_socket_path_wins() {
        let _lock = env_lock();
        let _guard = EnvGuard::set(&[("HERDR_SOCKET_PATH", "/tmp/explicit.sock")]);
        assert_eq!(socket_path(), "/tmp/explicit.sock");
    }

    #[test]
    fn socket_path_falls_back_to_the_short_variable() {
        let _lock = env_lock();
        let _guard = EnvGuard::set(&[("HERDR_SOCKET", "/tmp/short.sock")]);
        assert_eq!(socket_path(), "/tmp/short.sock");
    }

    #[test]
    fn config_path_precedence() {
        let _lock = env_lock();

        {
            let _guard = EnvGuard::set(&[
                ("HERDR_SPLITS_CONFIG", "/tmp/a.conf"),
                ("HERDR_PLUGIN_CONFIG_DIR", "/tmp/plugins"),
            ]);
            assert_eq!(config_path(), PathBuf::from("/tmp/a.conf"));
        }
        {
            let _guard = EnvGuard::set(&[("HERDR_PLUGIN_CONFIG_DIR", "/tmp/plugins")]);
            assert_eq!(
                config_path(),
                PathBuf::from("/tmp/plugins/herdr-splits.conf")
            );
        }
    }

    #[test]
    fn windows_pipe_prefix_is_added_once() {
        assert_eq!(
            windows_pipe_name(r"C:\tmp\herdr.sock"),
            r"\\.\pipe\C:\tmp\herdr.sock"
        );
        assert_eq!(
            windows_pipe_name(r"\\.\pipe\C:\tmp\herdr.sock"),
            r"\\.\pipe\C:\tmp\herdr.sock"
        );
    }

    #[test]
    fn env_pane_id_ignores_empty_values() {
        let _lock = env_lock();
        {
            let _guard = EnvGuard::set(&[("HERDR_PANE_ID", "")]);
            assert_eq!(env_pane_id(), None);
        }
        {
            let _guard = EnvGuard::set(&[("HERDR_PANE_ID", "w7:pN")]);
            assert_eq!(env_pane_id().as_deref(), Some("w7:pN"));
        }
    }
}
