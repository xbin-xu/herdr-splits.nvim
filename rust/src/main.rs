//! Herdr-side navigation/resize helper for herdr-splits.nvim.
//!
//! This binary replaces `scripts/herdr-nav.py` and `scripts/herdr-resize.py`
//! (and the shared `herdr_common.py`) on the Herdr side. Those scripts are
//! re-executed for every keypress from the plugin manifest, so on Windows the
//! interpreter start-up alone cost ~30–45 ms per keypress even after the slim
//! rewrite — more than the RPC work itself. A static binary starts in ~10 ms.
//!
//! It speaks the same NDJSON RPC to the same socket, in the same order, with
//! the same exit codes. The Python scripts stay in the tree as the reference
//! implementation and as the no-toolchain fallback.
//!
//! Exit codes (identical to the Python pair):
//!
//! * `0` — done, including "at the edge, nothing to do";
//! * `1` — herdr unreachable, no current pane, or an RPC call failed;
//! * `2` — usage error.

mod cli;
mod config;
mod ctx;
mod doctor;
mod editor;
mod geometry;
mod install;
mod json;
mod mru;
mod nav;
mod panes;
mod resize;
mod rpc;
#[cfg(test)]
mod testtmp;
mod vimname;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cli::{Command, Options};
use panes::Panes;
use rpc::{Client, ReplayClient, RpcError, SocketClient, TraceClient};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match cli::parse(&args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{}", error.message);
            return ExitCode::from(2);
        }
    };

    match run(&options) {
        Ok(code) => code,
        Err(Failure { message, code }) => {
            eprintln!("{message}");
            ExitCode::from(code)
        }
    }
}

/// A path that ends the process with an exit code, after the caller has
/// printed `message`.
struct Failure {
    message: String,
    code: u8,
}

impl Failure {
    fn new(message: impl Into<String>, code: u8) -> Failure {
        Failure {
            message: message.into(),
            code,
        }
    }
}

fn run(options: &Options) -> Result<ExitCode, Failure> {
    match &options.command {
        Command::Help => {
            print!("{}", cli::help_text());
            Ok(ExitCode::SUCCESS)
        }
        Command::Version => {
            println!("herdr-splits {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        Command::Install { root } => run_install(root.clone()),
        Command::Doctor => run_doctor(options),
        Command::Nav {
            pane_only,
            direction,
        } => with_client(options, |client| {
            let (config, config_path) = nav::load_config(*direction);
            let mut panes = Panes::new(client);
            let Some(current) = nav::current_pane(&mut panes, ctx::env_pane_id())
                .map_err(|err| connect_failure("nav", &err))?
            else {
                return Err(Failure::new(
                    "herdr-splits nav: could not determine current pane",
                    1,
                ));
            };
            nav::run(
                &mut panes,
                &config,
                &config_path,
                &current.pane_id,
                *direction,
                *pane_only,
            )
            .map_err(|err| call_failure("nav", &err))?;
            Ok(ExitCode::SUCCESS)
        }),
        Command::Resize { direction, amount } => with_client(options, |client| {
            let config = resize::load_config(*direction);
            let mut panes = Panes::new(client);
            let Some(current) = nav::current_pane(&mut panes, ctx::env_pane_id())
                .map_err(|err| connect_failure("resize", &err))?
            else {
                return Err(Failure::new(
                    "herdr-splits resize: could not determine current pane",
                    1,
                ));
            };
            resize::run(&mut panes, &config, &current.pane_id, *direction, *amount)
                .map_err(|err| call_failure("resize", &err))?;
            Ok(ExitCode::SUCCESS)
        }),
    }
}

/// Build the client (`--replay` for tests, otherwise the real socket), run
/// `body`, and add `--trace` output around it.
fn with_client<F>(options: &Options, body: F) -> Result<ExitCode, Failure>
where
    F: FnOnce(&mut dyn Client) -> Result<ExitCode, Failure>,
{
    let trace = options.trace.clone();
    if let Some(path) = options.replay.clone() {
        let fixture = std::fs::read_to_string(&path).map_err(|err| {
            Failure::new(
                format!(
                    "herdr-splits: cannot read replay fixture {}: {err}",
                    path.display()
                ),
                2,
            )
        })?;
        let replay = ReplayClient::from_json_lines(&fixture).map_err(|err| {
            Failure::new(
                format!("herdr-splits: bad replay fixture {}: {err}", path.display()),
                2,
            )
        })?;
        return match trace {
            Some(trace_path) => {
                let mut client = trace_wrap(&trace_path, Box::new(replay))?;
                body(&mut client)
            }
            None => {
                let mut client = replay;
                body(&mut client)
            }
        };
    }

    let socket = ctx::socket_path();
    let client = SocketClient::new(socket);
    match trace {
        Some(trace_path) => {
            let mut client = trace_wrap(&trace_path, Box::new(client))?;
            body(&mut client)
        }
        None => {
            let mut client = client;
            body(&mut client)
        }
    }
}

fn trace_wrap<'a>(path: &Path, inner: Box<dyn Client + 'a>) -> Result<TraceClient<'a>, Failure> {
    TraceClient::new(path, inner).map_err(|err| {
        Failure::new(
            format!(
                "herdr-splits: cannot open trace file {}: {err}",
                path.display()
            ),
            2,
        )
    })
}

/// Exit-1 message for a failure to reach the server at all. Matches the
/// Python wording (`cannot connect to herdr socket <path>: ...`) so existing
/// troubleshooting notes keep applying.
fn connect_failure(command: &str, error: &RpcError) -> Failure {
    if error.is_connect() {
        Failure::new(format!("herdr-splits {command}: {error}"), 1)
    } else {
        call_failure(command, error)
    }
}

/// Exit-1 message for a failure in a later step.
fn call_failure(command: &str, error: &RpcError) -> Failure {
    match error.method() {
        Some(method) => Failure::new(
            format!("herdr-splits {command}: {method} failed: {error}"),
            1,
        ),
        None => Failure::new(format!("herdr-splits {command}: {error}"), 1),
    }
}

fn run_install(root: Option<PathBuf>) -> Result<ExitCode, Failure> {
    let exe = std::env::current_exe().map_err(|err| {
        Failure::new(
            format!("herdr-splits install: cannot locate this binary: {err}"),
            1,
        )
    })?;
    let Some(root) = install::guess_root(&exe, root.as_deref()) else {
        return Err(Failure::new(
            "herdr-splits install: not inside a plugin checkout (no herdr-plugin.toml above \
             this binary); pass --root <plugin-root>",
            1,
        ));
    };
    // An explicit `--root` still has to be a checkout: creating scripts/bin
    // under an arbitrary directory (a mistyped path, `/`) is worse than failing.
    if !root.join("herdr-plugin.toml").is_file() {
        return Err(Failure::new(
            format!(
                "herdr-splits install: {} is not a plugin checkout (no herdr-plugin.toml)",
                root.display()
            ),
            1,
        ));
    }
    let outcome = install::install(&exe, &root).map_err(|err| {
        Failure::new(
            format!(
                "herdr-splits install: cannot write {}: {err}",
                root.join(install::DESTINATION).display()
            ),
            1,
        )
    })?;
    println!(
        "installed {} -> {}",
        outcome.source.display(),
        outcome.destination.display()
    );
    Ok(ExitCode::SUCCESS)
}

fn run_doctor(options: &Options) -> Result<ExitCode, Failure> {
    let report = match options.replay.clone() {
        Some(path) => {
            let fixture = std::fs::read_to_string(&path).map_err(|err| {
                Failure::new(
                    format!(
                        "herdr-splits: cannot read replay fixture {}: {err}",
                        path.display()
                    ),
                    2,
                )
            })?;
            let mut client = ReplayClient::from_json_lines(&fixture).map_err(|err| {
                Failure::new(
                    format!("herdr-splits: bad replay fixture {}: {err}", path.display()),
                    2,
                )
            })?;
            doctor::run(&mut client)
        }
        None => doctor::run_offline(),
    };
    doctor::print(&report);
    // `doctor` is diagnostic: an unhealthy server is reported, not an error.
    Ok(ExitCode::SUCCESS)
}
