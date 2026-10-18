//! Shared config file parsing (`herdr-splits.conf`).
//!
//! The conf is *generated* by the Neovim side and only ever read here, but a
//! legacy/hand-edited conf must still parse the way the bash fallback's
//! `sed | tail -n 1` and the Python `_conf_value` do: last value wins, `#`
//! starts a trailing comment, blank lines and full-line comments are skipped.

use std::fs;
use std::path::Path;

use crate::geometry::Direction;

/// Chord forwarded into the editor for a nav key, per direction.
pub fn default_nav_key(direction: Direction) -> &'static str {
    match direction {
        Direction::Left => "ctrl+h",
        Direction::Down => "ctrl+j",
        Direction::Up => "ctrl+k",
        Direction::Right => "ctrl+l",
    }
}

/// Chord forwarded into the editor for a resize key, per direction.
pub fn default_resize_key(direction: Direction) -> &'static str {
    match direction {
        Direction::Left => "alt+h",
        Direction::Down => "alt+j",
        Direction::Up => "alt+k",
        Direction::Right => "alt+l",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Nav,
    Resize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Chord forwarded into a Neovim pane for this keypress.
    pub key: String,
    /// Auto-unzoom when navigating away from a zoomed pane (`unzoom_on_nav`).
    pub unzoom: bool,
    /// Wrap at the pane boundary (`nav_at_edge`); `false` means `stop`.
    pub wrap_at_edge: bool,
    /// tmux-style pane selection (`most_recently_focused`).
    pub most_recently_focused: bool,
}

impl Config {
    /// Defaults with the direction's built-in chord filled in.
    pub fn defaults(kind: Kind, direction: Direction) -> Config {
        Config {
            key: match kind {
                Kind::Nav => default_nav_key(direction).to_string(),
                Kind::Resize => default_resize_key(direction).to_string(),
            },
            unzoom: true,
            wrap_at_edge: true,
            most_recently_focused: false,
        }
    }

    /// Read the shared conf. A missing or unreadable file is not an error: the
    /// defaults are authoritative then (the file is generated on `setup()`).
    pub fn load(path: &Path, kind: Kind, direction: Direction) -> Config {
        let mut config = Config::defaults(kind, direction);
        let Ok(text) = fs::read(path) else {
            return config;
        };
        // `errors="ignore"` on the Python side: decode lossily rather than
        // rejecting a file with a stray byte.
        let text = String::from_utf8_lossy(&text);
        config.apply(&text, kind, direction);
        config
    }

    fn apply(&mut self, text: &str, kind: Kind, direction: Direction) {
        let key_name = match kind {
            Kind::Nav => format!("nav_key_{}", direction.as_str()),
            Kind::Resize => format!("resize_key_{}", direction.as_str()),
        };
        if let Some(value) = conf_value(text, &key_name) {
            self.key = value;
        }
        if let Some(value) = conf_value(text, "unzoom_on_nav") {
            self.unzoom = !value.eq_ignore_ascii_case("false");
        }
        if let Some(value) = conf_value(text, "nav_at_edge") {
            self.wrap_at_edge = value != "stop";
        }
        if let Some(value) = conf_value(text, "most_recently_focused") {
            self.most_recently_focused = value.eq_ignore_ascii_case("true");
        }
    }
}

/// Last `key = value` in the conf text; `#` starts a trailing comment and an
/// empty value is treated as "unset" (matching the generated-file reader).
pub fn conf_value(text: &str, key: &str) -> Option<String> {
    let mut found = None;
    for line in text.lines() {
        let stripped = line.trim();
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        let Some((name, rest)) = stripped.split_once('=') else {
            continue;
        };
        if name.trim() != key {
            continue;
        }
        let value = rest.split('#').next().unwrap_or("").trim();
        if !value.is_empty() {
            found = Some(value.to_string());
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// The real generated conf from this machine's herdr config dir, minus the
    /// comments (fixtures stay readable, the header is not semantically load).
    const GENERATED: &str = "\
nav_key_left=ctrl+h
nav_key_down=ctrl+j
nav_key_up=ctrl+k
nav_key_right=ctrl+l
resize_key_left=alt+h
resize_key_down=alt+j
resize_key_up=alt+k
resize_key_right=alt+l
unzoom_on_nav=false
nav_at_edge=wrap
most_recently_focused=true
";

    #[test]
    fn defaults_match_the_documented_chords() {
        let config = Config::defaults(Kind::Nav, Direction::Left);
        assert_eq!(config.key, "ctrl+h");
        assert!(config.unzoom);
        assert!(config.wrap_at_edge);
        assert!(!config.most_recently_focused);

        assert_eq!(
            Config::defaults(Kind::Resize, Direction::Right).key,
            "alt+l"
        );
    }

    #[test]
    fn reads_every_managed_value() {
        let mut config = Config::defaults(Kind::Nav, Direction::Left);
        config.apply(GENERATED, Kind::Nav, Direction::Left);
        assert_eq!(config.key, "ctrl+h");
        assert!(!config.unzoom);
        assert!(config.wrap_at_edge);
        assert!(config.most_recently_focused);
    }

    #[test]
    fn direction_selects_the_matching_key() {
        for (direction, expected) in [
            (Direction::Left, "ctrl+h"),
            (Direction::Down, "ctrl+j"),
            (Direction::Up, "ctrl+k"),
            (Direction::Right, "ctrl+l"),
        ] {
            let mut config = Config::defaults(Kind::Nav, direction);
            config.apply(GENERATED, Kind::Nav, direction);
            assert_eq!(config.key, expected);
        }
        let mut resize = Config::defaults(Kind::Resize, Direction::Up);
        resize.apply(GENERATED, Kind::Resize, Direction::Up);
        assert_eq!(resize.key, "alt+k");
    }

    #[test]
    fn nav_at_edge_stop_is_honoured() {
        let mut config = Config::defaults(Kind::Nav, Direction::Left);
        config.apply("nav_at_edge=stop\n", Kind::Nav, Direction::Left);
        assert!(!config.wrap_at_edge);
    }

    #[test]
    fn most_recently_focused_only_accepts_true() {
        for (text, expected) in [
            ("most_recently_focused=true", true),
            ("most_recently_focused=True", true),
            ("most_recently_focused=yes", false),
            ("most_recently_focused=false", false),
        ] {
            let mut config = Config::defaults(Kind::Nav, Direction::Left);
            config.apply(text, Kind::Nav, Direction::Left);
            assert_eq!(config.most_recently_focused, expected, "{text}");
        }
    }

    #[test]
    fn last_occurrence_wins_and_comments_are_stripped() {
        let text = "# comment\nnav_key_left = alt+h  # trailing\nnav_key_left=ctrl+x\n\n";
        assert_eq!(conf_value(text, "nav_key_left").as_deref(), Some("ctrl+x"));
        assert_eq!(
            conf_value("nav_key_left=alt+h # trailing", "nav_key_left").as_deref(),
            Some("alt+h")
        );
    }

    #[test]
    fn empty_values_do_not_override_defaults() {
        assert_eq!(conf_value("nav_key_left=\n", "nav_key_left"), None);
        assert_eq!(conf_value("nav_key_left=   # note\n", "nav_key_left"), None);
        assert_eq!(conf_value("", "nav_key_left"), None);
    }

    #[test]
    fn a_missing_file_keeps_the_defaults() {
        let path = crate::testtmp::dir("conf-missing").join("herdr-splits.conf");
        assert!(!path.exists());
        let config = Config::load(&path, Kind::Nav, Direction::Down);
        assert_eq!(config, Config::defaults(Kind::Nav, Direction::Down));
    }

    #[test]
    fn a_real_file_round_trips_through_load() {
        let path = crate::testtmp::file("conf-roundtrip", "herdr-splits.conf");
        {
            let mut handle = fs::File::create(&path).unwrap();
            handle.write_all(GENERATED.as_bytes()).unwrap();
        }
        let config = Config::load(&path, Kind::Nav, Direction::Right);
        let _ = fs::remove_file(&path);
        assert_eq!(config.key, "ctrl+l");
        assert!(!config.unzoom);
        assert!(config.most_recently_focused);
    }
}
