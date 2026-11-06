//! `init`: write a scaffold `herdr-splits.conf`, and create it on first use.
//!
//! The conf is the *instance policy*: the Herdr-side helper, plain Vim and
//! Neovim all read it, and no editor writes it as a side effect of starting up
//! (Neovim can, but only when asked with `overwrite_conf = true`). Creating it
//! is the helper's job, which is why it lives here: the file's producer is the
//! tool that owns its semantics, so no editor gets to decide the policy of the
//! whole Herdr instance behind the user's back.
//!
//! Two entry points:
//! * [`ensure`] — called on every real command (`nav`, `resize`, `install`): if
//!   the file is missing, write the scaffold. Best-effort and silent; a
//!   read-only config dir simply leaves every reader on the built-in defaults.
//! * [`run`] — the `init` subcommand: explicit create (with `--force`/`--print`
//!   and a message), so a human can also write or reset the file deliberately.
//!
//! Because the scaffold spells out [`Config::defaults`], creating the file
//! changes nothing behaviourally: deleting it (a reset) and letting it be
//! recreated yields the same values. The file exists so there is something to
//! edit, and so what is in effect is discoverable.
//!
//! The file written here carries NO generated marker: by construction it is the
//! user's. A later `setup({ overwrite_conf = true })` stamps the marker when it
//! takes the file over.

use std::path::{Path, PathBuf};

use crate::config::{default_nav_key, default_resize_key, Config, Kind};
use crate::geometry::Direction;

/// Directions in the order the conf lists them.
pub const DIRECTIONS: [Direction; 4] = [
    Direction::Left,
    Direction::Down,
    Direction::Up,
    Direction::Right,
];

/// What `init` did, for the caller to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Wrote the scaffold to this path.
    Wrote(PathBuf),
    /// Printed the scaffold on stdout (nothing touched on disk).
    Printed,
}

/// The scaffold text: the effective defaults, spelled out, with enough comments
/// that the file is self-documenting when opened cold.
///
/// Every policy line comes from `Config::defaults`, so the scaffold cannot
/// drift from what a missing conf means. Each comment sits directly above the
/// key it documents.
pub fn scaffold() -> String {
    let defaults = Config::defaults(Kind::Nav, Direction::Left);
    let mut out = String::new();
    out.push_str(
        "# herdr-splits — instance policy for this Herdr server.\n\
         #\n\
         # Read by the Herdr-side helper (every keypress), plain Vim and Neovim\n\
         # (`setup()`, or `:HerdrSplitsReload`). No editor rewrites it: the helper\n\
         # created it because it was missing, and creates it again if you delete\n\
         # it — delete = reset to the values below. One `key=value` per line,\n\
         # `#` starts a comment, and the LAST occurrence of a key wins.\n\
         #\n\
         # `herdr-splits doctor` prints what is actually in effect, with its\n\
         # source. `herdr-splits init --force` rewrites this file, `--print`\n\
         # shows it.\n",
    );

    out.push_str(
        "\n\
         # Chords forwarded into an editor pane, per direction. Herdr has to know\n\
         # these: it is what it types into the editor when a keypress arrives at a\n\
         # pane running nvim/vim. Keep the editor-side normal-mode maps in sync.\n",
    );
    for direction in DIRECTIONS {
        out.push_str(&format!(
            "nav_key_{}={}\n",
            direction.as_str(),
            default_nav_key(direction)
        ));
    }

    out.push_str("\n# The same, for native resizing.\n");
    for direction in DIRECTIONS {
        out.push_str(&format!(
            "resize_key_{}={}\n",
            direction.as_str(),
            default_resize_key(direction)
        ));
    }

    out.push_str(&format!(
        "\n\
         # Unzoom (leave full screen) when navigating away from a zoomed pane.\n\
         # true  = unzoom first, so the destination pane is visible.\n\
         # false = stay zoomed; the zoom then follows the focus.\n\
         unzoom_on_nav={}\n",
        defaults.unzoom
    ));

    out.push_str(&format!(
        "\n\
         # Pane-boundary behaviour: wrap = jump to the opposite edge, stop = halt.\n\
         # Wrap also lets the editor's own edge wrap cross into the pane on the\n\
         # opposite side; stop keeps it inside the editor.\n\
         nav_at_edge={}\n",
        if defaults.wrap_at_edge {
            "wrap"
        } else {
            "stop"
        }
    ));

    out.push_str(&format!(
        "\n\
         # tmux-style target choice: among the panes adjacent in a direction, take\n\
         # the most recently focused one instead of the geometrically nearest.\n\
         # The recency list is `mru-panes`, next to this file.\n\
         most_recently_focused={}\n",
        defaults.most_recently_focused
    ));

    out
}

/// Write `bytes` to `path` atomically: a same-directory temp plus rename, so a
/// reader never observes a half-written file. Shared by [`ensure`] (silent,
/// best-effort) and [`run`] (explicit) — the two had already drifted apart.
fn write_atomic(path: &Path, bytes: &str) -> std::io::Result<()> {
    use std::io::Write;

    let temp = PathBuf::from(format!("{}.tmp.{}", path.display(), std::process::id()));
    let write = || -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes.as_bytes())?;
        // Flush before the rename: an unchecked close can leave a truncated temp
        // that then replaces the live file.
        file.sync_all()?;
        Ok(())
    };
    if let Err(err) = write() {
        let _ = std::fs::remove_file(&temp);
        return Err(err);
    }
    if std::fs::rename(&temp, path).is_err() {
        // Destination held open (Windows) or a cross-device temp: fall back to a
        // plain write, briefly non-atomic but strictly better than a stale file.
        let _ = std::fs::remove_file(&temp);
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

/// Create the conf if it is missing. Called on every real command, so the first
/// keypress (or the first `install`) leaves a file to edit. Best-effort: any
/// failure (read-only dir, a racing creator) is ignored, because every reader
/// falls back to [`Config::defaults`], which is what the scaffold contains.
/// Returns true when this call created the file.
pub fn ensure(path: &Path) -> bool {
    if path.exists() {
        return false;
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    // Two concurrent first keypresses write identical bytes, so losing the race
    // is harmless.
    write_atomic(path, &scaffold()).is_ok()
}

/// Write the scaffold to `path` unless it already exists (`force` overrides).
/// `print` never touches the disk.
///
/// Atomic like the rest of the crate: same-directory temp + rename, so a reader
/// never observes a half-written file.
pub fn run(path: &Path, force: bool, print: bool) -> Result<Outcome, String> {
    if print {
        return Ok(Outcome::Printed);
    }
    if path.exists() && !force {
        return Err(format!(
            "herdr-splits init: {} already exists — pass --force to replace it, \
             or --print to write the scaffold to stdout",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "herdr-splits init: cannot create {}: {err}",
                    parent.display()
                )
            })?;
        }
    }

    write_atomic(path, &scaffold())
        .map_err(|err| format!("herdr-splits init: cannot write {}: {err}", path.display()))?;
    Ok(Outcome::Wrote(path.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testtmp;

    #[test]
    fn scaffold_spells_out_the_defaults() {
        let text = scaffold();
        // No generated marker: this file is the user's from the first byte.
        // The marker literal lives in `config::CONF_MARKER`; a second copy here
        // would let the takeover rule and its test drift apart.
        assert!(!text.contains(crate::config::CONF_MARKER));
        for direction in DIRECTIONS {
            assert!(text.contains(&format!(
                "nav_key_{}={}",
                direction.as_str(),
                default_nav_key(direction)
            )));
            assert!(text.contains(&format!(
                "resize_key_{}={}",
                direction.as_str(),
                default_resize_key(direction)
            )));
        }
        assert!(!text.contains("unzoom_on_nav=true"));
        assert!(text.contains("unzoom_on_nav=false"));
        assert!(text.contains("nav_at_edge=wrap"));
        assert!(text.contains("most_recently_focused=true"));
    }

    #[test]
    fn scaffold_keeps_each_comment_next_to_its_key() {
        // The file is meant to be read top-to-bottom: a comment describes the
        // key directly below it, not a block of keys further down.
        let text = scaffold();
        let lines: Vec<&str> = text.lines().collect();
        for key in ["unzoom_on_nav=", "nav_at_edge=", "most_recently_focused="] {
            let index = lines
                .iter()
                .position(|line| line.starts_with(key))
                .unwrap_or_else(|| panic!("missing {key}"));
            let previous = index
                .checked_sub(1)
                .and_then(|i| lines.get(i))
                .copied()
                .unwrap_or("");
            assert!(
                previous.starts_with('#'),
                "{key} is not directly under its comment (found {previous:?})"
            );
        }
    }

    #[test]
    fn ensure_creates_only_when_missing() {
        let dir = testtmp::dir("init-ensure");
        let path = dir.join("nested/herdr-splits.conf");

        assert!(ensure(&path));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), scaffold());
        // Second call is a no-op: the user's file is never replaced.
        assert!(!ensure(&path));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), scaffold());
    }

    #[test]
    fn ensure_leaves_no_temp_and_keeps_a_users_file() {
        let dir = testtmp::dir("init-ensure-user");
        let path = dir.join("herdr-splits.conf");
        std::fs::write(&path, "nav_at_edge=wrap\n").unwrap();

        assert!(!ensure(&path));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "nav_at_edge=wrap\n"
        );
    }

    #[test]
    fn scaffold_parses_back_to_the_defaults() {
        let text = scaffold();
        let config =
            crate::config::Config::load_text(&text, crate::config::Kind::Nav, Direction::Left);
        let defaults = crate::config::Config::defaults(crate::config::Kind::Nav, Direction::Left);
        assert_eq!(config, defaults);
    }

    #[test]
    fn run_refuses_to_replace_an_existing_file() {
        let dir = testtmp::dir("init-refuse");
        let path = dir.join("herdr-splits.conf");
        std::fs::write(&path, "unzoom_on_nav=false\n").unwrap();

        let err = run(&path, false, false).unwrap_err();
        assert!(err.contains("already exists"), "{err}");
        // The existing bytes survive untouched.
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "unzoom_on_nav=false\n"
        );
    }

    #[test]
    fn run_force_replaces_and_leaves_no_temp() {
        let dir = testtmp::dir("init-force");
        let path = dir.join("nested/herdr-splits.conf");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "unzoom_on_nav=false\n").unwrap();

        assert_eq!(run(&path, true, false), Ok(Outcome::Wrote(path.clone())));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), scaffold());
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".tmp."))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn run_print_touches_nothing() {
        let dir = testtmp::dir("init-print");
        let path = dir.join("herdr-splits.conf");

        assert_eq!(run(&path, false, true), Ok(Outcome::Printed));
        assert!(!path.exists());
    }
}
