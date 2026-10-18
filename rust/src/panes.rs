//! Typed pane operations on top of [`Client`].
//!
//! Each method maps to one herdr RPC (`herdr api schema --json`, protocol 22)
//! and mirrors the helper of the same name in `scripts/herdr_common.py`. The
//! reply shapes are defensive: herdr omits fields, an error reply has no
//! `result`, and a stale pane id is a normal outcome rather than a crash.

use crate::geometry::{Cell, Rect};
use crate::json::Value;
use crate::rpc::{Client, RpcError};

/// The editor that owns a pane, as far as detection can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Editor {
    Neovim,
    Vim,
}

impl Editor {
    /// The token string the Neovim plugin reports (`tokens.editor`).
    pub fn as_token(self) -> &'static str {
        match self {
            Editor::Neovim => "nvim",
            Editor::Vim => "vim",
        }
    }
}

/// `pane.layout` decoded: the zoom flag plus rectangles in server order.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub zoomed: bool,
    pub panes: Vec<Cell>,
}

/// Pane facts carried by one `pane.get` reply.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PaneInfo {
    pub pane_id: String,
    /// `tokens.editor` reported by the Neovim plugin (`register.lua`).
    pub editor_token: Option<String>,
    /// The terminal title with escape sequences stripped.
    pub title: String,
}

/// The always-current signals `pane.get` carries, plus the process fallback.
pub struct Panes<'a> {
    client: &'a mut dyn Client,
}

impl<'a> Panes<'a> {
    pub fn new(client: &'a mut dyn Client) -> Panes<'a> {
        Panes { client }
    }

    /// `pane.current`: the focused pane, used only when `HERDR_PANE_ID` is
    /// missing (manual invocation).
    pub fn current(&mut self) -> Result<String, RpcError> {
        let reply = self.call("pane.current", Value::Object(Vec::new()))?;
        Ok(reply
            .path(&["result", "pane", "pane_id"])
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string())
    }

    /// `pane.get`, decoded to the fields detection needs. An error reply (a
    /// stale pane id) yields the default `PaneInfo`, mirroring the Python
    /// helper's `(resp.get("result") or {}).get("pane") or {}`.
    pub fn get(&mut self, pane_id: &str) -> Result<PaneInfo, RpcError> {
        let reply = self.call(
            "pane.get",
            Value::object(vec![("pane_id", Value::str(pane_id))]),
        )?;
        let pane = reply.path(&["result", "pane"]);
        Ok(PaneInfo {
            pane_id: pane
                .and_then(|pane| pane.get("pane_id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            editor_token: pane
                .and_then(|pane| pane.path(&["tokens", "editor"]))
                .and_then(Value::as_str)
                .map(str::to_string),
            title: pane
                .and_then(|pane| pane.get("terminal_title_stripped"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        })
    }

    /// `pane.process_info`: foreground process names. An error reply (older
    /// servers, or a pane whose processes are hidden) yields an empty list.
    pub fn foreground_processes(&mut self, pane_id: &str) -> Result<Vec<String>, RpcError> {
        let reply = self.call(
            "pane.process_info",
            Value::object(vec![("pane_id", Value::str(pane_id))]),
        )?;
        if reply.get("error").is_some() {
            return Ok(Vec::new());
        }
        let processes = reply
            .path(&["result", "process_info", "foreground_processes"])
            .and_then(Value::as_array);
        Ok(processes
            .map(|items| {
                items
                    .iter()
                    .filter_map(|proc| proc.get("name").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default())
    }

    /// `pane.edges`: the boolean edge flags plus the embedded layout.
    ///
    /// Returns `None` when the reply has no `result.edges` object, which is what
    /// the callers mean by "no answer" (they then fall back to a plain
    /// `pane.focus_direction`). A reply carrying `error` is *not* `None` here:
    /// callers treat a server-side error as "edge unknown" too, and the Python
    /// original did the same by reading the missing fields as falsy.
    pub fn edges(&mut self, pane_id: &str) -> Result<Option<Edges>, RpcError> {
        let reply = self.call(
            "pane.edges",
            Value::object(vec![("pane_id", Value::str(pane_id))]),
        )?;
        let Some(edges) = reply
            .path(&["result", "edges"])
            .filter(|value| matches!(value, Value::Object(_)))
        else {
            return Ok(None);
        };
        Ok(Some(Edges {
            left: edges.get("left").and_then(Value::as_bool).unwrap_or(false),
            right: edges.get("right").and_then(Value::as_bool).unwrap_or(false),
            up: edges.get("up").and_then(Value::as_bool).unwrap_or(false),
            down: edges.get("down").and_then(Value::as_bool).unwrap_or(false),
            zoomed: edges
                .path(&["layout", "zoomed"])
                .is_some_and(Value::is_truthy),
            layout: decode_layout(edges.get("layout")),
        }))
    }

    /// `pane.layout` for the current tab.
    pub fn layout(&mut self) -> Result<Layout, RpcError> {
        let reply = self.call("pane.layout", Value::Object(Vec::new()))?;
        Ok(decode_layout(reply.path(&["result", "layout"])))
    }

    pub fn send_keys(&mut self, pane_id: &str, keys: &[&str]) -> Result<(), RpcError> {
        self.call(
            "pane.send_keys",
            Value::object(vec![
                ("pane_id", Value::str(pane_id)),
                (
                    "keys",
                    Value::Array(keys.iter().map(|key| Value::str(*key)).collect()),
                ),
            ]),
        )?;
        Ok(())
    }

    pub fn focus(&mut self, pane_id: &str) -> Result<(), RpcError> {
        self.call(
            "pane.focus",
            Value::object(vec![("pane_id", Value::str(pane_id))]),
        )?;
        Ok(())
    }

    pub fn focus_direction(&mut self, pane_id: &str, direction: &str) -> Result<(), RpcError> {
        self.call(
            "pane.focus_direction",
            Value::object(vec![
                ("pane_id", Value::str(pane_id)),
                ("direction", Value::str(direction)),
            ]),
        )?;
        Ok(())
    }

    pub fn zoom_off(&mut self, pane_id: &str) -> Result<(), RpcError> {
        self.call(
            "pane.zoom",
            Value::object(vec![
                ("pane_id", Value::str(pane_id)),
                ("mode", Value::str("off")),
            ]),
        )?;
        Ok(())
    }

    pub fn resize(&mut self, pane_id: &str, direction: &str, amount: f64) -> Result<(), RpcError> {
        self.call(
            "pane.resize",
            Value::object(vec![
                ("pane_id", Value::str(pane_id)),
                ("direction", Value::str(direction)),
                ("amount", Value::Float(amount)),
            ]),
        )?;
        Ok(())
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, RpcError> {
        self.client.call(method, &params)
    }
}

/// `pane.edges` decoded.
#[derive(Debug, Clone, PartialEq)]
pub struct Edges {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub zoomed: bool,
    pub layout: Layout,
}

impl Edges {
    pub fn in_direction(&self, direction: crate::geometry::Direction) -> bool {
        match direction {
            crate::geometry::Direction::Left => self.left,
            crate::geometry::Direction::Right => self.right,
            crate::geometry::Direction::Up => self.up,
            crate::geometry::Direction::Down => self.down,
        }
    }
}

/// Decode a `layout` object. A missing layout is an empty, non-zoomed one.
fn decode_layout(layout: Option<&Value>) -> Layout {
    let Some(layout) = layout else {
        return Layout {
            zoomed: false,
            panes: Vec::new(),
        };
    };
    let panes = layout
        .get("panes")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|pane| {
                    // `pane_id` is required; a pane without one is unusable for
                    // the search and the Python side raised on it, so skip it.
                    let id = pane.get("pane_id").and_then(Value::as_str)?;
                    let rect = pane.get("rect");
                    let coordinate = |name: &str| {
                        rect.and_then(|rect| rect.get(name))
                            .and_then(Value::as_i64)
                            .unwrap_or(0)
                    };
                    Some(Cell::new(
                        id,
                        Rect::new(
                            coordinate("x"),
                            coordinate("y"),
                            coordinate("width"),
                            coordinate("height"),
                        ),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Layout {
        zoomed: layout.get("zoomed").is_some_and(Value::is_truthy),
        panes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::ReplayClient;

    fn replay(fixture: &str) -> ReplayClient {
        ReplayClient::from_json_lines(fixture).expect("fixture")
    }

    #[test]
    fn pane_get_decodes_token_and_title() {
        let mut client = replay(
            r#"{"method":"pane.get","response":{"result":{"pane":{"pane_id":"w1:p1","tokens":{"editor":"nvim"},"terminal_title_stripped":"main.rs - NVIM"}}}}"#,
        );
        let info = Panes::new(&mut client).get("w1:p1").unwrap();
        assert_eq!(info.pane_id, "w1:p1");
        assert_eq!(info.editor_token.as_deref(), Some("nvim"));
        assert_eq!(info.title, "main.rs - NVIM");
    }

    #[test]
    fn pane_get_tolerates_an_error_reply() {
        let mut client = replay(
            r#"{"method":"pane.get","response":{"error":{"code":"pane_not_found","message":"gone"}}}"#,
        );
        let info = Panes::new(&mut client).get("w1:p9").unwrap();
        assert_eq!(info, PaneInfo::default());
    }

    #[test]
    fn process_info_returns_names_in_order() {
        let mut client = replay(
            r#"{"method":"pane.process_info","response":{"result":{"process_info":{"foreground_processes":[{"name":"nvim.exe"},{"name":"bash.exe"}]}}}}"#,
        );
        let names = Panes::new(&mut client)
            .foreground_processes("w1:p1")
            .unwrap();
        assert_eq!(names, vec!["nvim.exe", "bash.exe"]);
    }

    #[test]
    fn process_info_error_yields_no_names() {
        let mut client =
            replay(r#"{"method":"pane.process_info","response":{"error":{"code":"unsupported"}}}"#);
        assert!(Panes::new(&mut client)
            .foreground_processes("w1:p1")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn edges_decode_flags_and_embedded_layout() {
        let mut client = replay(
            r#"{"method":"pane.edges","response":{"result":{"edges":{"pane_id":"A","left":true,"right":false,"up":true,"down":true,"layout":{"zoomed":false,"panes":[{"pane_id":"A","rect":{"x":0,"y":0,"width":40,"height":20}},{"pane_id":"B","rect":{"x":40,"y":0,"width":40,"height":20}}]}}}}}"#,
        );
        let edges = Panes::new(&mut client).edges("A").unwrap().unwrap();
        assert!(edges.left && edges.up && edges.down);
        assert!(!edges.right);
        assert!(!edges.zoomed);
        assert_eq!(edges.layout.panes.len(), 2);
        assert_eq!(edges.layout.panes[1].rect, Rect::new(40, 0, 40, 20));
    }

    #[test]
    fn edges_missing_result_is_none() {
        let mut client = replay(r#"{"method":"pane.edges","response":{"result":{}}}"#);
        assert!(Panes::new(&mut client).edges("A").unwrap().is_none());
    }

    #[test]
    fn layout_decodes_zoom_and_empty_panes() {
        let mut client = replay(
            r#"{"method":"pane.layout","response":{"result":{"layout":{"zoomed":true,"panes":[]}}}}"#,
        );
        let layout = Panes::new(&mut client).layout().unwrap();
        assert!(layout.zoomed);
        assert!(layout.panes.is_empty());
    }

    #[test]
    fn zoom_off_sends_the_off_mode() {
        let mut client = replay(
            r#"{"method":"pane.zoom","response":{"result":{"pane":{"pane_id":"A","zoomed":false}}}}"#,
        );
        Panes::new(&mut client).zoom_off("A").unwrap();
        let (method, params) = &client.calls()[0];
        assert_eq!(method, "pane.zoom");
        assert_eq!(params.get("mode").and_then(Value::as_str), Some("off"));
    }

    #[test]
    fn resize_sends_the_amount_as_a_float() {
        let mut client = replay(r#"{"method":"pane.resize","response":{"result":{}}}"#);
        Panes::new(&mut client).resize("A", "left", 0.03).unwrap();
        let (_, params) = &client.calls()[0];
        assert!(
            params.to_json().contains(r#""amount":0.03"#),
            "{}",
            params.to_json()
        );
    }

    #[test]
    fn send_keys_wraps_the_chord_in_an_array() {
        let mut client = replay(r#"{"method":"pane.send_keys","response":{"result":{}}}"#);
        Panes::new(&mut client).send_keys("A", &["ctrl+h"]).unwrap();
        let (_, params) = &client.calls()[0];
        assert_eq!(params.to_json(), r#"{"pane_id":"A","keys":["ctrl+h"]}"#);
    }
}
