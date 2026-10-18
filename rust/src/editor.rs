//! Editor detection: which program owns a pane's navigation keys?
//!
//! Three signals, in the order the Python original checks them:
//!
//! 1. the Neovim plugin's TTL'd registration token (`tokens.editor == "nvim"`),
//! 2. the Vim plugin's terminal-title marker (`[herdr-splits]`),
//! 3. foreground process names, for plugin-less vim/gvim on Unix.
//!
//! The title is checked before `process_info` because it is exact and already
//! in hand. Process names remain the fallback only: on Windows the pane shell's
//! MSYS spawn layer hides everything below it, which is why the title marker
//! exists in the first place.

use crate::panes::{Editor, Panes};
use crate::rpc::RpcError;
use crate::vimname::is_vim_process_name;

/// The Vim plugin appends this marker to its terminal title under `HERDR_ENV`.
/// It is the plain-Vim analogue of the nvim pane token: it advertises the
/// plugin (not merely "a vim process"), which is what the forwarding decision
/// needs.
pub const VIM_TITLE_MARKER: &str = "[herdr-splits]";

/// Identify the editor owning the pane.
///
/// Returns `Ok(None)` for plain shells and for panes whose signals are
/// inconclusive; only transport failures are errors.
pub fn pane_editor(panes: &mut Panes<'_>, pane_id: &str) -> Result<Option<Editor>, RpcError> {
    if pane_id.is_empty() {
        return Ok(None);
    }
    let info = panes.get(pane_id)?;
    classify(panes, pane_id, &info)
}

/// Decide the editor from an already-fetched `pane.get`.
///
/// Split out so `doctor` can report on the pane facts it fetched without paying
/// for a second roundtrip (and so a finite replay fixture only needs one
/// scripted `pane.get`).
pub fn classify(
    panes: &mut Panes<'_>,
    pane_id: &str,
    info: &crate::panes::PaneInfo,
) -> Result<Option<Editor>, RpcError> {
    if pane_id.is_empty() {
        return Ok(None);
    }
    if info.editor_token.as_deref() == Some(Editor::Neovim.as_token()) {
        return Ok(Some(Editor::Neovim));
    }
    if info.title.contains(VIM_TITLE_MARKER) {
        return Ok(Some(Editor::Vim));
    }

    for name in panes.foreground_processes(pane_id)? {
        if is_vim_process_name(&name) {
            let lowered = name.to_lowercase();
            return Ok(Some(if lowered.contains("nvim") {
                Editor::Neovim
            } else {
                Editor::Vim
            }));
        }
    }
    Ok(None)
}

/// True when the pane runs an editor that owns the navigation keys.
pub fn is_vim_pane(panes: &mut Panes<'_>, pane_id: &str) -> Result<bool, RpcError> {
    Ok(pane_editor(panes, pane_id)?.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::ReplayClient;

    fn detect(fixture: &str, pane_id: &str) -> (Option<Editor>, usize) {
        let mut client = replay(fixture);
        let editor =
            pane_editor(&mut Panes::new(&mut client), pane_id).expect("no transport error");
        (editor, client.calls().len())
    }

    fn replay(fixture: &str) -> ReplayClient {
        ReplayClient::from_json_lines(fixture).expect("fixture")
    }

    /// Fixtures mirror `tests/test_detection.py`, which pins the decision order.
    #[test]
    fn token_identifies_nvim_without_further_queries() {
        let (editor, calls) = detect(
            r#"{"method":"pane.get","response":{"result":{"pane":{"tokens":{"editor":"nvim"}}}}}"#,
            "w1:p1",
        );
        assert_eq!(editor, Some(Editor::Neovim));
        assert_eq!(calls, 1);
    }

    #[test]
    fn title_marker_identifies_vim_without_further_queries() {
        let (editor, calls) = detect(
            r#"{"method":"pane.get","response":{"result":{"pane":{"terminal_title_stripped":"foo.txt - VIM [herdr-splits]"}}}}"#,
            "w1:p1",
        );
        assert_eq!(editor, Some(Editor::Vim));
        assert_eq!(calls, 1);
    }

    #[test]
    fn marker_matches_anywhere_in_the_title() {
        let (editor, _) = detect(
            r#"{"method":"pane.get","response":{"result":{"pane":{"terminal_title_stripped":"x [herdr-splits] y"}}}}"#,
            "w1:p1",
        );
        assert_eq!(editor, Some(Editor::Vim));
    }

    #[test]
    fn process_names_are_the_last_fallback() {
        let fixture = r#"{"method":"pane.get","response":{"result":{"pane":{}}}}
{"method":"pane.process_info","response":{"result":{"process_info":{"foreground_processes":[{"name":"nvim.exe"}]}}}}"#;
        let (editor, calls) = detect(fixture, "w1:p1");
        assert_eq!(editor, Some(Editor::Neovim));
        assert_eq!(calls, 2);

        for (name, expected) in [
            ("vim.exe", Editor::Vim),
            ("gvim.exe", Editor::Vim),
            ("vim", Editor::Vim),
            ("gview", Editor::Vim),
            ("nvim", Editor::Neovim),
        ] {
            let fixture = [
                r#"{"method":"pane.get","response":{"result":{"pane":{}}}}"#,
                &format!(
                    r#"{{"method":"pane.process_info","response":{{"result":{{"process_info":{{"foreground_processes":[{{"name":"{name}"}}]}}}}}}}}"#
                ),
            ]
            .join("\n");
            let (editor, _) = detect(&fixture, "w1:p1");
            assert_eq!(editor, Some(expected), "{name}");
        }

        let (editor, _) = detect(
            r#"{"method":"pane.get","response":{"result":{"pane":{}}}}
{"method":"pane.process_info","response":{"result":{"process_info":{"foreground_processes":[{"name":"bash.exe"}]}}}}"#,
            "w1:p1",
        );
        assert_eq!(editor, None);
    }

    #[test]
    fn empty_pane_id_short_circuits() {
        let mut client = replay("");
        let editor = pane_editor(&mut Panes::new(&mut client), "").unwrap();
        assert_eq!(editor, None);
        assert_eq!(client.calls().len(), 0);
    }

    #[test]
    fn pane_get_error_falls_through_to_process_names() {
        let fixture = r#"{"method":"pane.get","response":{"error":{"code":"pane_not_found"}}}
{"method":"pane.process_info","response":{"result":{"process_info":{"foreground_processes":[{"name":"vim"}]}}}}"#;
        let (editor, _) = detect(fixture, "w1:p1");
        assert_eq!(editor, Some(Editor::Vim));
    }

    #[test]
    fn is_vim_pane_wraps_the_kind() {
        let mut client = replay(
            r#"{"method":"pane.get","response":{"result":{"pane":{"terminal_title_stripped":"[herdr-splits]"}}}}"#,
        );
        assert!(is_vim_pane(&mut Panes::new(&mut client), "w1:p1").unwrap());

        let mut client = replay(
            r#"{"method":"pane.get","response":{"result":{"pane":{}}}}
{"method":"pane.process_info","response":{"result":{"process_info":{"foreground_processes":[{"name":"bash"}]}}}}"#,
        );
        assert!(!is_vim_pane(&mut Panes::new(&mut client), "w1:p1").unwrap());
    }

    #[test]
    fn a_token_other_than_nvim_is_not_an_editor() {
        let (editor, _) = detect(
            r#"{"method":"pane.get","response":{"result":{"pane":{"tokens":{"editor":"something"}}}}}
{"method":"pane.process_info","response":{"result":{"process_info":{"foreground_processes":[{"name":"bash"}]}}}}"#,
            "w1:p1",
        );
        assert_eq!(editor, None);
    }
}
