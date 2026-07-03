//! Shared line scanner for the flowchart parser.
//!
//! A tiny cursor over a `&str`, driven by the node/shape and edge/arrow
//! scanners in the sibling submodules. Fields and methods are `pub(super)` so
//! those siblings can advance and inspect it directly.

use super::super::token::find_unquoted;

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
    /// Read a node/edge identifier. Beyond alphanumerics, `_` and `.`, a `-` or
    /// `/` is part of the id when it does *not* begin an edge connector —
    /// upstream's NODE_STRING allows `a-node`/`a/b` but stops the dash before an
    /// arrow (`-->`, `-.->`). So a `-`/`/` is consumed only when the next char
    /// continues the id (alphanumeric / `_` / `.`); a following `-`/`/`/`>`/`.`,
    /// whitespace, or end-of-input leaves it for the arrow scanner.
    pub(super) fn read_ident(&mut self) -> Option<String> {
        let rem = self.remaining();
        let chars: Vec<(usize, char)> = rem.char_indices().collect();
        let mut end = 0;
        let mut k = 0;
        while k < chars.len() {
            let (idx, c) = chars[k];
            if c.is_alphanumeric() || c == '_' || c == '.' {
                end = idx + c.len_utf8();
            } else if c == '-' || c == '/' {
                match chars.get(k + 1).map(|&(_, n)| n) {
                    Some(n) if n.is_alphanumeric() || n == '_' => {
                        end = idx + c.len_utf8();
                    }
                    _ => break,
                }
            } else {
                break;
            }
            k += 1;
        }
        if end == 0 {
            return None;
        }
        let s = rem[..end].to_string();
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
    /// Like [`read_until`] but the terminator is matched only outside a `"…"`
    /// quoted run, so a shape/label may embed its own closer (`A["a ] b"]`).
    pub(super) fn read_until_unquoted(&mut self, terminator: &str) -> Option<String> {
        let rem = self.remaining();
        let pos = find_unquoted(rem, terminator)?;
        let s = rem[..pos].to_string();
        self.i += pos;
        Some(s)
    }
}
