//! Resize decision tree.
//!
//! Detection shares [`crate::editor::pane_editor`] with navigation, but resize
//! is Neovim-only: the Vim adapter maps navigation and not resizing, so a
//! plain-Vim pane must keep pane-level resizing (otherwise the forwarded
//! `alt+hjkl` would land in Vim unmapped). Neovim decides for itself whether to
//! resize its own split or hand the chord back to Herdr.
//!
//! Zoom state is left untouched, matching smart-splits' default.

use crate::config::{Config, Kind};
use crate::ctx;
use crate::editor::pane_editor;
use crate::geometry::Direction;
use crate::panes::{Editor, Panes};
use crate::rpc::RpcError;

/// Run one resize keypress.
pub fn run(
    panes: &mut Panes<'_>,
    config: &Config,
    pane_id: &str,
    direction: Direction,
    amount: f64,
) -> Result<(), RpcError> {
    if pane_editor(panes, pane_id)? == Some(Editor::Neovim) {
        return panes.send_keys(pane_id, &[config.key.as_str()]);
    }
    panes.resize(pane_id, direction.as_str(), amount)
}

/// Read the resize config for `direction` from the shared conf.
pub fn load_config(direction: Direction) -> Config {
    let path = ctx::config_path();
    Config::load(&path, Kind::Resize, direction)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::ReplayClient;

    fn replay(fixture: &str) -> ReplayClient {
        ReplayClient::from_json_lines(fixture).expect("fixture")
    }

    fn methods(client: &ReplayClient) -> Vec<&str> {
        client
            .calls()
            .iter()
            .map(|(method, _)| method.as_str())
            .collect()
    }

    fn resize_config() -> Config {
        Config::defaults(Kind::Resize, Direction::Left)
    }

    #[test]
    fn neovim_pane_gets_the_resize_chord() {
        let fixture = [
            r#"{"method":"pane.get","response":{"result":{"pane":{"tokens":{"editor":"nvim"}}}}}"#
                .to_string(),
            r#"{"method":"pane.send_keys","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        let mut config = resize_config();
        config.key = "alt+h".to_string();
        run(
            &mut Panes::new(&mut client),
            &config,
            "A",
            Direction::Left,
            crate::cli::DEFAULT_AMOUNT,
        )
        .unwrap();

        assert_eq!(methods(&client), vec!["pane.get", "pane.send_keys"]);
        assert_eq!(
            client.calls()[1].1.to_json(),
            r#"{"pane_id":"A","keys":["alt+h"]}"#
        );
    }

    #[test]
    fn plain_vim_pane_is_resized_at_the_herdr_level() {
        // The Vim plugin's title marker identifies *an editor*, but resize is
        // Neovim-only, so this must still hit pane.resize.
        let fixture = [
            r#"{"method":"pane.get","response":{"result":{"pane":{"terminal_title_stripped":"foo.txt - VIM [herdr-splits]"}}}}"#.to_string(),
            r#"{"method":"pane.resize","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        run(
            &mut Panes::new(&mut client),
            &resize_config(),
            "A",
            Direction::Right,
            0.05,
        )
        .unwrap();

        assert_eq!(methods(&client), vec!["pane.get", "pane.resize"]);
        assert_eq!(
            client.calls()[1].1.to_json(),
            r#"{"pane_id":"A","direction":"right","amount":0.05}"#
        );
    }

    #[test]
    fn shell_pane_is_resized_at_the_herdr_level() {
        let fixture = [
            r#"{"method":"pane.get","response":{"result":{"pane":{}}}}"#.to_string(),
            r#"{"method":"pane.process_info","response":{"result":{"process_info":{"foreground_processes":[{"name":"bash"}]}}}}"#.to_string(),
            r#"{"method":"pane.resize","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        run(
            &mut Panes::new(&mut client),
            &resize_config(),
            "A",
            Direction::Down,
            crate::cli::DEFAULT_AMOUNT,
        )
        .unwrap();
        assert_eq!(
            methods(&client),
            vec!["pane.get", "pane.process_info", "pane.resize"]
        );
    }

    #[test]
    fn default_amount_is_encoded_as_a_float() {
        let fixture = [
            r#"{"method":"pane.get","response":{"result":{"pane":{}}}}"#.to_string(),
            r#"{"method":"pane.process_info","response":{"result":{"process_info":{"foreground_processes":[]}}}}"#.to_string(),
            r#"{"method":"pane.resize","response":{"result":{}}}"#.to_string(),
        ]
        .join("\n");
        let mut client = replay(&fixture);
        run(
            &mut Panes::new(&mut client),
            &resize_config(),
            "A",
            Direction::Up,
            crate::cli::DEFAULT_AMOUNT,
        )
        .unwrap();
        let body = client.calls()[2].1.to_json();
        assert!(body.contains(r#""amount":0.03"#), "{body}");
    }

    #[test]
    fn load_config_reads_the_resize_keys() {
        // Point the resolution at a scratch conf via HERDR_SPLITS_CONFIG; the
        // environment is process-global, so hold the lock while it is set.
        let _env = crate::testtmp::env_lock();
        let path = crate::testtmp::file("resize-conf", "herdr-splits.conf");
        std::fs::write(&path, "resize_key_left=alt+left\n").unwrap();
        let previous = std::env::var("HERDR_SPLITS_CONFIG").ok();
        std::env::set_var("HERDR_SPLITS_CONFIG", &path);
        let config = load_config(Direction::Left);
        match previous {
            Some(value) => std::env::set_var("HERDR_SPLITS_CONFIG", value),
            None => std::env::remove_var("HERDR_SPLITS_CONFIG"),
        }
        let _ = std::fs::remove_file(&path);
        assert_eq!(config.key, "alt+left");
    }

    #[test]
    fn transport_failures_propagate() {
        let fixture = r#"{"method":"pane.get","connect_error":"denied"}"#;
        let mut client = replay(fixture);
        let error = run(
            &mut Panes::new(&mut client),
            &resize_config(),
            "A",
            Direction::Left,
            0.03,
        )
        .unwrap_err();
        assert!(error.is_connect());
    }

    #[test]
    fn direction_strings_match_the_rpc_schema() {
        for direction in [
            Direction::Left,
            Direction::Down,
            Direction::Up,
            Direction::Right,
        ] {
            assert!(matches!(
                direction.as_str(),
                "left" | "down" | "up" | "right"
            ));
        }
    }
}
