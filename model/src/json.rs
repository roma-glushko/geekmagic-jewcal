//! A streaming JSON splitter. It follows the nesting of a document arriving in
//! chunks of any size and hands back the raw text of the values the caller
//! selects (e.g. each element of `items`), so each one can be deserialized on
//! its own with `serde_json::from_slice`. Nothing else is kept in memory.
//!
//! `serde_json` can only stream from `std::io::Read`, which the ESP8266
//! (`no_std`) does not have; a 40 KB response does not fit its heap at once.
//! The splitter trusts the input to be JSON: it checks nesting, not grammar,
//! and the selected pieces are validated by serde.

use crate::prelude::*;

/// Containers deeper than this make the document invalid (Hebcal: 4).
pub const MAX_DEPTH: usize = 8;
/// Longer keys are kept truncated and never match a selection.
const KEY_MAX: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Frame {
    /// `expect_key`: after `{` or `,`, before the member's key.
    Object {
        expect_key: bool,
    },
    Array,
}

/// One step of a [`Path`] pattern.
#[derive(Clone, Copy, Debug)]
pub enum Seg<'a> {
    /// The member with this key, inside an object.
    Key(&'a str),
    /// Any element of an array.
    Elem,
}

/// Where a value sits: the members and elements that lead to it.
pub struct Path<'a> {
    frames: &'a [Frame],
    keys: &'a [[u8; KEY_MAX]],
    key_lens: &'a [u8],
}

impl Path<'_> {
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Whether the value is exactly at `pattern` (one segment per level).
    pub fn is(&self, pattern: &[Seg]) -> bool {
        pattern.len() == self.frames.len()
            && pattern.iter().enumerate().all(|(i, seg)| match (seg, self.frames[i]) {
                (Seg::Elem, Frame::Array) => true,
                (Seg::Key(k), Frame::Object { .. }) => {
                    let len = self.key_lens[i] as usize;
                    len <= KEY_MAX && &self.keys[i][..len] == k.as_bytes()
                }
                _ => false,
            })
    }
}

/// Receives the selected values.
pub trait Sink {
    /// Called as each value starts (outside an already selected value):
    /// `Some(tag)` captures it, and the tag comes back with its text.
    fn select(&mut self, path: &Path) -> Option<u8>;
    /// The complete text of a selected value, or `None` when it was longer
    /// than the splitter's limit and has been skipped.
    fn value(&mut self, tag: u8, raw: Option<&[u8]>);
}

pub struct Splitter {
    frames: [Frame; MAX_DEPTH],
    keys: [[u8; KEY_MAX]; MAX_DEPTH],
    key_lens: [u8; MAX_DEPTH],
    depth: usize,
    in_str: bool,
    escaped: bool,
    str_is_key: bool,
    in_scalar: bool,
    /// Tag and depth of the value being captured.
    capture: Option<(u8, usize)>,
    buf: Vec<u8>,
    limit: usize,
    overflow: bool,
    /// A top-level value has been seen (and possibly finished).
    started: bool,
    error: Option<&'static str>,
}

impl Splitter {
    /// `limit`: the longest value that is captured; longer ones are skipped.
    pub fn new(limit: usize) -> Self {
        Self {
            frames: [Frame::Array; MAX_DEPTH],
            keys: [[0; KEY_MAX]; MAX_DEPTH],
            key_lens: [0; MAX_DEPTH],
            depth: 0,
            in_str: false,
            escaped: false,
            str_is_key: false,
            in_scalar: false,
            capture: None,
            buf: Vec::new(),
            limit,
            overflow: false,
            started: false,
            error: None,
        }
    }

    pub fn feed(&mut self, chunk: &[u8], sink: &mut impl Sink) {
        for &b in chunk {
            if self.error.is_some() {
                return;
            }
            self.byte(b, sink);
        }
    }

    /// Check that the document ended where it should.
    pub fn finish(&mut self, sink: &mut impl Sink) -> Result<(), &'static str> {
        if self.in_scalar {
            // A bare top-level number or literal ends with the input.
            self.in_scalar = false;
            self.end_value(sink);
        }
        if let Some(e) = self.error {
            return Err(e);
        }
        if !self.started || self.depth > 0 || self.in_str {
            return Err("truncated JSON");
        }
        Ok(())
    }

    fn byte(&mut self, b: u8, sink: &mut impl Sink) {
        if self.in_str {
            self.push(b);
            if self.escaped {
                self.escaped = false;
                self.key_byte(b);
            } else if b == b'\\' {
                self.escaped = true;
            } else if b == b'"' {
                self.in_str = false;
                if !self.str_is_key {
                    self.end_value(sink);
                }
            } else {
                self.key_byte(b);
            }
            return;
        }
        if self.in_scalar {
            if !matches!(b, b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r') {
                self.push(b);
                return;
            }
            self.in_scalar = false;
            self.end_value(sink);
        }
        match b {
            b' ' | b'\t' | b'\n' | b'\r' => self.push(b),
            b'"' if matches!(self.top(), Some(Frame::Object { expect_key: true })) => {
                self.push(b);
                self.in_str = true;
                self.str_is_key = true;
                self.key_lens[self.depth - 1] = 0;
            }
            b'"' => {
                self.begin_value(sink);
                self.push(b);
                self.in_str = true;
                self.str_is_key = false;
            }
            b'{' | b'[' => {
                self.begin_value(sink);
                self.push(b);
                if self.depth == MAX_DEPTH {
                    self.error = Some("JSON nested too deeply");
                    return;
                }
                self.frames[self.depth] = if b == b'{' { Frame::Object { expect_key: true } } else { Frame::Array };
                self.key_lens[self.depth] = 0;
                self.depth += 1;
            }
            b'}' | b']' => {
                self.push(b);
                let matches = match self.top() {
                    Some(Frame::Object { .. }) => b == b'}',
                    Some(Frame::Array) => b == b']',
                    None => false,
                };
                if !matches {
                    self.error = Some("unbalanced JSON");
                    return;
                }
                self.depth -= 1;
                self.end_value(sink);
            }
            b':' => {
                self.push(b);
                if let Some(Frame::Object { expect_key }) = self.frames[..self.depth].last_mut() {
                    *expect_key = false;
                }
            }
            b',' => {
                self.push(b);
                if let Some(Frame::Object { expect_key }) = self.frames[..self.depth].last_mut() {
                    *expect_key = true;
                }
            }
            _ => {
                self.begin_value(sink);
                self.push(b);
                self.in_scalar = true;
            }
        }
    }

    fn top(&self) -> Option<Frame> {
        self.depth.checked_sub(1).map(|i| self.frames[i])
    }

    fn key_byte(&mut self, b: u8) {
        if self.str_is_key && self.depth > 0 {
            let i = self.depth - 1;
            let len = self.key_lens[i] as usize;
            if len < KEY_MAX {
                self.keys[i][len] = b;
            }
            // Saturates above KEY_MAX: such a key never matches.
            self.key_lens[i] = (len + 1).min(KEY_MAX + 1) as u8;
        }
    }

    fn push(&mut self, b: u8) {
        if self.capture.is_some() && !self.overflow {
            if self.buf.len() < self.limit {
                self.buf.push(b);
            } else {
                self.overflow = true;
            }
        }
    }

    fn begin_value(&mut self, sink: &mut impl Sink) {
        if self.depth == 0 {
            if self.started {
                self.error = Some("trailing data after JSON");
                return;
            }
            self.started = true;
        }
        if self.capture.is_none() {
            let path = Path {
                frames: &self.frames[..self.depth],
                keys: &self.keys[..self.depth],
                key_lens: &self.key_lens[..self.depth],
            };
            if let Some(tag) = sink.select(&path) {
                self.capture = Some((tag, self.depth));
                self.buf.clear();
                self.overflow = false;
            }
        }
    }

    /// A value at the current depth has just ended.
    fn end_value(&mut self, sink: &mut impl Sink) {
        if let Some((tag, depth)) = self.capture {
            if depth == self.depth {
                self.capture = None;
                sink.value(tag, (!self.overflow).then_some(&self.buf[..]));
                // Do not hold on to an unusually large buffer.
                if self.buf.capacity() > 2048 {
                    self.buf = Vec::new();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Collect {
        got: Vec<(u8, Option<String>)>,
        pattern: Vec<Vec<&'static str>>,
    }

    impl Sink for Collect {
        fn select(&mut self, path: &Path) -> Option<u8> {
            self.pattern
                .iter()
                .position(|p| {
                    let segs: Vec<Seg> = p.iter().map(|s| if *s == "*" { Seg::Elem } else { Seg::Key(s) }).collect();
                    path.is(&segs)
                })
                .map(|i| i as u8)
        }
        fn value(&mut self, tag: u8, raw: Option<&[u8]>) {
            self.got.push((tag, raw.map(|r| String::from_utf8(r.to_vec()).unwrap())));
        }
    }

    type Split = (Vec<(u8, Option<String>)>, Result<(), &'static str>);

    fn split(doc: &str, patterns: &[&[&'static str]], chunk: usize, limit: usize) -> Split {
        let mut sink = Collect { got: Vec::new(), pattern: patterns.iter().map(|p| p.to_vec()).collect() };
        let mut s = Splitter::new(limit);
        for c in doc.as_bytes().chunks(chunk) {
            s.feed(c, &mut sink);
        }
        let end = s.finish(&mut sink);
        (sink.got, end)
    }

    const DOC: &str = r#"{"a":1, "items" : [ {"t":"x,]}\"{"}, [1,2], "s", 3.5e2, true, null ],
        "loc":{"lat":40.7,"name":"Néw {York}"},"verylongkeynamethatistoolong":{"x":1},"n":-12}"#;

    #[test]
    fn selects_elements_members_and_scalars() {
        let pats: &[&[&str]] = &[&["items", "*"], &["loc"], &["n"], &["loc", "lat"]];
        for chunk in [1, 2, 3, 7, 64, 4096] {
            let (got, end) = split(DOC, pats, chunk, 1024);
            assert_eq!(end, Ok(()), "chunk {chunk}");
            let texts: Vec<(u8, &str)> = got.iter().map(|(t, s)| (*t, s.as_deref().unwrap())).collect();
            assert_eq!(
                texts,
                vec![
                    (0, r#"{"t":"x,]}\"{"}"#),
                    (0, "[1,2]"),
                    (0, r#""s""#),
                    (0, "3.5e2"),
                    (0, "true"),
                    (0, "null"),
                    // `loc` is captured whole, so `loc.lat` is not reported separately.
                    (1, r#"{"lat":40.7,"name":"Néw {York}"}"#),
                    (2, "-12"),
                ],
                "chunk {chunk}"
            );
        }
    }

    #[test]
    fn long_values_are_skipped_and_long_keys_never_match() {
        let (got, end) = split(DOC, &[&["loc"], &["verylongkeynamethatistoolong"], &["verylongkeynamet"]], 5, 10);
        assert_eq!(end, Ok(()));
        assert_eq!(got, vec![(0, None)]);
    }

    #[test]
    fn rejects_broken_documents() {
        assert_eq!(split(r#"{"a":[1,2}"#, &[], 3, 64).1, Err("unbalanced JSON"));
        assert_eq!(split(r#"{"a":[1,2]"#, &[], 3, 64).1, Err("truncated JSON"));
        assert_eq!(split(r#"{"a":"x"#, &[], 3, 64).1, Err("truncated JSON"));
        assert_eq!(split("", &[], 3, 64).1, Err("truncated JSON"));
        assert_eq!(split("{} {}", &[], 3, 64).1, Err("trailing data after JSON"));
        assert_eq!(split("[[[[[[[[[1]]]]]]]]]", &[], 3, 64).1, Err("JSON nested too deeply"));
        assert_eq!(split("42", &[&[]], 1, 64), (vec![(0, Some("42".into()))], Ok(())));
    }
}
