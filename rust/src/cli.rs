//! Argument parsing.
//!
//! The shipped entry points are two actions the manifest calls with a fixed
//! argv, so the parser only has to be exact, not clever:
//!
//! ```text
//! herdr-splits nav [--pane-only] <left|down|up|right>
//! herdr-splits resize <left|down|up|right> [amount]
//! herdr-splits install [--root <plugin-root>]
//! herdr-splits doctor
//! herdr-splits --version
//! ```
//!
//! Two options exist for tests and diagnostics rather than for users:
//! `--replay <file>` answers RPC calls from a fixture instead of the socket,
//! and `--trace <file>` appends every call as `method<TAB>params<TAB>ok|error`.

use std::path::PathBuf;

use crate::geometry::Direction;

/// Default resize step, matching `config.default_amount` on the Neovim side.
pub const DEFAULT_AMOUNT: f64 = 0.03;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Nav {
        pane_only: bool,
        direction: Direction,
    },
    Resize {
        direction: Direction,
        amount: f64,
    },
    Install {
        root: Option<PathBuf>,
    },
    Doctor,
    Version,
    Help,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub command: Command,
    /// Hidden test hook: replay RPC replies from this fixture file.
    pub replay: Option<PathBuf>,
    /// Diagnostics: append every RPC call to this file.
    pub trace: Option<PathBuf>,
}

/// Parse failure; `message` is printed to stderr and the process exits 2.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageError {
    pub message: String,
}

impl UsageError {
    fn new(message: impl Into<String>) -> UsageError {
        UsageError {
            message: message.into(),
        }
    }
}

const USAGE: &str = "usage: herdr-splits <nav|resize|install|doctor> ...";

/// Full help text (`--help`), kept short: the README owns the long version.
pub fn help_text() -> String {
    format!(
        "\
herdr-splits {version} — Herdr-side navigation/resize helper for herdr-splits.nvim

USAGE:
{USAGE}

COMMANDS:
  nav [--pane-only] <left|down|up|right>   navigate panes/editors
  resize <left|down|up|right> [amount]     resize the focused pane (default {DEFAULT_AMOUNT})
  install [--root <plugin-root>]           copy this binary into <root>/scripts/bin
  doctor                                   print resolved paths and probe the server

OPTIONS:
  --replay <file>   answer RPC calls from a fixture (tests)
  --trace <file>    append every RPC call to <file>
  --version, -V     print the version
  -h, --help        print this help
",
        version = env!("CARGO_PKG_VERSION"),
    )
}

/// Parse `args` (without the program name).
pub fn parse(args: &[String]) -> Result<Options, UsageError> {
    let mut replay = None;
    let mut trace = None;
    let mut positional: Vec<String> = Vec::new();

    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--replay" | "--trace" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(UsageError::new(format!("{arg} needs a file argument")));
                };
                let path = PathBuf::from(value);
                if arg == "--replay" {
                    replay = Some(path);
                } else {
                    trace = Some(path);
                }
                index += 2;
            }
            "-h" | "--help" | "help" => {
                return Ok(Options {
                    command: Command::Help,
                    replay,
                    trace,
                })
            }
            "-V" | "--version" | "version" => {
                return Ok(Options {
                    command: Command::Version,
                    replay,
                    trace,
                })
            }
            // Only these begin with a dash on purpose; everything else is
            // positional so a stray flag becomes a usage error below.
            "--pane-only" => {
                positional.push(arg.to_string());
                index += 1;
            }
            // `--root` belongs to the `install` subcommand: keep it (and its
            // value) positional so the subcommand parses it, and leave the
            // unknown-flag check to the subcommand's own loop.
            other if other == "--root" || other.starts_with("--root=") => {
                positional.push(other.to_string());
                index += 1;
            }
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(UsageError::new(format!("unknown option: {other}")));
            }
            _ => {
                positional.push(arg.to_string());
                index += 1;
            }
        }
    }

    let Some(subcommand) = positional.first() else {
        return Err(UsageError::new(USAGE.to_string()));
    };

    let command = match subcommand.as_str() {
        "nav" => {
            let mut pane_only = false;
            let mut direction = None;
            for arg in &positional[1..] {
                if arg == "--pane-only" {
                    pane_only = true;
                } else if direction.is_none() {
                    direction = Direction::parse(arg);
                    if direction.is_none() {
                        return Err(UsageError::new(
                            "usage: herdr-splits nav [--pane-only] <left|down|up|right>",
                        ));
                    }
                } else {
                    return Err(UsageError::new(
                        "usage: herdr-splits nav [--pane-only] <left|down|up|right>",
                    ));
                }
            }
            let Some(direction) = direction else {
                return Err(UsageError::new(
                    "usage: herdr-splits nav [--pane-only] <left|down|up|right>",
                ));
            };
            Command::Nav {
                pane_only,
                direction,
            }
        }
        "resize" => {
            let rest = &positional[1..];
            if rest.is_empty() || rest.len() > 2 {
                return Err(UsageError::new(
                    "usage: herdr-splits resize <left|down|up|right> [amount]",
                ));
            }
            let Some(direction) = Direction::parse(&rest[0]) else {
                return Err(UsageError::new(
                    "usage: herdr-splits resize <left|down|up|right> [amount]",
                ));
            };
            let amount = match rest.get(1) {
                None => DEFAULT_AMOUNT,
                Some(text) => parse_amount(text)?,
            };
            Command::Resize { direction, amount }
        }
        "install" => {
            let mut root = None;
            let mut rest = positional[1..].iter();
            while let Some(arg) = rest.next() {
                match arg.as_str() {
                    "--root" => {
                        let Some(value) = rest.next() else {
                            return Err(UsageError::new("--root needs a path argument"));
                        };
                        root = Some(PathBuf::from(value));
                    }
                    other if other.starts_with("--root=") => {
                        root = Some(PathBuf::from(other.trim_start_matches("--root=")));
                    }
                    other => {
                        return Err(UsageError::new(format!(
                            "usage: herdr-splits install [--root <plugin-root>] \
                             (unexpected {other:?})"
                        )))
                    }
                }
            }
            Command::Install { root }
        }
        "doctor" => {
            if positional.len() > 1 {
                return Err(UsageError::new("usage: herdr-splits doctor"));
            }
            Command::Doctor
        }
        other => {
            return Err(UsageError::new(format!(
                "unknown command: {other}\n{USAGE}"
            )))
        }
    };

    Ok(Options {
        command,
        replay,
        trace,
    })
}

/// `float()` semantics for the resize argument, minus the traceback: Python
/// accepted trailing spaces and exponents, which `str::parse` already does
/// after trimming.
fn parse_amount(text: &str) -> Result<f64, UsageError> {
    let trimmed = text.trim();
    match trimmed.parse::<f64>() {
        Ok(value) if value.is_finite() => Ok(value),
        _ => Err(UsageError::new(format!(
            "invalid amount: '{text}'\nusage: herdr-splits resize <left|down|up|right> [amount]"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn parse_ok(values: &[&str]) -> Options {
        parse(&args(values)).expect("should parse")
    }

    fn parse_err(values: &[&str]) -> String {
        let error = parse(&args(values)).expect_err("should fail");
        error.message
    }

    fn contains(text: &str, needle: &str) -> bool {
        text.contains(needle)
    }

    #[test]
    fn parses_nav_for_every_direction() {
        for (text, direction) in [
            ("left", Direction::Left),
            ("down", Direction::Down),
            ("up", Direction::Up),
            ("right", Direction::Right),
        ] {
            assert_eq!(
                parse_ok(&["nav", text]).command,
                Command::Nav {
                    pane_only: false,
                    direction
                }
            );
        }
    }

    #[test]
    fn pane_only_is_accepted_before_or_after_the_direction() {
        for values in [
            &["nav", "--pane-only", "left"][..],
            &["nav", "left", "--pane-only"][..],
        ] {
            assert_eq!(
                parse_ok(values).command,
                Command::Nav {
                    pane_only: true,
                    direction: Direction::Left
                }
            );
        }
    }

    #[test]
    fn nav_without_a_direction_is_a_usage_error() {
        assert!(contains(&parse_err(&["nav"]), "<left|down|up|right>"));
        assert!(contains(
            &parse_err(&["nav", "--pane-only"]),
            "<left|down|up|right>"
        ));
        assert!(contains(
            &parse_err(&["nav", "sideways"]),
            "<left|down|up|right>"
        ));
        assert!(contains(
            &parse_err(&["nav", "left", "right"]),
            "<left|down|up|right>"
        ));
    }

    #[test]
    fn resize_defaults_to_three_percent() {
        assert_eq!(
            parse_ok(&["resize", "left"]).command,
            Command::Resize {
                direction: Direction::Left,
                amount: DEFAULT_AMOUNT
            }
        );
    }

    #[test]
    fn resize_accepts_an_explicit_amount() {
        assert_eq!(
            parse_ok(&["resize", "up", "0.1"]).command,
            Command::Resize {
                direction: Direction::Up,
                amount: 0.1
            }
        );
        assert_eq!(
            parse_ok(&["resize", "up", " 5 "]).command,
            Command::Resize {
                direction: Direction::Up,
                amount: 5.0
            }
        );
    }

    #[test]
    fn resize_rejects_bad_amounts_and_arities() {
        assert!(contains(&parse_err(&["resize"]), "<left|down|up|right>"));
        assert!(contains(
            &parse_err(&["resize", "left", "abc"]),
            "invalid amount"
        ));
        assert!(contains(
            &parse_err(&["resize", "left", "0.1", "extra"]),
            "usage"
        ));
        assert!(contains(&parse_err(&["resize", "sideways"]), "usage"));
        assert!(contains(
            &parse_err(&["resize", "left", "inf"]),
            "invalid amount"
        ));
    }

    #[test]
    fn install_takes_an_optional_root() {
        assert_eq!(
            parse_ok(&["install"]).command,
            Command::Install { root: None }
        );
        assert_eq!(
            parse_ok(&["install", "--root", "/tmp/plugin"]).command,
            Command::Install {
                root: Some(PathBuf::from("/tmp/plugin"))
            }
        );
        assert_eq!(
            parse_ok(&["install", "--root=/tmp/plugin"]).command,
            Command::Install {
                root: Some(PathBuf::from("/tmp/plugin"))
            }
        );
        assert!(contains(&parse_err(&["install", "--root"]), "--root"));
        assert!(contains(
            &parse_err(&["install", "--bogus"]),
            "unknown option"
        ));
    }

    #[test]
    fn doctor_version_and_help() {
        assert_eq!(parse_ok(&["doctor"]).command, Command::Doctor);
        assert_eq!(parse_ok(&["--version"]).command, Command::Version);
        assert_eq!(parse_ok(&["-V"]).command, Command::Version);
        assert_eq!(parse_ok(&["--help"]).command, Command::Help);
        assert_eq!(parse_ok(&["-h"]).command, Command::Help);
        assert!(contains(&parse_err(&["doctor", "extra"]), "usage"));
    }

    #[test]
    fn no_subcommand_is_a_usage_error() {
        assert!(contains(&parse_err(&[]), "usage: herdr-splits"));
    }

    #[test]
    fn unknown_command_is_a_usage_error() {
        assert!(contains(
            &parse_err(&["navigate", "left"]),
            "unknown command"
        ));
    }

    #[test]
    fn hidden_options_are_extracted_from_anywhere() {
        let options = parse_ok(&[
            "--trace",
            "/tmp/t.log",
            "nav",
            "left",
            "--replay",
            "/tmp/fixture.jsonl",
        ]);
        assert_eq!(options.trace, Some(PathBuf::from("/tmp/t.log")));
        assert_eq!(options.replay, Some(PathBuf::from("/tmp/fixture.jsonl")));
        assert_eq!(
            options.command,
            Command::Nav {
                pane_only: false,
                direction: Direction::Left
            }
        );
    }

    #[test]
    fn hidden_options_need_a_value() {
        assert!(contains(
            &parse_err(&["nav", "left", "--replay"]),
            "--replay"
        ));
    }

    #[test]
    fn unknown_flags_are_rejected() {
        assert!(contains(
            &parse_err(&["nav", "left", "--bogus"]),
            "unknown option"
        ));
    }
}
