//! Tiny JSON value type, encoder and decoder.
//!
//! The client sends one small request object and decodes one reply line, so a
//! full serde-class implementation would be dead weight: this crate has no
//! dependencies on purpose (Herdr builds plugins offline on the installer's
//! machine). Behaviour mirrors `scripts/herdr_common.py`:
//!
//! * floats encode like Python's `repr` (`0.03`, not `0.029999999999999999`),
//! * object keys keep their insertion order,
//! * decoding stops after the first complete value (the reply is NDJSON),
//! * `\uXXXX` escapes and surrogate pairs decode to real characters.

use std::fmt::Write as _;

/// Maximum reply we are willing to buffer. A herdr reply for these calls is a
/// few hundred bytes; anything larger is garbage and treated as "no reply".
pub const MAX_REPLY_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    /// Insertion-ordered object. `BTreeMap` would sort keys; herdr does not
    /// care about request key order, but the trace fixtures diff the exact
    /// bytes, so keep the order the caller wrote.
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn object(pairs: Vec<(&str, Value)>) -> Value {
        Value::Object(
            pairs
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        )
    }

    pub fn str(value: impl Into<String>) -> Value {
        Value::Str(value.into())
    }

    /// Look up an object member. Returns `None` for non-objects and for
    /// missing keys, so callers can chain `get(...).and_then(...)`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(pairs) => pairs
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// Follow a path of object keys.
    pub fn path(&self, keys: &[&str]) -> Option<&Value> {
        let mut current = self;
        for key in keys {
            current = current.get(key)?;
        }
        Some(current)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(flag) => Some(*flag),
            _ => None,
        }
    }

    /// Numeric view, used by the decoder tests (production code reads the
    /// fields it needs through `as_i64`/`as_bool`).
    #[cfg(test)]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(number) => Some(*number as f64),
            Value::Float(number) => Some(*number),
            _ => None,
        }
    }

    /// Structural equality that ignores object key order. Used by the replay
    /// fixtures, which state the parameters they expect.
    pub fn json_eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) | (Value::Float(b), Value::Int(a)) => *a as f64 == *b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.json_eq(y))
            }
            (Value::Object(a), Value::Object(b)) => {
                a.len() == b.len()
                    && a.iter().all(|(key, value)| {
                        b.iter()
                            .find(|(other_key, _)| other_key == key)
                            .is_some_and(|(_, other_value)| value.json_eq(other_value))
                    })
            }
            _ => false,
        }
    }

    /// A JSON integer as `i64`, also accepting an integral float.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(number) => Some(*number),
            Value::Float(number) if number.fract() == 0.0 => Some(*number as i64),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    /// Truthiness used by the Python original for `layout.zoomed` and
    /// friends: only an explicit `false`/missing/none is falsy.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(flag) => *flag,
            Value::Int(number) => *number != 0,
            Value::Float(number) => *number != 0.0,
            Value::Str(text) => !text.is_empty(),
            Value::Array(items) => !items.is_empty(),
            Value::Object(pairs) => !pairs.is_empty(),
        }
    }
}

// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------

impl Value {
    pub fn to_json(&self) -> String {
        let mut out = String::with_capacity(128);
        self.write_json(&mut out);
        out
    }

    fn write_json(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(true) => out.push_str("true"),
            Value::Bool(false) => out.push_str("false"),
            Value::Int(number) => {
                let _ = write!(out, "{number}");
            }
            Value::Float(number) => out.push_str(&format_float(*number)),
            Value::Str(text) => write_json_string(text, out),
            Value::Array(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    item.write_json(out);
                }
                out.push(']');
            }
            Value::Object(pairs) => {
                out.push('{');
                for (index, (key, value)) in pairs.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    write_json_string(key, out);
                    out.push(':');
                    value.write_json(out);
                }
                out.push('}');
            }
        }
    }
}

/// Render a float the way Python's `repr` does for the values this client
/// sends: the shortest form that round-trips, so `0.03` stays `0.03` and an
/// integral value keeps its `.0` (`3.0`, matching `repr(3.0)`).
///
/// Rust's `Debug` for `f64` is exactly that shortest round-trip form, so both
/// branches below are the same call; the guard exists only to document why the
/// output matches the Python client byte for byte.
fn format_float(number: f64) -> String {
    if !number.is_finite() {
        // JSON has no NaN/Infinity; the parameters we send never carry them.
        return "null".to_string();
    }
    format!("{number:?}")
}

fn write_json_string(text: &str, out: &mut String) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            control if (control as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", control as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

// ---------------------------------------------------------------------------
// Decoder
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct DecodeError {
    pub message: String,
    pub offset: usize,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for DecodeError {}

fn error<T>(message: impl Into<String>, offset: usize) -> Result<T, DecodeError> {
    Err(DecodeError {
        message: message.into(),
        offset,
    })
}

/// Decode the first JSON value in `text`, ignoring anything after it.
pub fn decode(text: &str) -> Result<Value, DecodeError> {
    let bytes = text.as_bytes();
    // Tolerate a UTF-8 BOM: herdr does not send one, hand-made fixtures might.
    let start = if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        3
    } else {
        0
    };
    let mut parser = Parser {
        text,
        bytes,
        offset: start,
    };
    let value = parser.parse_value()?;
    Ok(value)
}

struct Parser<'a> {
    text: &'a str,
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Parser<'a> {
    fn skip_whitespace(&mut self) {
        while let Some(byte) = self.bytes.get(self.offset) {
            match byte {
                b' ' | b'\t' | b'\n' | b'\r' => self.offset += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }

    fn parse_value(&mut self) -> Result<Value, DecodeError> {
        self.skip_whitespace();
        match self.peek() {
            None => error("unexpected end of JSON", self.offset),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => self.parse_string().map(Value::Str),
            Some(b't') => self.parse_literal("true", Value::Bool(true)),
            Some(b'f') => self.parse_literal("false", Value::Bool(false)),
            Some(b'n') => self.parse_literal("null", Value::Null),
            Some(_) => self.parse_number(),
        }
    }

    fn parse_literal(&mut self, literal: &str, value: Value) -> Result<Value, DecodeError> {
        if self.bytes[self.offset..].starts_with(literal.as_bytes()) {
            self.offset += literal.len();
            Ok(value)
        } else {
            error("invalid JSON literal", self.offset)
        }
    }

    fn parse_object(&mut self) -> Result<Value, DecodeError> {
        self.offset += 1; // '{'
        let mut pairs: Vec<(String, Value)> = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.offset += 1;
            return Ok(Value::Object(pairs));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return error("bad JSON object key", self.offset);
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return error("bad JSON object separator", self.offset);
            }
            self.offset += 1;
            let value = self.parse_value()?;
            // Last duplicate key wins, matching Python's `dict` assignment.
            if let Some(existing) = pairs.iter_mut().find(|(name, _)| name == &key) {
                existing.1 = value;
            } else {
                pairs.push((key, value));
            }
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.offset += 1;
                }
                Some(b'}') => {
                    self.offset += 1;
                    return Ok(Value::Object(pairs));
                }
                None => return error("unterminated JSON object", self.offset),
                _ => return error("bad JSON object", self.offset),
            }
        }
    }

    fn parse_array(&mut self) -> Result<Value, DecodeError> {
        self.offset += 1; // '['
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.offset += 1;
            return Ok(Value::Array(items));
        }
        loop {
            let value = self.parse_value()?;
            items.push(value);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.offset += 1;
                }
                Some(b']') => {
                    self.offset += 1;
                    return Ok(Value::Array(items));
                }
                None => return error("unterminated JSON array", self.offset),
                _ => return error("bad JSON array", self.offset),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, DecodeError> {
        self.offset += 1; // opening quote
        let start = self.offset;
        // Fast path: no escapes.
        let mut scan = self.offset;
        while let Some(byte) = self.bytes.get(scan) {
            match byte {
                b'"' => {
                    let text = self.text[start..scan].to_string();
                    self.offset = scan + 1;
                    return Ok(text);
                }
                b'\\' => break,
                _ => scan += 1,
            }
        }

        let mut out = String::with_capacity(scan.saturating_sub(start) + 8);
        out.push_str(&self.text[start..scan]);
        self.offset = scan;
        loop {
            let Some(byte) = self.peek() else {
                return error("unterminated JSON string", self.offset);
            };
            match byte {
                b'"' => {
                    self.offset += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.offset += 1;
                    self.parse_escape(&mut out)?;
                }
                _ => {
                    let rest = &self.text[self.offset..];
                    let character = rest.chars().next().ok_or_else(|| DecodeError {
                        message: "invalid UTF-8 in JSON string".to_string(),
                        offset: self.offset,
                    })?;
                    out.push(character);
                    self.offset += character.len_utf8();
                }
            }
        }
    }

    fn parse_escape(&mut self, out: &mut String) -> Result<(), DecodeError> {
        let Some(byte) = self.peek() else {
            return error("unterminated JSON escape", self.offset);
        };
        self.offset += 1;
        match byte {
            b'"' => out.push('"'),
            b'\\' => out.push('\\'),
            b'/' => out.push('/'),
            b'b' => out.push('\u{08}'),
            b'f' => out.push('\u{0c}'),
            b'n' => out.push('\n'),
            b'r' => out.push('\r'),
            b't' => out.push('\t'),
            b'u' => {
                let high = self.parse_hex4()?;
                if (0xd800..=0xdbff).contains(&high) {
                    // A surrogate pair: the low half must follow.
                    if self.bytes[self.offset..].starts_with(b"\\u") {
                        let saved = self.offset;
                        self.offset += 2;
                        let low = self.parse_hex4()?;
                        if (0xdc00..=0xdfff).contains(&low) {
                            let code = 0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00);
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                            return Ok(());
                        }
                        // Not a low surrogate: rewind and emit the high half
                        // alone, matching Python's permissive behaviour.
                        self.offset = saved;
                    }
                    out.push(char::from_u32(high).unwrap_or('\u{fffd}'));
                } else {
                    out.push(char::from_u32(high).unwrap_or('\u{fffd}'));
                }
            }
            other => {
                // Python falls back to the raw escaped character.
                out.push(other as char);
            }
        }
        Ok(())
    }

    fn parse_hex4(&mut self) -> Result<u32, DecodeError> {
        if self.offset + 4 > self.bytes.len() {
            return error("truncated \\u escape", self.offset);
        }
        let slice = &self.text[self.offset..self.offset + 4];
        let value = u32::from_str_radix(slice, 16).map_err(|_| DecodeError {
            message: "invalid \\u escape".to_string(),
            offset: self.offset,
        })?;
        self.offset += 4;
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<Value, DecodeError> {
        let start = self.offset;
        while let Some(byte) = self.peek() {
            match byte {
                b'+' | b'-' | b'.' | b'e' | b'E' | b'0'..=b'9' => self.offset += 1,
                _ => break,
            }
        }
        let token = &self.text[start..self.offset];
        if token.is_empty() {
            return error("bad JSON number", start);
        }
        let fractional = token.contains('.') || token.contains('e') || token.contains('E');
        if !fractional {
            if let Ok(number) = token.parse::<i64>() {
                return Ok(Value::Int(number));
            }
        }
        token
            .parse::<f64>()
            .map(Value::Float)
            .map_err(|_| DecodeError {
                message: format!("bad JSON number {token:?}"),
                offset: start,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Payload shapes copied from `herdr-splits-slim/bench/equiv.py`, which is
    /// the equivalence harness the Python side already trusts.
    const PAYLOADS: &[&str] = &[
        r#"{"id":"req","result":{"pane":{"pane_id":"w1:p1","tokens":{"editor":"nvim"},"terminal_title_stripped":"x [herdr-splits] y"}}}"#,
        r#"{"id":"req","result":{"edges":{"left":true,"right":false,"up":true,"down":true,"pane_id":"A","layout":{"zoomed":false,"panes":[{"pane_id":"A","rect":{"x":0,"y":0,"width":40,"height":20}},{"pane_id":"B","rect":{"x":40,"y":0,"width":40,"height":20}}]}}}}"#,
        r#"{"error":{"code":"pane_not_found","message":"pane w1:p9 not found"},"id":"req"}"#,
        r#"{"result":{"layout":{"zoomed":true,"panes":[]}}}"#,
        r#"{"a":null,"b":-1.5e3,"c":0,"d":"","e":[],"f":{},"g":[[1,2],[3]]}"#,
        r#"{"unicode":"\u4e2d\u6587","esc":"a\"b\\c\nd","sur":"\ud83d\ude00"}"#,
        r#"{"process_info":{"foreground_processes":[{"name":"nvim.exe","pid":123,"argv":["nvim","f"],"cwd":null}]}}"#,
    ];

    #[test]
    fn decodes_herdr_reply_shapes() {
        for payload in PAYLOADS {
            let value = decode(payload).unwrap_or_else(|err| panic!("{payload}: {err}"));
            assert!(matches!(value, Value::Object(_)), "{payload}");
        }
    }

    #[test]
    fn decodes_unicode_escapes_and_surrogate_pairs() {
        let value = decode(PAYLOADS[5]).unwrap();
        assert_eq!(value.get("unicode").and_then(Value::as_str), Some("中文"));
        assert_eq!(value.get("esc").and_then(Value::as_str), Some("a\"b\\c\nd"));
        assert_eq!(value.get("sur").and_then(Value::as_str), Some("😀"));
    }

    #[test]
    fn keeps_integers_and_floats_apart() {
        let value =
            decode(r#"{"a":null,"b":-1.5e3,"c":0,"d":"","e":[],"f":{},"g":[[1,2],[3]]}"#).unwrap();
        assert_eq!(value.get("c").and_then(Value::as_i64), Some(0));
        assert_eq!(value.get("b").and_then(Value::as_f64), Some(-1500.0));
        assert_eq!(
            value.get("e").and_then(Value::as_array).map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn booleans_and_null_round_trip() {
        let value = decode(r#"{"t":true,"f":false,"n":null}"#).unwrap();
        assert_eq!(value.get("t").and_then(Value::as_bool), Some(true));
        assert_eq!(value.get("f").and_then(Value::as_bool), Some(false));
        assert!(matches!(value.get("n"), Some(Value::Null)));
    }

    #[test]
    fn duplicate_keys_keep_the_last_value() {
        let value = decode(r#"{"x":1,"x":2}"#).unwrap();
        assert_eq!(value.get("x").and_then(Value::as_i64), Some(2));
    }

    #[test]
    fn encodes_the_request_shapes_the_client_sends() {
        let request = Value::object(vec![
            ("id", Value::str("req")),
            ("method", Value::str("pane.get")),
            (
                "params",
                Value::object(vec![("pane_id", Value::str("w1:p1"))]),
            ),
        ]);
        assert_eq!(
            request.to_json(),
            r#"{"id":"req","method":"pane.get","params":{"pane_id":"w1:p1"}}"#
        );
    }

    #[test]
    fn encodes_resize_amount_like_python_repr() {
        let request = Value::object(vec![
            ("id", Value::str("req")),
            ("method", Value::str("pane.resize")),
            (
                "params",
                Value::object(vec![
                    ("pane_id", Value::str("w1:p1")),
                    ("direction", Value::str("left")),
                    ("amount", Value::Float(0.03)),
                ]),
            ),
        ]);
        assert!(
            request.to_json().contains(r#""amount":0.03}"#),
            "{}",
            request.to_json()
        );
    }

    #[test]
    fn encodes_escapes_control_characters() {
        let value = Value::str("a\"b\\c\nd\te\u{1}");
        assert_eq!(value.to_json(), r#""a\"b\\c\nd\te\u0001""#);
    }

    #[test]
    fn encode_decode_round_trip() {
        let cases = [
            Value::Null,
            Value::Bool(true),
            Value::Int(-7),
            Value::Float(1.5),
            Value::str("中文 ✓"),
            Value::Array(vec![Value::Int(1), Value::str("x"), Value::Null]),
            Value::object(vec![("k", Value::Int(1))]),
        ];
        for case in cases {
            let text = case.to_json();
            let decoded = decode(&text).unwrap_or_else(|err| panic!("{text}: {err}"));
            assert_eq!(decoded, case);
        }
    }

    #[test]
    fn rejects_garbage_without_panicking() {
        for bad in ["", "{", "[1,", r#"{"a":}"#, "\u{ef}\u{bb}\u{bf}"] {
            let result = decode(bad);
            assert!(result.is_err() || bad == "\u{ef}\u{bb}\u{bf}", "{bad:?}");
        }
    }

    #[test]
    fn tolerates_trailing_ndjson_newline() {
        let value = decode("{\"a\":1}\n{\"b\":2}\n").unwrap();
        assert_eq!(value.get("a").and_then(Value::as_i64), Some(1));
    }

    #[test]
    fn path_walks_nested_objects() {
        let value = decode(PAYLOADS[1]).unwrap();
        let zoomed = value.path(&["result", "edges", "layout", "zoomed"]);
        assert_eq!(zoomed.and_then(Value::as_bool), Some(false));
        assert!(value.path(&["result", "edges", "missing"]).is_none());
        assert!(value.path(&["left"]).is_none());
    }

    #[test]
    fn json_eq_ignores_object_key_order() {
        let left = decode(r#"{"a":1,"b":[1,2],"c":{"x":true}}"#).unwrap();
        let right = decode(r#"{"c":{"x":true},"b":[1,2],"a":1}"#).unwrap();
        assert!(left.json_eq(&right));
        let different = decode(r#"{"a":1,"b":[1,2],"c":{"x":false}}"#).unwrap();
        assert!(!left.json_eq(&different));
        assert!(!left.json_eq(&Value::Null));
    }

    #[test]
    fn json_eq_treats_integral_float_as_equal() {
        assert!(decode("1").unwrap().json_eq(&decode("1.0").unwrap()));
    }
}
