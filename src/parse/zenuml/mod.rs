//! ZenUML parser. ZenUML is a sequence-style notation; we translate it to a
//! [`SequenceDiagram`] so it reuses the sequence renderer.
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

use super::ast::{
    AltBranch, Participant, ParticipantKind, SequenceBlock, SequenceDiagram, SequenceItem,
};
use super::ParseError;

mod blocks;
mod message;

const DEFAULT_STARTER: &str = "Starter";

/// A structural token: an opening/closing brace, or a logical statement string
/// tagged with the 1-based source line it started on (for error reporting).
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Open,
    Close,
    Stmt(String, usize),
}

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
        diag: SequenceDiagram::default(),
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

/// Strip a `//` or `%%` line comment. Only a `//` run (two slashes) counts, so a
/// single `/` inside a path like `GET /login` survives.
fn strip_line_comment(line: &str) -> &str {
    let cut = [line.find("//"), line.find("%%")]
        .into_iter()
        .flatten()
        .min();
    match cut {
        Some(pos) => &line[..pos],
        None => line,
    }
}

/// Split a body into brace/statement tokens. Braces inside parentheses or
/// quotes are kept literal; newlines and `;` terminate statements. Each
/// statement is tagged with the 1-based source line it started on.
fn tokenize(body: &str, base_line: usize) -> Vec<Tok> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    let mut paren = 0u32;
    let mut in_quote = false;
    let mut line = base_line;
    let mut stmt_line = base_line;
    let flush = |cur: &mut String, toks: &mut Vec<Tok>, stmt_line: usize| {
        let s = cur.trim();
        if !s.is_empty() {
            toks.push(Tok::Stmt(s.to_string(), stmt_line));
        }
        cur.clear();
    };
    for ch in body.chars() {
        if !ch.is_whitespace() && cur.trim().is_empty() {
            stmt_line = line;
        }
        if ch == '\n' {
            line += 1;
        }
        if in_quote {
            cur.push(ch);
            if ch == '"' {
                in_quote = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_quote = true;
                cur.push(ch);
            }
            '(' => {
                paren += 1;
                cur.push(ch);
            }
            ')' => {
                paren = paren.saturating_sub(1);
                cur.push(ch);
            }
            '{' if paren == 0 => {
                flush(&mut cur, &mut toks, stmt_line);
                toks.push(Tok::Open);
            }
            '}' if paren == 0 => {
                flush(&mut cur, &mut toks, stmt_line);
                toks.push(Tok::Close);
            }
            '\n' | ';' if paren == 0 => flush(&mut cur, &mut toks, stmt_line),
            _ => cur.push(ch),
        }
    }
    flush(&mut cur, &mut toks, stmt_line);
    toks
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

    /// A participant declaration: a bare identifier (`Bob`) or an alias
    /// (`A as Alice`, id `A` displayed as `Alice`). Returns `false` for anything
    /// that carries a call (`(`) or an arrow (`->`), leaving it to `parse_call`.
    fn try_declaration(&mut self, s: &str) -> bool {
        let s = s.trim();
        if s.contains('(') || s.contains("->") {
            return false;
        }
        if let Some((id, display)) = split_alias(s) {
            self.declare_alias(&id, &display);
            return true;
        }
        if !s.is_empty() && is_identifier(s) {
            self.ensure(s);
            return true;
        }
        false
    }

    fn declare_alias(&mut self, id: &str, display: &str) {
        if let Some(p) = self.diag.participants.iter_mut().find(|p| p.id == id) {
            p.display = display.to_string();
        } else {
            self.diag.participants.push(Participant {
                id: id.to_string(),
                display: display.to_string(),
                kind: ParticipantKind::Participant,
            });
        }
    }

    /// Declare a participant from an annotator line (`Actor Alice`,
    /// `Database DB`, `Starter(Alice)`).
    fn annotator(&mut self, rest: &str) {
        let rest = rest.trim();
        if let Some(inner) = rest
            .strip_prefix("Starter(")
            .and_then(|r| r.strip_suffix(')'))
        {
            let id = inner.trim().to_string();
            if !id.is_empty() {
                self.ensure(&id);
                self.starter = Some(id);
            }
            return;
        }
        let (kind_word, name) = match rest.split_once(char::is_whitespace) {
            Some((k, n)) => (k, n.trim()),
            None => return, // `@Type` with no name declares nothing.
        };
        if name.is_empty() {
            return;
        }
        let kind = match kind_word.to_ascii_lowercase().as_str() {
            "actor" => ParticipantKind::Actor,
            "boundary" => ParticipantKind::Boundary,
            "control" => ParticipantKind::Control,
            "entity" => ParticipantKind::Entity,
            "database" => ParticipantKind::Database,
            _ => ParticipantKind::Participant,
        };
        self.declare(name, kind);
    }

    /// The originating participant for a context: the enclosing receiver, or the
    /// starter (created lazily on first top-level use).
    fn source(&mut self, ctx: Option<&str>) -> String {
        match ctx {
            Some(c) => c.to_string(),
            None => {
                let id = self
                    .starter
                    .clone()
                    .unwrap_or_else(|| DEFAULT_STARTER.into());
                self.ensure(&id);
                self.starter = Some(id.clone());
                id
            }
        }
    }

    fn ensure(&mut self, id: &str) {
        if !self.diag.participants.iter().any(|p| p.id == id) {
            self.diag.participants.push(Participant {
                id: id.to_string(),
                display: id.to_string(),
                kind: ParticipantKind::Participant,
            });
        }
    }

    fn declare(&mut self, id: &str, kind: ParticipantKind) {
        if let Some(p) = self.diag.participants.iter_mut().find(|p| p.id == id) {
            p.kind = kind;
        } else {
            self.diag.participants.push(Participant {
                id: id.to_string(),
                display: id.to_string(),
                kind,
            });
        }
    }
}

/// True if `s` is a single participant identifier (letters, digits, `_`).
fn is_identifier(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Split an `Id as Display` alias declaration into `(id, display)`. `id` must be
/// a plain identifier; `display` may be quoted. Returns `None` when the `as`
/// keyword is absent.
fn split_alias(s: &str) -> Option<(String, String)> {
    let (id, rest) = s.trim().split_once(char::is_whitespace)?;
    let after = rest.trim_start().strip_prefix("as")?;
    // `as` must be a whole word, not the head of a longer identifier.
    if !after.starts_with(char::is_whitespace) {
        return None;
    }
    let display = after.trim().trim_matches('"').trim();
    if !is_identifier(id) || display.is_empty() {
        return None;
    }
    Some((id.to_string(), display.to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(src: &str) -> SequenceDiagram {
        parse(src).unwrap()
    }

    #[test]
    fn annotator_declares_actor_and_starter() {
        let d = parse_ok("zenuml\n@Actor Alice\n@Database DB\n@Starter(Alice)\nDB.query()\n");
        let alice = d.participants.iter().find(|p| p.id == "Alice").unwrap();
        assert_eq!(alice.kind, ParticipantKind::Actor);
        // The call originates from the declared starter, not the synthetic one.
        assert!(matches!(
            d.items.first(),
            Some(SequenceItem::Message(m)) if m.from == "Alice" && m.to == "DB"
        ));
        assert!(d.participants.iter().all(|p| p.id != DEFAULT_STARTER));
    }

    #[test]
    fn comments_are_stripped() {
        let d = parse_ok("zenuml\n// a comment\nA.b() // trailing\n");
        assert_eq!(
            d.items
                .iter()
                .filter(|i| matches!(i, SequenceItem::Message(_)))
                .count(),
            1
        );
    }

    #[test]
    fn stereotype_annotators_set_participant_kind() {
        let d = parse_ok(
            "zenuml\n@Boundary UI\n@Control Ctrl\n@Entity Order\n@Database DB\nUI.click()\n",
        );
        let kind = |id: &str| d.participants.iter().find(|p| p.id == id).unwrap().kind;
        assert_eq!(kind("UI"), ParticipantKind::Boundary);
        assert_eq!(kind("Ctrl"), ParticipantKind::Control);
        assert_eq!(kind("Order"), ParticipantKind::Entity);
        assert_eq!(kind("DB"), ParticipantKind::Database);
    }

    #[test]
    fn bare_and_alias_declarations() {
        let d = parse_ok("zenuml\nBob\nA as Alice\nA.greet()\n");
        // Declaration order is column order: Bob, then A.
        assert_eq!(d.participants[0].id, "Bob");
        let a = d.participants.iter().find(|p| p.id == "A").unwrap();
        assert_eq!(a.display, "Alice");
        // The declarations produced no phantom Starter self-message; only the
        // real `A.greet()` call remains (from the implicit starter to A).
        let msgs: Vec<_> = d
            .items
            .iter()
            .filter_map(|i| match i {
                SequenceItem::Message(m) => Some(m),
                _ => None,
            })
            .collect();
        assert_eq!(msgs.len(), 1);
        assert_eq!((&*msgs[0].from, &*msgs[0].to), (DEFAULT_STARTER, "A"));
    }

    #[test]
    fn new_materializes_participant_with_creation_message() {
        let d = parse_ok("zenuml\nnew A1\nnew A2(with, parameters)\n");
        assert!(d
            .items
            .iter()
            .any(|i| matches!(i, SequenceItem::Create(id) if id == "A1")));
        assert!(d
            .items
            .iter()
            .any(|i| matches!(i, SequenceItem::Create(id) if id == "A2")));
        // Each `new` draws a creation message to the new participant, not a
        // Starter self-call.
        let create_msg = d
            .items
            .iter()
            .filter_map(|i| match i {
                SequenceItem::Message(m) if m.to == "A1" => Some(m),
                _ => None,
            })
            .next()
            .unwrap();
        assert_ne!(&*create_msg.from, "A1");
        assert_eq!(create_msg.text, "«create»");
    }

    #[test]
    fn foreach_is_a_loop_keyword() {
        let d = parse_ok("zenuml\nforeach (item) {\n  A.step()\n}\n");
        assert!(matches!(
            d.items.first(),
            Some(SequenceItem::Loop(b)) if b.label == "item" && b.items.len() == 1
        ));
    }

    #[test]
    fn rejects_missing_header() {
        assert!(matches!(
            parse("flowchart TD\n"),
            Err(ParseError::Syntax { line: 1, .. })
        ));
    }
}
