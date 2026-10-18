//! `doctor`: print what the helper resolved and whether the server answers.
//!
//! Manual invocation has no argv plumbing to report failures through, and
//! `herdr plugin log` only shows the action's own output. This subcommand makes
//! the same information reachable from a terminal (and from
//! `:checkhealth herdr-splits`, which runs it).

use crate::config::{Config, Kind};
use crate::ctx;
use crate::editor;
use crate::geometry::Direction;
use crate::nav::{self, PaneIdSource};
use crate::panes::Panes;
use crate::rpc::Client;

/// One diagnostic line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub lines: Vec<String>,
    /// True when the failure is one a user can fix by starting/attaching herdr.
    pub healthy: bool,
}

/// Probe the environment and the running server.
pub fn run(client: &mut dyn Client) -> Report {
    let mut lines = Vec::new();
    lines.push(format!("herdr-splits {}", env!("CARGO_PKG_VERSION")));
    lines.push(format!(
        "exe: {}",
        std::env::current_exe().map_or_else(
            |err| format!("<unknown: {err}>"),
            |path| path.display().to_string(),
        )
    ));
    let socket = ctx::socket_path();
    lines.push(format!("socket: {socket}"));
    let config_path = ctx::config_path();
    lines.push(format!("config: {}", config_path.display()));
    lines.push(format!(
        "config exists: {}",
        if config_path.is_file() { "yes" } else { "no" }
    ));

    let (nav_config, _) = nav::load_config(Direction::Left);
    let resize_config = Config::load(&config_path, Kind::Resize, Direction::Left);
    lines.push(format!(
        "conf: unzoom_on_nav={} nav_at_edge={} most_recently_focused={}",
        nav_config.unzoom,
        if nav_config.wrap_at_edge {
            "wrap"
        } else {
            "stop"
        },
        nav_config.most_recently_focused
    ));
    lines.push(format!(
        "chords: nav(left)={} resize(left)={}",
        nav_config.key, resize_config.key
    ));

    let mut panes = Panes::new(client);
    let env_pane = ctx::env_pane_id();
    lines.push(format!(
        "HERDR_PANE_ID: {}",
        env_pane.as_deref().unwrap_or("<unset>")
    ));

    let mut healthy = true;
    match nav::current_pane(&mut panes, env_pane) {
        Ok(Some(pane)) => {
            let source = match pane.source {
                PaneIdSource::Environment => "environment",
                PaneIdSource::Rpc => "pane.current",
            };
            lines.push(format!("current pane: {} ({source})", pane.pane_id));
            // One `pane.get` supplies both the editor decision and the title
            // reported below; a diagnostic must not need two roundtrips.
            match panes.get(&pane.pane_id) {
                Ok(info) => {
                    match editor::classify(&mut panes, &pane.pane_id, &info) {
                        Ok(Some(editor)) => lines.push(format!("editor: {}", editor.as_token())),
                        Ok(None) => lines.push("editor: none (plain pane)".to_string()),
                        Err(err) => {
                            healthy = false;
                            lines.push(format!("editor: <error: {err}>"));
                        }
                    }
                    lines.push(format!("title: {:?}", info.title));
                }
                Err(err) => {
                    healthy = false;
                    lines.push(format!("pane.get: <error: {err}>"));
                }
            }
        }
        Ok(None) => {
            healthy = false;
            lines.push("current pane: <unknown>".to_string());
        }
        Err(err) => {
            healthy = false;
            lines.push(format!("current pane: <error: {err}>"));
        }
    }

    if !healthy {
        lines.push(String::new());
        lines.push(
            "If the socket is wrong, check HERDR_SOCKET_PATH. If herdr runs elevated,\n\
             an ordinary terminal cannot open its pipe: start the helper from an\n\
             elevated shell, or restart the server with --allow-unelevated-clients."
                .to_string(),
        );
    }
    Report { lines, healthy }
}

/// `doctor` without a live server: still useful (paths and config).
pub fn run_offline() -> Report {
    run(&mut crate::rpc::SocketClient::new(ctx::socket_path()))
}

/// Convenience for the CLI: print a report to stdout.
pub fn print(report: &Report) {
    for line in &report.lines {
        println!("{line}");
    }
}
