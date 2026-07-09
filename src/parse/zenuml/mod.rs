//! ZenUML parser. ZenUML is a sequence-style notation; we translate it to a
//! [`SequenceDiagram`] (with its `zenuml` flag set) that the dedicated ZenUML
//! renderer in `src/svg/sequence/zenuml.rs` draws with call-nesting activation
//! bars, hierarchical numbering, and top-only boxed participants.
//!
//! Supported subset:
//!
//! ```text
//! zenuml
//!     title <text>
//!     @Actor Alice                    // annotator: declares Alice as an actor
//!     @Boundary UI                    // UML stereotypes: boundary/control/
//!     @Control Ctrl                   //   entity/database get distinct glyphs
//!     @Entity Order
//!     @Database DB                    // any other annotator declares a participant
//!     @Starter(Alice)                 // sets the implicit top-level caller
//!     <From> -> <To>: <message>       // plain messages
//!     <From> ->> <To>: <message>
//!     <Recv>.method(args)             // method call from the current context
//!     <From> -> <Recv>.method() {     // nesting: body runs in <Recv>'s context
//!         ret = process()             //   assignment → dashed return arrow
//!         return value                //   explicit return to the caller
//!         @return value               //   `@return`/`@reply` alias the above
//!     }
//!     if (cond) { … } else if (c) { … } else { … }   // → alt frame
//!     while (cond) { … }              // → loop frame
//!     opt (cond) { … }                // → opt frame
//!     par { … }                       // → par frame
//!     try { … } catch (e) { … } finally { … }         // → critical frame
//! ```
//!
//! A bare `method()` / `A.method()` originates from the current context: the
//! [`@Starter`] participant at the top level (defaulting to a synthetic
//! `Starter` lane), or the receiver of the enclosing method-call brace.

use super::ast::{AltBranch, SequenceBlock, SequenceDiagram, SequenceItem};
use super::ParseError;
use lexer::{strip_line_comment, tokenize, Tok};

mod blocks;
mod declare;
mod lexer;
mod message;
#[cfg(test)]
mod tests;

const DEFAULT_STARTER: &str = "Starter";

pub(crate) fn parse(input: &str) -> Result<SequenceDiagram, ParseError> {
    let mut lines = input.lines().enumerate();
    // Header: first non-empty, non-comment line must be `zenuml`.
    let mut header_line = None;
    for (idx, raw) in lines.by_ref() {
        let line = strip_line_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line != "zenuml" {
            return Err(ParseError::header(idx + 1, "expected 'zenuml' header"));
        }
        header_line = Some(idx);
        break;
    }
    if header_line.is_none() {
        return Err(ParseError::Empty);
    }

    // The rest of the document is brace-structured; tokenize it whole. The body
    // begins on the line right after the `zenuml` header (1-based).
    let base_line = header_line.unwrap() + 2;
    let body: String = input
        .lines()
        .skip(header_line.unwrap() + 1)
        .map(strip_line_comment)
        .collect::<Vec<_>>()
        .join("\n");
    let toks = tokenize(&body, base_line);

    let mut p = Parser {
        toks,
        pos: 0,
        diag: SequenceDiagram {
            zenuml: true,
            ..SequenceDiagram::default()
        },
        starter: None,
        error: None,
    };
    let items = p.parse_items(None, None);
    if let Some(err) = p.error {
        return Err(err);
    }
    p.diag.items = items;
    Ok(p.diag)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    diag: SequenceDiagram,
    starter: Option<String>,
    /// First syntax error hit while walking the (position-free) token stream.
    /// Reported after the walk finishes.
    error: Option<ParseError>,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    /// Consume an `Open`, the items up to the matching `Close`, and the `Close`.
    /// A missing `Open` yields an empty body (tolerant of malformed input).
    fn braced(&mut self, ctx: Option<&str>, ret: Option<&str>) -> Vec<SequenceItem> {
        if self.peek() != Some(&Tok::Open) {
            return Vec::new();
        }
        self.bump();
        let items = self.parse_items(ctx, ret);
        if self.peek() == Some(&Tok::Close) {
            self.bump();
        }
        items
    }

    fn parse_items(&mut self, ctx: Option<&str>, ret: Option<&str>) -> Vec<SequenceItem> {
        let mut items = Vec::new();
        while let Some(tok) = self.peek() {
            match tok {
                Tok::Close => break,
                Tok::Open => {
                    // Stray block: run it in the current context.
                    let inner = self.braced(ctx, ret);
                    items.extend(inner);
                }
                Tok::Stmt(..) => {
                    let (s, line) = match self.bump() {
                        Some(Tok::Stmt(s, line)) => (s, line),
                        _ => unreachable!(),
                    };
                    self.handle_stmt(&s, line, ctx, ret, &mut items);
                }
            }
        }
        items
    }

    fn handle_stmt(
        &mut self,
        s: &str,
        line: usize,
        ctx: Option<&str>,
        ret: Option<&str>,
        items: &mut Vec<SequenceItem>,
    ) {
        // Annotators / declarations. `@return`/`@reply` are annotation aliases
        // for the bare `return` keyword, not participant declarations.
        if let Some(rest) = s.strip_prefix('@') {
            let kw = first_word(rest);
            if kw == "return" || kw == "reply" {
                self.emit_return(&after_kw(rest, &kw), line, ctx, ret, items);
            } else {
                self.annotator(rest);
            }
            return;
        }
        if let Some(rest) = s.strip_prefix("title ") {
            self.diag.title = Some(rest.trim().to_string());
            return;
        }

        match first_word(s).as_str() {
            "if" => self.parse_if(s, ctx, ret, items),
            "while" | "for" | "forEach" | "foreach" | "loop" => {
                let label = strip_kw_cond(s);
                let body = self.braced(ctx, ret);
                items.push(SequenceItem::Loop(SequenceBlock { label, items: body }));
            }
            "opt" => {
                let label = strip_kw_cond(s);
                let body = self.braced(ctx, ret);
                items.push(SequenceItem::Opt(SequenceBlock { label, items: body }));
            }
            "par" => {
                let body = self.braced(ctx, ret);
                items.push(SequenceItem::Par(vec![AltBranch {
                    label: String::new(),
                    items: body,
                }]));
            }
            "try" => self.parse_try(ctx, ret, items),
            // A stray chain keyword (malformed input) — run any body inline.
            "else" | "catch" | "finally" => {
                let body = self.braced(ctx, ret);
                items.extend(body);
            }
            "return" => self.emit_return(&after_kw(s, "return"), line, ctx, ret, items),
            "new" => self.parse_new(s, ctx, items),
            // A bare `Bob` or an `A as Alice` alias declares a participant; only
            // fall through to a message/call when it isn't a declaration.
            _ if self.try_declaration(s) => {}
            _ => self.parse_call(s, ctx, ret, items),
        }
    }
}

/// The leading identifier word of a statement (letters, digits, `_`).
fn first_word(s: &str) -> String {
    s.trim()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Everything after the leading keyword, trimmed.
fn after_kw(s: &str, kw: &str) -> String {
    s.trim()
        .strip_prefix(kw)
        .map(|r| r.trim().to_string())
        .unwrap_or_default()
}

/// Strip a leading control keyword and an optional `( … )` condition wrapper,
/// e.g. `if (x > 0)` → `x > 0`, `while cond` → `cond`, `catch (e)` → `e`.
fn strip_kw_cond(s: &str) -> String {
    let rest = s.trim();
    let word = first_word(rest);
    let rest = rest[word.len()..].trim();
    match rest.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        Some(inner) => inner.trim().to_string(),
        None => rest.to_string(),
    }
}
