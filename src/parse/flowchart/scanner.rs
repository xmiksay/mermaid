//! Shared line scanner for the flowchart parser.
//!
//! A tiny cursor over a `&str`, driven by the node/shape and edge/arrow
//! scanners in the sibling submodules. Fields and methods are `pub(super)` so
//! those siblings can advance and inspect it directly.

pub(super) struct Scanner<'a> {
    pub(super) s: &'a str,
    pub(super) i: usize,
}

impl<'a> Scanner<'a> {
    pub(super) fn new(s: &'a str) -> Self {
        Self { s, i: 0 }
    }
    pub(super) fn eof(&self) -> bool {
        self.i >= self.s.len()
    }
    pub(super) fn remaining(&self) -> &'a str {
        &self.s[self.i..]
    }
    pub(super) fn peek_str(&self, prefix: &str) -> bool {
        self.remaining().starts_with(prefix)
    }
    pub(super) fn try_consume(&mut self, prefix: &str) -> bool {
        if self.peek_str(prefix) {
            self.i += prefix.len();
            true
        } else {
            false
        }
    }
    pub(super) fn advance(&mut self, n: usize) {
        self.i += n;
    }
    pub(super) fn skip_ws(&mut self) {
        while let Some(c) = self.remaining().chars().next() {
            if c == ' ' || c == '\t' {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
    }
    pub(super) fn read_ident(&mut self) -> Option<String> {
        let mut end = 0;
        for c in self.remaining().chars() {
            if c.is_alphanumeric() || c == '_' || c == '.' {
                end += c.len_utf8();
            } else {
                break;
            }
        }
        if end == 0 {
            return None;
        }
        let s = self.remaining()[..end].to_string();
        self.i += end;
        Some(s)
    }
    pub(super) fn read_until(&mut self, terminator: &str) -> Option<String> {
        let rem = self.remaining();
        let pos = rem.find(terminator)?;
        let s = rem[..pos].to_string();
        self.i += pos;
        Some(s)
    }
}
