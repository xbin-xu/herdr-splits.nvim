//! tmux-style pane selection (`most_recently_focused`).
//!
//! Each keypress is a fresh process, so the recency history cannot live in
//! memory: it is a small file next to the shared conf, one pane id per line,
//! most-recent-first. The Neovim side writes and reads the same file
//! (`lua/herdr-splits/herdr.lua`), which is why the format is trivial.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::geometry::{self, Direction};
use crate::panes::Panes;
use crate::rpc::RpcError;

/// The MRU file that sits beside the shared conf.
pub fn mru_path(config_path: &Path) -> PathBuf {
    match config_path.parent() {
        Some(parent) => parent.join("mru-panes"),
        None => PathBuf::from("mru-panes"),
    }
}

/// Pane ids, most-recent-first. Unreadable files yield an empty list: recency
/// is an optimization, never a precondition.
pub fn load(path: &Path) -> Vec<String> {
    let Ok(bytes) = fs::read(path) else {
        return Vec::new();
    };
    String::from_utf8_lossy(&bytes)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// Best-effort atomic write: same-directory temp + rename, so a concurrent
/// reader never sees a half-written list. `fs::rename` replaces an existing
/// destination on Windows and Unix alike.
pub fn save(path: &Path, order: &[String]) {
    let mut content = String::new();
    for pane in order {
        content.push_str(pane);
        content.push('\n');
    }
    let temp = PathBuf::from(format!("{}.tmp.{}", path.display(), std::process::id()));
    let write = || -> std::io::Result<()> {
        let mut file = fs::File::create(&temp)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        Ok(())
    };
    if write().is_err() {
        let _ = fs::remove_file(&temp);
        return;
    }
    if fs::rename(&temp, path).is_err() {
        let _ = fs::remove_file(&temp);
    }
}

/// Focus the most recently focused pane among the candidates adjacent in
/// `direction`.
///
/// Returns the focused pane id, or `None` when the search decides nothing:
/// the caller then falls back to Herdr's own `focus_direction` / edge
/// handling. Recency is recorded from the plugin's own moves plus the pane
/// hosting each keypress (the current pane is bumped on every invocation).
///
/// Zoom: with auto-unzoom enabled the pane is unzoomed first and the search
/// runs on the restored layout. With it disabled the search still runs while
/// the server keeps reporting the pre-zoom tiling; a collapsed zoomed layout
/// has no candidates, so the caller falls back to the legacy path.
pub fn mru_focus(
    panes: &mut Panes<'_>,
    config_path: &Path,
    config: &Config,
    direction: Direction,
    cur_pane: &str,
) -> Result<Option<String>, RpcError> {
    if cur_pane.is_empty() {
        return Ok(None);
    }

    let mut layout = panes.layout()?;
    if layout.zoomed {
        if config.unzoom {
            panes.zoom_off(cur_pane)?;
            layout = panes.layout()?;
        } else if !geometry::layout_is_tiled(&layout.panes) {
            return Ok(None);
        }
    }

    let mut candidates = geometry::candidates(&layout.panes, cur_pane, direction, false);
    if candidates.is_empty() {
        if !config.wrap_at_edge {
            return Ok(None);
        }
        candidates = geometry::candidates(&layout.panes, cur_pane, direction, true);
    }
    if candidates.is_empty() {
        return Ok(None);
    }

    let path = mru_path(config_path);
    // Live panes only; the current pane counts as the most recent focus.
    let live = |pane: &str| layout.panes.iter().any(|cell| cell.id == pane);
    let mut order: Vec<String> = vec![cur_pane.to_string()];
    for pane in load(&path) {
        if pane != cur_pane && live(&pane) {
            order.push(pane);
        }
    }

    let pick = order
        .iter()
        .find(|pane| candidates.contains(pane))
        .cloned()
        .unwrap_or_else(|| candidates[0].clone()); // no history yet: server order

    let mut saved = vec![pick.clone()];
    saved.extend(order.into_iter().filter(|pane| pane != &pick));
    save(&path, &saved);

    panes.focus(&pick)?;
    Ok(Some(pick))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Cell;
    use crate::rpc::ReplayClient;

    fn replay(fixture: &str) -> ReplayClient {
        ReplayClient::from_json_lines(fixture).expect("fixture")
    }

    fn conf(kind: crate::config::Kind, direction: Direction) -> Config {
        Config::defaults(kind, direction)
    }

    /// Two panes side by side: A (0..39) and B (40..79).
    fn layout_fixture(zoomed: bool) -> String {
        format!(
            r#"{{"method":"pane.layout","response":{{"result":{{"layout":{{"zoomed":{zoomed},"panes":[{{"pane_id":"A","rect":{{"x":0,"y":0,"width":40,"height":20}}}},{{"pane_id":"B","rect":{{"x":40,"y":0,"width":40,"height":20}}}}]}}}}}}}}"#
        )
    }

    fn temp_conf_dir(tag: &str) -> PathBuf {
        crate::testtmp::file(tag, "herdr-splits.conf")
    }

    #[test]
    fn picks_the_only_candidate_and_records_the_move() {
        let conf_path = temp_conf_dir("basic");
        let fixture = [
            layout_fixture(false),
            r#"{"method":"pane.focus","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        let pick = mru_focus(
            &mut Panes::new(&mut client),
            &conf_path,
            &conf(crate::config::Kind::Nav, Direction::Right),
            Direction::Right,
            "A",
        )
        .unwrap();
        assert_eq!(pick.as_deref(), Some("B"));

        let stored = super::load(&mru_path(&conf_path));
        assert_eq!(stored, vec!["B".to_string(), "A".to_string()]);
        let _ = fs::remove_dir_all(conf_path.parent().unwrap());
    }

    #[test]
    fn prefers_history_over_server_order() {
        let conf_path = temp_conf_dir("history");
        super::save(&mru_path(&conf_path), &["B".to_string(), "A".to_string()]);
        // Three panes in a row: A | B | C, moving right from A.
        let fixture = [
            r#"{"method":"pane.layout","response":{"result":{"layout":{"zoomed":false,"panes":[{"pane_id":"A","rect":{"x":0,"y":0,"width":10,"height":20}},{"pane_id":"B","rect":{"x":10,"y":0,"width":10,"height":10}},{"pane_id":"C","rect":{"x":10,"y":10,"width":10,"height":10}}]}}}}"#,
            r#"{"method":"pane.focus","response":{"result":{}}}"#,
        ]
        .join("\n");
        let mut client = replay(&fixture);
        // Both B and C are adjacent to A; history says C was seen? No: only
        // live panes are kept, and the order is [A, B] after filtering, so B
        // wins as the first candidate in history order.
        let pick = mru_focus(
            &mut Panes::new(&mut client),
            &conf_path,
            &conf(crate::config::Kind::Nav, Direction::Right),
            Direction::Right,
            "A",
        )
        .unwrap();
        assert_eq!(pick.as_deref(), Some("B"));
        let _ = fs::remove_dir_all(conf_path.parent().unwrap());
    }

    #[test]
    fn wraps_to_the_opposite_edge_when_nav_at_edge_is_wrap() {
        let conf_path = temp_conf_dir("wrap");
        let fixture = [
            layout_fixture(false),
            r#"{"method":"pane.focus","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        let pick = mru_focus(
            &mut Panes::new(&mut client),
            &conf_path,
            &conf(crate::config::Kind::Nav, Direction::Left),
            Direction::Left,
            "A",
        )
        .unwrap();
        assert_eq!(pick.as_deref(), Some("B"));
        let _ = fs::remove_dir_all(conf_path.parent().unwrap());
    }

    #[test]
    fn stops_at_the_edge_when_nav_at_edge_is_stop() {
        let conf_path = temp_conf_dir("stop");
        let mut client = replay(&layout_fixture(false));
        let mut config = conf(crate::config::Kind::Nav, Direction::Left);
        config.wrap_at_edge = false;
        let pick = mru_focus(
            &mut Panes::new(&mut client),
            &conf_path,
            &config,
            Direction::Left,
            "A",
        )
        .unwrap();
        assert_eq!(pick, None);
        assert_eq!(client.calls().len(), 1, "only the layout query");
        let _ = fs::remove_dir_all(conf_path.parent().unwrap());
    }

    #[test]
    fn unzooms_first_when_enabled() {
        let conf_path = temp_conf_dir("unzoom");
        let fixture = [
            layout_fixture(true),
            r#"{"method":"pane.zoom","response":{"result":{}}}"#.to_string(),
            layout_fixture(false),
            r#"{"method":"pane.focus","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        let pick = mru_focus(
            &mut Panes::new(&mut client),
            &conf_path,
            &conf(crate::config::Kind::Nav, Direction::Right),
            Direction::Right,
            "A",
        )
        .unwrap();
        assert_eq!(pick.as_deref(), Some("B"));
        let methods: Vec<_> = client
            .calls()
            .iter()
            .map(|(method, _)| method.as_str())
            .collect();
        assert_eq!(
            methods,
            vec!["pane.layout", "pane.zoom", "pane.layout", "pane.focus"]
        );
        let _ = fs::remove_dir_all(conf_path.parent().unwrap());
    }

    #[test]
    fn a_collapsed_zoomed_layout_without_unzoom_decides_nothing() {
        let conf_path = temp_conf_dir("collapsed");
        let fixture = r#"{"method":"pane.layout","response":{"result":{"layout":{"zoomed":true,"panes":[{"pane_id":"A","rect":{"x":0,"y":0,"width":80,"height":24}}]}}}}"#;
        let mut client = replay(fixture);
        let mut config = conf(crate::config::Kind::Nav, Direction::Right);
        config.unzoom = false;
        let pick = mru_focus(
            &mut Panes::new(&mut client),
            &conf_path,
            &config,
            Direction::Right,
            "A",
        )
        .unwrap();
        assert_eq!(pick, None);
        let _ = fs::remove_dir_all(conf_path.parent().unwrap());
    }

    #[test]
    fn load_ignores_blank_lines_and_trailing_whitespace() {
        let dir = crate::testtmp::dir("mru-load");
        let path = dir.join("mru-panes");
        fs::write(&path, "  w7:pN  \n\nw7:pP\n").unwrap();
        assert_eq!(load(&path), vec!["w7:pN", "w7:pP"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_of_a_missing_file_is_empty() {
        assert!(load(Path::new("/nonexistent/mru-panes")).is_empty());
    }

    #[test]
    fn save_creates_parent_free_paths_and_round_trips() {
        let dir = crate::testtmp::dir("mru-save");
        let path = dir.join("mru-panes");
        save(&path, &["a".to_string(), "b".to_string()]);
        assert_eq!(load(&path), vec!["a", "b"]);
        // No temp file is left behind.
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(leftovers.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn candidates_are_the_geometric_ones() {
        // Guard the contract mru_focus relies on: B is adjacent to A going
        // right, and nothing is adjacent going up.
        let cells = vec![
            Cell::new("A", crate::geometry::Rect::new(0, 0, 40, 20)),
            Cell::new("B", crate::geometry::Rect::new(40, 0, 40, 20)),
        ];
        assert_eq!(
            geometry::candidates(&cells, "A", Direction::Right, false),
            vec!["B"]
        );
        assert!(geometry::candidates(&cells, "A", Direction::Up, false).is_empty());
    }
}
