//! Navigation decision tree.
//!
//! The Herdr-side counterpart of `lua/herdr-splits/nav.lua`: a keypress that
//! lands in Herdr, not in an editor. The order is exactly the Python
//! implementation's, because the two must agree while both exist:
//!
//! 1. Editor pane (`--pane-only` aside): forward the chord and stop. The
//!    editor plugin owns in-split movement, edge crossing and unzoom; unzooming
//!    here would break moving between splits inside one pane.
//! 2. `most_recently_focused` on: pick the adjacent pane by recency; success
//!    ends the run.
//! 3. Plain pane: read `pane.edges`, unzoom first if needed, then focus the
//!    neighbour — wrapping to the opposite direction at an edge, unless
//!    `nav_at_edge=stop`.
//!
//! `--pane-only` is the plain-Vim edge hand-off: pane-level selection only,
//! never forwarding a chord, because that would loop back into Vim itself.

use std::path::Path;

use crate::config::{Config, Kind};
use crate::ctx;
use crate::editor::is_vim_pane;
use crate::geometry::{self, Direction};
use crate::mru;
use crate::panes::{Edges, Layout, Panes};
use crate::rpc::RpcError;

/// Where the pane id came from; used by `doctor` for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneIdSource {
    /// `HERDR_PANE_ID`, injected into plugin actions.
    Environment,
    /// A `pane.current` roundtrip (manual invocation).
    Rpc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentPane {
    pub pane_id: String,
    pub source: PaneIdSource,
}

/// Resolve the current pane, preferring the injected environment variable.
pub fn current_pane(
    panes: &mut Panes<'_>,
    env_pane: Option<String>,
) -> Result<Option<CurrentPane>, RpcError> {
    if let Some(pane_id) = env_pane {
        return Ok(Some(CurrentPane {
            pane_id,
            source: PaneIdSource::Environment,
        }));
    }
    let pane_id = panes.current()?;
    if pane_id.is_empty() {
        return Ok(None);
    }
    Ok(Some(CurrentPane {
        pane_id,
        source: PaneIdSource::Rpc,
    }))
}

/// Run one navigation keypress. Returns `Ok(())` on success; the caller maps
/// transport failures to the documented stderr lines and exit code 1.
pub fn run(
    panes: &mut Panes<'_>,
    config: &Config,
    config_path: &Path,
    pane_id: &str,
    direction: Direction,
    pane_only: bool,
) -> Result<(), RpcError> {
    // --pane-only (Neovim/Vim crossing out at its edge): pane-level selection
    // only — never forward chords into panes, or the call would loop back into
    // the editor.
    if pane_only {
        if try_mru(panes, config, config_path, direction, pane_id)? {
            return Ok(());
        }
        return panes.focus_direction(pane_id, direction.as_str());
    }

    // Editor pane: forward the chord; the editor plugin owns movement + unzoom.
    if is_vim_pane(panes, pane_id)? {
        return panes.send_keys(pane_id, &[config.key.as_str()]);
    }

    // Plain pane: tmux-style MRU first when opted in; the legacy geometry
    // below is the fall-through when the search found nothing (edges,
    // collapsed zoomed layout, single pane).
    if try_mru(panes, config, config_path, direction, pane_id)? {
        return Ok(());
    }

    // Plain pane: unzoom if needed, then focus (wrapping at edges).
    let Some(mut edges) = panes.edges(pane_id)? else {
        return panes.focus_direction(pane_id, direction.as_str());
    };
    let mut edges_trusted = true;
    if edges.zoomed {
        if config.unzoom {
            panes.zoom_off(pane_id)?;
            edges = panes.edges(pane_id)?.unwrap_or(Edges {
                left: false,
                right: false,
                up: false,
                down: false,
                zoomed: false,
                layout: Layout {
                    zoomed: false,
                    panes: Vec::new(),
                },
            });
        } else {
            // Current Herdr keeps the pre-zoom tiling and accurate edge flags
            // while zoomed. Only a collapsed layout (older servers: the zoomed
            // pane alone, every edge reported reached) cannot drive a wrap.
            edges_trusted = geometry::layout_is_tiled(&edges.layout.panes);
        }
    }

    if edges_trusted && edges.in_direction(direction) {
        if !config.wrap_at_edge {
            return Ok(());
        }
        return panes.focus_direction(pane_id, direction.opposite().as_str());
    }
    panes.focus_direction(pane_id, direction.as_str())
}

/// The `most_recently_focused` branch. `false` hands the caller back to
/// Herdr's own `focus_direction` / edge handling (option off, collapsed zoomed
/// layout, socket error).
fn try_mru(
    panes: &mut Panes<'_>,
    config: &Config,
    config_path: &Path,
    direction: Direction,
    pane_id: &str,
) -> Result<bool, RpcError> {
    if !config.most_recently_focused {
        return Ok(false);
    }
    Ok(mru::mru_focus(panes, config_path, config, direction, pane_id)?.is_some())
}

/// Read the nav config for `direction` from the shared conf.
pub fn load_config(direction: Direction) -> (Config, std::path::PathBuf) {
    let path = ctx::config_path();
    (Config::load(&path, Kind::Nav, direction), path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::ReplayClient;
    use std::path::PathBuf;

    fn replay(fixture: &str) -> ReplayClient {
        ReplayClient::from_json_lines(fixture).expect("fixture")
    }

    fn nav_config() -> Config {
        Config::defaults(Kind::Nav, Direction::Left)
    }

    fn methods(client: &ReplayClient) -> Vec<&str> {
        client
            .calls()
            .iter()
            .map(|(method, _)| method.as_str())
            .collect()
    }

    fn edges_response(zoomed: bool, panes: &[(&str, i64, i64)]) -> String {
        let panes: Vec<String> = panes
            .iter()
            .map(|(id, x, width)| {
                format!(
                    r#"{{"pane_id":"{id}","rect":{{"x":{x},"y":0,"width":{width},"height":20}}}}"#
                )
            })
            .collect();
        format!(
            r#"{{"method":"pane.edges","response":{{"result":{{"edges":{{"left":true,"right":false,"up":true,"down":true,"pane_id":"A","layout":{{"zoomed":{zoomed},"panes":[{}]}}}}}}}}}}"#,
            panes.join(",")
        )
    }

    /// Detection fixtures for a plain shell pane: `pane.get` finds no token and
    /// no title marker, `pane.process_info` reports a shell. Every nav case
    /// below that is not about detection starts with these two replies.
    fn plain_detection() -> String {
        [
            r#"{"method":"pane.get","response":{"result":{"pane":{"pane_id":"A"}}}}"#,
            r#"{"method":"pane.process_info","response":{"result":{"process_info":{"foreground_processes":[{"name":"bash"}]}}}}"#,
        ]
        .join("\n")
    }
    /// `tests/test_herdr_nav.py::test_zoomed_pane_wraps_at_the_edge`
    #[test]
    fn zoomed_pane_wraps_at_the_edge_with_unzoom_disabled() {
        let fixture = [
            plain_detection(),
            edges_response(true, &[("A", 0, 40), ("B", 40, 40)]),
            r#"{"method":"pane.focus_direction","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        let mut config = nav_config();
        config.unzoom = false;
        run(
            &mut Panes::new(&mut client),
            &config,
            Path::new("conf"),
            "A",
            Direction::Left,
            false,
        )
        .unwrap();

        assert_eq!(
            methods(&client),
            vec![
                "pane.get",
                "pane.process_info",
                "pane.edges",
                "pane.focus_direction"
            ]
        );
        let (_, params) = &client.calls()[3];
        // At the left edge with wrap on, the focus goes to the opposite side.
        assert_eq!(
            params.get("direction").and_then(|v| v.as_str()),
            Some("right")
        );
    }

    /// `tests/test_herdr_nav.py::test_collapsed_zoomed_layout_keeps_requested_direction`
    #[test]
    fn collapsed_zoomed_layout_keeps_the_requested_direction() {
        let fixture = [
            plain_detection(),
            edges_response(true, &[("A", 0, 80)]),
            r#"{"method":"pane.focus_direction","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        let mut config = nav_config();
        config.unzoom = false;
        run(
            &mut Panes::new(&mut client),
            &config,
            Path::new("conf"),
            "A",
            Direction::Left,
            false,
        )
        .unwrap();

        assert_eq!(
            methods(&client),
            vec![
                "pane.get",
                "pane.process_info",
                "pane.edges",
                "pane.focus_direction"
            ]
        );
        let (_, params) = &client.calls()[3];
        assert_eq!(
            params.get("direction").and_then(|v| v.as_str()),
            Some("left")
        );
    }

    #[test]
    fn editor_pane_gets_the_chord_forwarded() {
        let fixture = [
            r#"{"method":"pane.get","response":{"result":{"pane":{"tokens":{"editor":"nvim"}}}}}"#
                .to_string(),
            r#"{"method":"pane.send_keys","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        let mut config = nav_config();
        config.key = "ctrl+h".to_string();
        run(
            &mut Panes::new(&mut client),
            &config,
            Path::new("conf"),
            "A",
            Direction::Left,
            false,
        )
        .unwrap();

        assert_eq!(methods(&client), vec!["pane.get", "pane.send_keys"]);
        assert_eq!(
            client.calls()[1].1.to_json(),
            r#"{"pane_id":"A","keys":["ctrl+h"]}"#
        );
    }

    #[test]
    fn pane_only_never_forwards_a_chord() {
        let fixture = r#"{"method":"pane.focus_direction","response":{"result":{}}}"#;
        let mut client = replay(fixture);
        run(
            &mut Panes::new(&mut client),
            &nav_config(),
            Path::new("conf"),
            "A",
            Direction::Right,
            true,
        )
        .unwrap();
        assert_eq!(methods(&client), vec!["pane.focus_direction"]);
        assert_eq!(
            client.calls()[0].1.to_json(),
            r#"{"pane_id":"A","direction":"right"}"#
        );
    }

    #[test]
    fn plain_pane_at_the_edge_with_stop_does_nothing() {
        let fixture = [
            plain_detection(),
            edges_response(false, &[("A", 0, 40), ("B", 40, 40)]),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        let mut config = nav_config();
        config.wrap_at_edge = false;
        run(
            &mut Panes::new(&mut client),
            &config,
            Path::new("conf"),
            "A",
            Direction::Left,
            false,
        )
        .unwrap();
        assert_eq!(
            methods(&client),
            vec!["pane.get", "pane.process_info", "pane.edges"]
        );
    }

    #[test]
    fn plain_pane_focuses_the_requested_direction_away_from_the_edge() {
        let fixture = [
            plain_detection(),
            edges_response(false, &[("A", 0, 40), ("B", 40, 40)]),
            r#"{"method":"pane.focus_direction","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        run(
            &mut Panes::new(&mut client),
            &nav_config(),
            Path::new("conf"),
            "A",
            Direction::Right,
            false,
        )
        .unwrap();
        let (_, params) = &client.calls()[3];
        assert_eq!(
            params.get("direction").and_then(|v| v.as_str()),
            Some("right")
        );
    }

    #[test]
    fn plain_pane_unzooms_before_deciding() {
        let fixture = [
            plain_detection(),
            edges_response(true, &[("A", 0, 40), ("B", 40, 40)]),
            r#"{"method":"pane.zoom","response":{"result":{}}}"#.to_string(),
            edges_response(false, &[("A", 0, 40), ("B", 40, 40)]),
            r#"{"method":"pane.focus_direction","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        run(
            &mut Panes::new(&mut client),
            &nav_config(),
            Path::new("conf"),
            "A",
            Direction::Left,
            false,
        )
        .unwrap();
        assert_eq!(
            methods(&client),
            vec![
                "pane.get",
                "pane.process_info",
                "pane.edges",
                "pane.zoom",
                "pane.edges",
                "pane.focus_direction"
            ]
        );
        // After unzooming, A is still at the left edge, so it wraps.
        assert_eq!(
            client.calls()[5]
                .1
                .get("direction")
                .and_then(|v| v.as_str()),
            Some("right")
        );
    }

    #[test]
    fn missing_edges_falls_back_to_the_requested_direction() {
        let fixture = [
            plain_detection(),
            r#"{"method":"pane.edges","response":{"result":{}}}"#.to_string(),
            r#"{"method":"pane.focus_direction","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        run(
            &mut Panes::new(&mut client),
            &nav_config(),
            Path::new("conf"),
            "A",
            Direction::Left,
            false,
        )
        .unwrap();
        assert_eq!(
            client.calls()[3]
                .1
                .get("direction")
                .and_then(|v| v.as_str()),
            Some("left")
        );
    }

    #[test]
    fn mru_branch_runs_before_the_edges_path() {
        let config_path: PathBuf = crate::testtmp::file("nav-mru", "herdr-splits.conf");
        std::fs::write(&config_path, "most_recently_focused=true\n").unwrap();

        let fixture = [
            r#"{"method":"pane.get","response":{"result":{"pane":{}}}}"#.to_string(),
            r#"{"method":"pane.process_info","response":{"result":{"process_info":{"foreground_processes":[{"name":"bash"}]}}}}"#.to_string(),
            r#"{"method":"pane.layout","response":{"result":{"layout":{"zoomed":false,"panes":[{"pane_id":"A","rect":{"x":0,"y":0,"width":40,"height":20}},{"pane_id":"B","rect":{"x":40,"y":0,"width":40,"height":20}}]}}}}"#.to_string(),
            r#"{"method":"pane.focus","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        let config = Config::load(&config_path, Kind::Nav, Direction::Right);
        assert!(config.most_recently_focused);
        run(
            &mut Panes::new(&mut client),
            &config,
            &config_path,
            "A",
            Direction::Right,
            false,
        )
        .unwrap();

        assert_eq!(
            methods(&client),
            vec!["pane.get", "pane.process_info", "pane.layout", "pane.focus"]
        );
        assert_eq!(
            client.calls()[3].1.get("pane_id").and_then(|v| v.as_str()),
            Some("B")
        );
    }

    #[test]
    fn current_pane_prefers_the_environment() {
        let mut client = replay("");
        let pane = current_pane(&mut Panes::new(&mut client), Some("w7:pN".to_string()))
            .unwrap()
            .unwrap();
        assert_eq!(pane.source, PaneIdSource::Environment);
        assert_eq!(pane.pane_id, "w7:pN");
        assert!(client.calls().is_empty());
    }

    #[test]
    fn current_pane_falls_back_to_the_rpc() {
        let mut client = replay(
            r#"{"method":"pane.current","response":{"result":{"pane":{"pane_id":"w1:p1"}}}}"#,
        );
        let pane = current_pane(&mut Panes::new(&mut client), None)
            .unwrap()
            .unwrap();
        assert_eq!(pane.source, PaneIdSource::Rpc);
        assert_eq!(pane.pane_id, "w1:p1");
    }

    #[test]
    fn current_pane_without_a_server_pane_is_none() {
        let mut client = replay(r#"{"method":"pane.current","response":{"result":{}}}"#);
        assert!(current_pane(&mut Panes::new(&mut client), None)
            .unwrap()
            .is_none());
    }
}
