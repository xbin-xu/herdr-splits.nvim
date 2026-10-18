//! Herdr control-socket RPC: one JSON request per connection, one reply line.
//!
//! Transport is OS-specific but the protocol is not:
//!
//! * Unix (Linux, macOS): a Unix domain socket at `HERDR_SOCKET_PATH`.
//! * Windows: a named pipe; herdr derives the pipe name by prefixing
//!   `\\.\pipe\` to the same path.
//!
//! The editor side does the same thing over libuv (`lua/herdr-splits/rpc.lua`);
//! this is the Herdr-side counterpart, used instead of spawning the `herdr`
//! CLI because every spawn costs 10–20 ms on Windows.
//!
//! Both a [`Client`] trait and two test doubles are defined here:
//! [`ReplayClient`] answers from a fixture file and [`TraceClient`] records the
//! call sequence, so the decision trees can be tested without a running herdr.

use std::fmt;
use std::io::{self, Read, Write};
use std::time::Duration;

use crate::json::{self, Value, MAX_REPLY_BYTES};

/// Request ids are fixed: herdr echoes them and nothing correlates on them
/// (`herdr_common.py` uses the literal `"req"` for the same reason).
pub const REQUEST_ID: &str = "req";

/// Reply deadline. A hung server must not hang the keypress: the Python client
/// sets a 5 s socket timeout, and the editor side uses 2 s.
pub const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum RpcError {
    /// Connecting failed: no server, permission denied, dead pipe.
    Connect { path: String, source: io::Error },
    /// Write or read failed after the connection was established.
    Transport { source: io::Error },
    /// No newline (or EOF) before the deadline.
    Timeout { stage: &'static str },
    /// The reply was not a JSON object.
    Decode { message: String },
    /// Reply was longer than [`MAX_REPLY_BYTES`].
    TooLarge,
    /// A call failed; carries the method so the message can name it without
    /// every caller re-wrapping (the Python original's tracebacks named the
    /// file, which is not useful in a keypress path).
    Call {
        method: String,
        source: Box<RpcError>,
    },
}

impl fmt::Display for RpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RpcError::Connect { path, source } => {
                write!(formatter, "cannot connect to herdr socket {path}: {source}")
            }
            RpcError::Transport { source } => write!(formatter, "{source}"),
            RpcError::Timeout { stage } => write!(formatter, "timed out while {stage}"),
            RpcError::Decode { message } => write!(formatter, "malformed reply: {message}"),
            RpcError::TooLarge => write!(formatter, "reply exceeded {MAX_REPLY_BYTES} bytes"),
            RpcError::Call { method, source } => write!(formatter, "{method} failed: {source}"),
        }
    }
}

impl std::error::Error for RpcError {}

impl RpcError {
    /// True when the failure happened before a request was delivered, i.e.
    /// "herdr is not reachable" rather than "the call itself went wrong".
    pub fn is_connect(&self) -> bool {
        match self {
            RpcError::Connect { .. } => true,
            RpcError::Call { source, .. } => source.is_connect(),
            _ => false,
        }
    }

    /// The RPC method that failed, if any.
    pub fn method(&self) -> Option<&str> {
        match self {
            RpcError::Call { method, .. } => Some(method),
            _ => None,
        }
    }

    /// Tag the failure with the method that produced it.
    pub fn in_method(self, method: &str) -> RpcError {
        RpcError::Call {
            method: method.to_string(),
            source: Box::new(self),
        }
    }
}

/// One RPC endpoint.
pub trait Client {
    fn call(&mut self, method: &str, params: &Value) -> Result<Value, RpcError>;
}

// ---------------------------------------------------------------------------
// Wire encoding
// ---------------------------------------------------------------------------

fn encode_request(method: &str, params: &Value) -> String {
    let request = Value::Object(vec![
        ("id".to_string(), Value::str(REQUEST_ID)),
        ("method".to_string(), Value::str(method)),
        ("params".to_string(), params.clone()),
    ]);
    let mut payload = request.to_json();
    payload.push('\n');
    payload
}

/// Take the first complete line out of `buffer`, decoding it as a JSON object.
fn decode_reply(buffer: &[u8]) -> Result<Value, RpcError> {
    let line = match buffer.iter().position(|byte| *byte == b'\n') {
        Some(index) => &buffer[..index],
        None => buffer,
    };
    let text = String::from_utf8_lossy(line);
    let value = json::decode(&text).map_err(|err| RpcError::Decode {
        message: err.to_string(),
    })?;
    if !matches!(value, Value::Object(_)) {
        return Err(RpcError::Decode {
            message: "reply is not a JSON object".to_string(),
        });
    }
    Ok(value)
}

// ---------------------------------------------------------------------------
// Real transport
// ---------------------------------------------------------------------------

/// Client that talks to the live herdr server.
pub struct SocketClient {
    path: String,
}

impl SocketClient {
    pub fn new(path: impl Into<String>) -> SocketClient {
        SocketClient { path: path.into() }
    }
}

impl Client for SocketClient {
    fn call(&mut self, method: &str, params: &Value) -> Result<Value, RpcError> {
        let mut transport = connect(&self.path)?;
        self.exchange(&mut *transport, method, params)
            .map_err(|err| err.in_method(method))
    }
}

impl SocketClient {
    /// Send one request and read one reply line over an established transport.
    /// Split out so tests can drive the raw protocol without a server.
    fn exchange(
        &self,
        transport: &mut dyn Transport,
        method: &str,
        params: &Value,
    ) -> Result<Value, RpcError> {
        let payload = encode_request(method, params);
        transport
            .send(payload.as_bytes())
            .map_err(|source| RpcError::Transport { source })?;

        let mut buffer = Vec::with_capacity(512);
        let mut chunk = [0u8; 8192];
        loop {
            if buffer.contains(&b'\n') {
                break;
            }
            if buffer.len() > MAX_REPLY_BYTES {
                return Err(RpcError::TooLarge);
            }
            let read = transport
                .recv(&mut chunk)
                .map_err(|source| RpcError::Transport { source })?;
            if read == 0 {
                // EOF: herdr closes after one reply, so decode what we have.
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
        }
        decode_reply(&buffer)
    }
}

fn connect(path: &str) -> Result<Box<dyn Transport>, RpcError> {
    #[cfg(unix)]
    {
        unix::connect(path).map(|transport| Box::new(transport) as Box<dyn Transport>)
    }
    #[cfg(windows)]
    {
        windows::connect(path).map(|transport| Box::new(transport) as Box<dyn Transport>)
    }
}

trait Transport {
    fn send(&mut self, data: &[u8]) -> io::Result<()>;
    fn recv(&mut self, buffer: &mut [u8]) -> io::Result<usize>;
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::net::UnixStream;

    pub struct UnixTransport {
        stream: UnixStream,
    }

    pub fn connect(path: &str) -> Result<UnixTransport, RpcError> {
        let stream = UnixStream::connect(path).map_err(|source| RpcError::Connect {
            path: path.to_string(),
            source,
        })?;
        stream
            .set_read_timeout(Some(TIMEOUT))
            .map_err(|source| RpcError::Transport { source })?;
        stream
            .set_write_timeout(Some(TIMEOUT))
            .map_err(|source| RpcError::Transport { source })?;
        Ok(UnixTransport { stream })
    }

    impl Transport for UnixTransport {
        fn send(&mut self, data: &[u8]) -> io::Result<()> {
            self.stream.write_all(data)
        }

        fn recv(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            match self.stream.read(buffer) {
                Err(err)
                    if matches!(
                        err.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        RpcError::Timeout {
                            stage: "waiting for a reply",
                        },
                    ))
                }
                other => other,
            }
        }
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
    use std::thread;

    /// Named-pipe transport.
    ///
    /// Windows has no socket read timeout for files, and `CreateFile`'s pipe
    /// mode is fixed by the server, so the reader runs on its own thread and
    /// the caller waits on a channel with a deadline. On timeout the process
    /// exits; the reader thread dies with it.
    pub struct PipeTransport {
        writer: std::fs::File,
        incoming: Receiver<io::Result<Vec<u8>>>,
        buffer: Vec<u8>,
        eof: bool,
    }

    pub fn connect(path: &str) -> Result<PipeTransport, RpcError> {
        let pipe_name = crate::ctx::windows_pipe_name(path);
        let mut writer = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&pipe_name)
            .map_err(|source| RpcError::Connect {
                path: path.to_string(),
                source,
            })?;
        // A second handle on the same pipe would block on read, so hand the
        // original to the reader thread and keep a clone for writing.
        let mut reader = writer
            .try_clone()
            .map_err(|source| RpcError::Transport { source })?;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut chunk = [0u8; 8192];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => {
                        let _ = tx.send(Ok(Vec::new()));
                        break;
                    }
                    Ok(read) => {
                        if tx.send(Ok(chunk[..read].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        let _ = tx.send(Err(err));
                        break;
                    }
                }
            }
        });
        // Touch the writer so the unused-mut lint stays quiet on rebuilds.
        writer
            .flush()
            .map_err(|source| RpcError::Transport { source })?;
        Ok(PipeTransport {
            writer,
            incoming: rx,
            buffer: Vec::new(),
            eof: false,
        })
    }

    impl Transport for PipeTransport {
        fn send(&mut self, data: &[u8]) -> io::Result<()> {
            self.writer.write_all(data)?;
            self.writer.flush()
        }

        fn recv(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if !self.buffer.is_empty() {
                let take = self.buffer.len().min(buffer.len());
                buffer[..take].copy_from_slice(&self.buffer[..take]);
                self.buffer.drain(..take);
                return Ok(take);
            }
            if self.eof {
                return Ok(0);
            }
            match self.incoming.recv_timeout(TIMEOUT) {
                Ok(Ok(chunk)) => {
                    if chunk.is_empty() {
                        self.eof = true;
                        return Ok(0);
                    }
                    let take = chunk.len().min(buffer.len());
                    buffer[..take].copy_from_slice(&chunk[..take]);
                    if take < chunk.len() {
                        self.buffer.extend_from_slice(&chunk[take..]);
                    }
                    Ok(take)
                }
                Ok(Err(err)) => Err(err),
                Err(RecvTimeoutError::Timeout) => Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    RpcError::Timeout {
                        stage: "waiting for a reply",
                    },
                )),
                Err(RecvTimeoutError::Disconnected) => Ok(0),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Test doubles
// ---------------------------------------------------------------------------

/// One scripted reply. Fixtures are JSON Lines objects:
///
/// ```json
/// {"method":"pane.get","response":{"result":{"pane":{"pane_id":"w1:p1"}}}}
/// ```
///
/// `response` is returned as-is; `"error":"<text>"` instead makes the call fail
/// with a transport error, and `"connect_error":"<text>"` makes the call fail
/// as if the socket were unreachable.
#[derive(Debug, Clone)]
pub struct ReplayStep {
    pub method: String,
    pub params: Option<Value>,
    response: ReplayResponse,
}

#[derive(Debug, Clone)]
enum ReplayResponse {
    Reply(Value),
    Transport(String),
    Connect(String),
}

/// Answers calls from a fixture list, one reply per call, in order.
///
/// Each call consumes the first unconsumed step whose method matches, which is
/// how the Python tests' `side_effect` lists behave for the mixed sequences
/// (`pane.edges` appearing twice in one run, for instance). A step may also
/// name the `params` it expects; a mismatch fails the call so a fixture pins
/// the request shape, not just the call order.
pub struct ReplayClient {
    steps: Vec<(ReplayStep, bool)>,
    calls: Vec<(String, Value)>,
}

impl ReplayClient {
    pub fn from_json_lines(text: &str) -> Result<ReplayClient, String> {
        let mut steps = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let value =
                json::decode(line).map_err(|err| format!("fixture line {}: {err}", index + 1))?;
            let method = value
                .get("method")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("fixture line {}: missing \"method\"", index + 1))?
                .to_string();
            let params = value.get("params").cloned();
            let response =
                if let Some(message) = value.get("connect_error").and_then(Value::as_str) {
                    ReplayResponse::Connect(message.to_string())
                } else if let Some(message) = value.get("error").and_then(Value::as_str) {
                    ReplayResponse::Transport(message.to_string())
                } else {
                    ReplayResponse::Reply(value.get("response").cloned().ok_or_else(|| {
                        format!("fixture line {}: missing \"response\"", index + 1)
                    })?)
                };
            steps.push((
                ReplayStep {
                    method,
                    params,
                    response,
                },
                false,
            ));
        }
        Ok(ReplayClient {
            steps,
            calls: Vec::new(),
        })
    }

    /// Methods called so far, in order (used by tests for extra assertions).
    #[cfg(test)]
    pub fn calls(&self) -> &[(String, Value)] {
        &self.calls
    }
}

impl Client for ReplayClient {
    fn call(&mut self, method: &str, params: &Value) -> Result<Value, RpcError> {
        self.calls.push((method.to_string(), params.clone()));
        let index = self
            .steps
            .iter()
            .position(|(step, used)| !used && step.method == method)
            .ok_or_else(|| RpcError::Decode {
                message: format!("fixture has no reply for {method}"),
            })?;
        self.steps[index].1 = true;
        let step = &self.steps[index].0;
        if let Some(expected) = &step.params {
            if !expected.json_eq(params) {
                return Err(RpcError::Decode {
                    message: format!(
                        "fixture expected params {} for {method}, got {}",
                        expected.to_json(),
                        params.to_json()
                    ),
                }
                .in_method(method));
            }
        }
        match &step.response {
            ReplayResponse::Reply(value) => Ok(value.clone()),
            ReplayResponse::Transport(message) => Err(RpcError::Transport {
                source: io::Error::other(message.clone()),
            }),
            ReplayResponse::Connect(message) => Err(RpcError::Connect {
                path: "replay".to_string(),
                source: io::Error::other(message.clone()),
            }),
        }
    }
}

/// Records every call as `method<TAB>params-json<TAB>ok|error` and forwards to
/// an inner client (or nothing at all, when `--trace` is combined with
/// `--replay`).
pub struct TraceClient<'a> {
    file: std::fs::File,
    inner: Box<dyn Client + 'a>,
}

impl<'a> TraceClient<'a> {
    pub fn new(path: &std::path::Path, inner: Box<dyn Client + 'a>) -> io::Result<TraceClient<'a>> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(TraceClient { file, inner })
    }
}

impl Client for TraceClient<'_> {
    fn call(&mut self, method: &str, params: &Value) -> Result<Value, RpcError> {
        let outcome = self.inner.call(method, params);
        let status = if outcome.is_ok() { "ok" } else { "error" };
        // Best effort: a broken trace file must never break navigation.
        let _ = writeln!(self.file, "{method}\t{}\t{status}", params.to_json());
        let _ = self.file.flush();
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_the_documented_request_shape() {
        let payload = encode_request(
            "pane.get",
            &Value::object(vec![("pane_id", Value::str("w1:p1"))]),
        );
        assert_eq!(
            payload,
            "{\"id\":\"req\",\"method\":\"pane.get\",\"params\":{\"pane_id\":\"w1:p1\"}}\n"
        );
    }

    #[test]
    fn encodes_an_empty_params_object() {
        let payload = encode_request("pane.layout", &Value::Object(Vec::new()));
        assert_eq!(
            payload,
            "{\"id\":\"req\",\"method\":\"pane.layout\",\"params\":{}}\n"
        );
    }

    #[test]
    fn decodes_the_first_line_and_ignores_the_rest() {
        let reply = decode_reply(b"{\"result\":{}}\n{\"ignored\":true}\n").unwrap();
        assert!(reply.get("result").is_some());
    }

    #[test]
    fn decodes_a_reply_without_a_trailing_newline() {
        let reply = decode_reply(b"{\"result\":{\"pane\":{}}}").unwrap();
        assert!(reply.path(&["result", "pane"]).is_some());
    }

    #[test]
    fn rejects_a_non_object_reply() {
        assert!(decode_reply(b"[1,2]\n").is_err());
        assert!(decode_reply(b"not json\n").is_err());
    }

    #[test]
    fn replay_consumes_matching_steps_in_order() {
        let fixture = r#"
            {"method":"pane.get","response":{"result":{"pane":{"pane_id":"A"}}}}
            {"method":"pane.edges","response":{"result":{"edges":{"left":true}}}}
            {"method":"pane.edges","response":{"result":{"edges":{"left":false}}}}
        "#;
        let mut client = ReplayClient::from_json_lines(fixture).unwrap();
        let first = client
            .call("pane.edges", &Value::Object(Vec::new()))
            .unwrap();
        assert_eq!(
            first
                .path(&["result", "edges", "left"])
                .and_then(Value::as_bool),
            Some(true)
        );
        let second = client
            .call("pane.edges", &Value::Object(Vec::new()))
            .unwrap();
        assert_eq!(
            second
                .path(&["result", "edges", "left"])
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(client.calls().len(), 2);
    }

    #[test]
    fn replay_reports_an_unscripted_method() {
        let mut client =
            ReplayClient::from_json_lines(r#"{"method":"pane.get","response":{"result":{}}}"#)
                .unwrap();
        let error = client
            .call("pane.zoom", &Value::Object(Vec::new()))
            .unwrap_err();
        assert!(error.to_string().contains("pane.zoom"));
    }

    #[test]
    fn replay_simulates_transport_and_connect_failures() {
        let fixture = r#"
            {"method":"pane.edges","error":"一"}
            {"method":"pane.get","connect_error":"denied"}
        "#;
        let mut client = ReplayClient::from_json_lines(fixture).unwrap();
        let transport = client
            .call("pane.edges", &Value::Object(Vec::new()))
            .unwrap_err();
        assert!(!transport.is_connect());
        let connect = client
            .call("pane.get", &Value::Object(Vec::new()))
            .unwrap_err();
        assert!(connect.is_connect());
    }

    #[test]
    fn trace_records_methods_and_outcomes() {
        let path = crate::testtmp::file("trace", "calls.log");
        {
            let fixture = r#"{"method":"pane.get","response":{"result":{}}}"#;
            let replay = ReplayClient::from_json_lines(fixture).unwrap();
            let mut trace = TraceClient::new(&path, Box::new(replay)).unwrap();
            trace
                .call(
                    "pane.get",
                    &Value::object(vec![("pane_id", Value::str("A"))]),
                )
                .unwrap();
        }
        let recorded = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(recorded, "pane.get\t{\"pane_id\":\"A\"}\tok\n");
    }

    #[cfg(unix)]
    #[test]
    fn unix_client_reports_a_missing_socket_as_a_connect_error() {
        let mut client = SocketClient::new("/nonexistent/herdr.sock");
        let error = client
            .call("pane.get", &Value::Object(Vec::new()))
            .unwrap_err();
        assert!(error.is_connect(), "{error}");
        // The message names the path, like the Python original.
        assert!(
            error.to_string().contains("/nonexistent/herdr.sock"),
            "{error}"
        );
    }

    /// A transport that serves one canned reply, so the wire protocol can be
    /// exercised without herdr.
    struct ScriptedTransport {
        sent: Vec<u8>,
        reply: Vec<u8>,
        served: bool,
    }

    impl Transport for ScriptedTransport {
        fn send(&mut self, data: &[u8]) -> std::io::Result<()> {
            self.sent.extend_from_slice(data);
            Ok(())
        }

        fn recv(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if self.served {
                return Ok(0);
            }
            self.served = true;
            let take = self.reply.len().min(buffer.len());
            buffer[..take].copy_from_slice(&self.reply[..take]);
            Ok(take)
        }
    }

    #[test]
    fn exchange_writes_the_request_and_decodes_the_reply() {
        let client = SocketClient::new("unused");
        let mut transport = ScriptedTransport {
            sent: Vec::new(),
            reply: b"{\"id\":\"req\",\"result\":{\"pane\":{\"pane_id\":\"w1:p1\"}}}\n".to_vec(),
            served: false,
        };
        let reply = client
            .exchange(
                &mut transport,
                "pane.get",
                &Value::object(vec![("pane_id", Value::str("w1:p1"))]),
            )
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&transport.sent),
            "{\"id\":\"req\",\"method\":\"pane.get\",\"params\":{\"pane_id\":\"w1:p1\"}}\n"
        );
        assert_eq!(
            reply
                .path(&["result", "pane", "pane_id"])
                .and_then(Value::as_str),
            Some("w1:p1")
        );
    }

    #[test]
    fn exchange_survives_a_reply_split_across_reads() {
        let client = SocketClient::new("unused");
        let mut transport = ScriptedTransport {
            sent: Vec::new(),
            // No trailing newline: the reader must stop at EOF instead.
            reply: b"{\"result\":{}}".to_vec(),
            served: false,
        };
        assert!(client
            .exchange(&mut transport, "pane.layout", &Value::Object(Vec::new()))
            .is_ok());
    }

    #[test]
    fn method_tagging_names_the_failing_call() {
        let error = RpcError::Transport {
            source: std::io::Error::other("boom"),
        }
        .in_method("pane.zoom");
        assert_eq!(error.method(), Some("pane.zoom"));
        assert!(error.to_string().contains("pane.zoom"));
        assert!(error.to_string().contains("boom"));
        assert!(!error.is_connect());
    }
}
