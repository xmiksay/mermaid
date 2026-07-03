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
    AltBranch, ArrowKind, Message, Participant, ParticipantKind, SequenceBlock, SequenceDiagram,
    SequenceItem,
};
use super::ParseError;

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
            return Err(ParseError::Syntax {
                message: "expected 'zenuml' header".into(),
                line: idx + 1,
            });
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
            "while" | "for" | "forEach" | "loop" => {
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
            _ => self.parse_call(s, ctx, ret, items),
        }
    }

    /// `if (c) { … } else if (c2) { … } else { … }` → an `alt` frame.
    fn parse_if(
        &mut self,
        s: &str,
        ctx: Option<&str>,
        ret: Option<&str>,
        items: &mut Vec<SequenceItem>,
    ) {
        let mut branches = vec![AltBranch {
            label: strip_kw_cond(s),
            items: self.braced(ctx, ret),
        }];
        while let Some(Tok::Stmt(next, _)) = self.peek() {
            if first_word(next) != "else" {
                break;
            }
            let next = next.clone();
            self.bump();
            let tail = after_kw(&next, "else");
            let label = if first_word(&tail) == "if" {
                strip_kw_cond(&tail)
            } else {
                "else".to_string()
            };
            let plain_else = first_word(&tail) != "if";
            branches.push(AltBranch {
                label,
                items: self.braced(ctx, ret),
            });
            if plain_else {
                break;
            }
        }
        items.push(SequenceItem::Alt(branches));
    }

    /// `try { … } catch (e) { … } finally { … }` → a `critical` frame.
    fn parse_try(&mut self, ctx: Option<&str>, ret: Option<&str>, items: &mut Vec<SequenceItem>) {
        let mut branches = vec![AltBranch {
            label: String::new(),
            items: self.braced(ctx, ret),
        }];
        while let Some(Tok::Stmt(next, _)) = self.peek() {
            let fw = first_word(next);
            if fw != "catch" && fw != "finally" {
                break;
            }
            let next = next.clone();
            self.bump();
            let label = if fw == "catch" {
                let arg = strip_kw_cond(&next);
                if arg.is_empty() {
                    "catch".to_string()
                } else {
                    format!("catch {arg}")
                }
            } else {
                "finally".to_string()
            };
            branches.push(AltBranch {
                label,
                items: self.braced(ctx, ret),
            });
        }
        items.push(SequenceItem::Critical(branches));
    }

    /// A message or method call, optionally with a `{ … }` body (nesting) and an
    /// `x = …` assignment (which draws a dashed return arrow).
    fn parse_call(
        &mut self,
        s: &str,
        ctx: Option<&str>,
        _ret: Option<&str>,
        items: &mut Vec<SequenceItem>,
    ) {
        let (assign, rhs) = split_assignment(s);
        let call = match self.classify_call(rhs, ctx) {
            Some(c) => c,
            None => return,
        };
        let Call {
            from,
            to,
            text,
            arrow,
        } = call;
        self.ensure(&from);
        self.ensure(&to);
        items.push(SequenceItem::Message(Message {
            from: from.clone(),
            to: to.clone(),
            text,
            arrow,
        }));

        let has_brace = self.peek() == Some(&Tok::Open);
        if has_brace {
            // The body runs in the receiver's context; a `return` there replies
            // to this call's sender.
            let body = self.braced(Some(&to), Some(&from));
            items.extend(body);
        }
        // A nested call or an assigned result draws a dashed return back to the
        // caller (skipped for self-calls, which need no reply arrow).
        if (has_brace || assign.is_some()) && from != to {
            items.push(SequenceItem::Message(Message {
                from: to,
                to: from,
                text: assign.unwrap_or_default(),
                arrow: ArrowKind::DashedArrow,
            }));
        }
    }

    /// Resolve a call's `from`/`to`/`text`/`arrow`. Returns `None` for an empty
    /// statement that names no participant.
    fn classify_call(&mut self, rhs: &str, ctx: Option<&str>) -> Option<Call> {
        // Explicit arrow form: `A ->> B: msg` or `A -> B.method()`.
        for (sep, arrow) in [("->>", ArrowKind::SolidArrow), ("->", ArrowKind::Solid)] {
            if let Some((left, right)) = rhs.split_once(sep) {
                let from = left.trim().to_string();
                let right = right.trim();
                let (to, text) = match right.split_once(':') {
                    Some((t, msg)) => (t.trim().to_string(), msg.trim().to_string()),
                    None => split_receiver(right),
                };
                if from.is_empty() || to.is_empty() {
                    return None;
                }
                return Some(Call {
                    from,
                    to,
                    text,
                    arrow,
                });
            }
        }
        // Method-call form from the current context: `Recv.method()` / `method()`.
        let rhs = rhs.trim();
        if rhs.is_empty() {
            return None;
        }
        let from = self.source(ctx);
        let (to, text) = split_receiver(rhs);
        let to = if to.is_empty() { from.clone() } else { to };
        Some(Call {
            from,
            to,
            text,
            arrow: ArrowKind::SolidArrow,
        })
    }

    fn emit_return(
        &mut self,
        text: &str,
        line: usize,
        ctx: Option<&str>,
        ret: Option<&str>,
        items: &mut Vec<SequenceItem>,
    ) {
        // A top-level `return` has no caller to reply to — an author error.
        let Some(target) = ret else {
            if self.error.is_none() {
                self.error = Some(ParseError::Syntax {
                    message: "`return` outside of a method-call body has no caller to reply to"
                        .into(),
                    line,
                });
            }
            return;
        };
        let from = self.source(ctx);
        self.ensure(target);
        items.push(SequenceItem::Message(Message {
            from,
            to: target.to_string(),
            text: text.trim().to_string(),
            arrow: ArrowKind::DashedArrow,
        }));
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

struct Call {
    from: String,
    to: String,
    text: String,
    arrow: ArrowKind,
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

/// Split a `Recv.method(args)` into `(Recv, "method(args)")`; a call with no
/// receiver dot returns an empty receiver and the whole text.
fn split_receiver(s: &str) -> (String, String) {
    let s = s.trim();
    // Only a dot before the argument list separates a receiver.
    let paren = s.find('(').unwrap_or(s.len());
    match s[..paren].find('.') {
        Some(dot) => (s[..dot].trim().to_string(), s[dot + 1..].trim().to_string()),
        None => (String::new(), s.to_string()),
    }
}

/// Split a leading `name = …` assignment. The left side must be a plain
/// identifier, so a message body containing `=` (`A -> B: n = 5`) is untouched.
fn split_assignment(s: &str) -> (Option<String>, &str) {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'=' if depth == 0 => {
                let prev = if i > 0 { bytes[i - 1] } else { b' ' };
                let next = *bytes.get(i + 1).unwrap_or(&b' ');
                if matches!(prev, b'<' | b'>' | b'=' | b'!') || next == b'=' {
                    return (None, s);
                }
                let left = s[..i].trim();
                if !left.is_empty() && left.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return (Some(left.to_string()), s[i + 1..].trim());
                }
                return (None, s);
            }
            _ => {}
        }
    }
    (None, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(src: &str) -> SequenceDiagram {
        parse(src).unwrap()
    }

    #[test]
    fn basic_arrow() {
        let d = parse_ok("zenuml\nAlice -> Bob: Hello\nBob ->> Alice: Reply\n");
        assert_eq!(d.participants.len(), 2);
        assert_eq!(d.items.len(), 2);
    }

    #[test]
    fn method_call_uses_starter() {
        let d = parse_ok("zenuml\nA.b()\n");
        // Starter (implicit caller) + A.
        assert_eq!(d.participants.len(), 2);
        assert!(matches!(
            d.items.first(),
            Some(SequenceItem::Message(m)) if m.from == DEFAULT_STARTER && m.to == "A" && m.text == "b()"
        ));
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
    fn nesting_braces_body_and_return() {
        let d = parse_ok("zenuml\nA -> B.method() {\n  ret = process()\n}\n");
        // A -> B: method(), then B -> B: process() (self, no return), then the
        // implicit dashed return B --> A for the closed brace.
        let msgs: Vec<_> = d
            .items
            .iter()
            .filter_map(|i| match i {
                SequenceItem::Message(m) => Some(m),
                _ => None,
            })
            .collect();
        assert_eq!(msgs.len(), 3);
        assert_eq!((&*msgs[0].from, &*msgs[0].to), ("A", "B"));
        assert_eq!((&*msgs[1].from, &*msgs[1].to), ("B", "B"));
        assert_eq!((&*msgs[2].from, &*msgs[2].to), ("B", "A"));
        assert_eq!(msgs[2].arrow, ArrowKind::DashedArrow);
    }

    #[test]
    fn assignment_without_brace_returns() {
        let d = parse_ok("zenuml\nres = A.load()\n");
        let msgs: Vec<_> = d
            .items
            .iter()
            .filter_map(|i| match i {
                SequenceItem::Message(m) => Some(m),
                _ => None,
            })
            .collect();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[1].text, "res");
        assert_eq!(msgs[1].arrow, ArrowKind::DashedArrow);
    }

    #[test]
    fn explicit_return_replies_to_caller() {
        let d = parse_ok("zenuml\nA -> B.handle() {\n  return done\n}\n");
        let ret = d
            .items
            .iter()
            .filter_map(|i| match i {
                SequenceItem::Message(m) if m.arrow == ArrowKind::DashedArrow => Some(m),
                _ => None,
            })
            .find(|m| m.text == "done")
            .unwrap();
        assert_eq!((&*ret.from, &*ret.to), ("B", "A"));
    }

    #[test]
    fn if_else_maps_to_alt() {
        let d = parse_ok(
            "zenuml\nif (ok) {\n  A.a()\n} else if (retry) {\n  A.b()\n} else {\n  A.c()\n}\n",
        );
        let alt = d
            .items
            .iter()
            .find_map(|i| match i {
                SequenceItem::Alt(b) => Some(b),
                _ => None,
            })
            .unwrap();
        assert_eq!(alt.len(), 3);
        assert_eq!(alt[0].label, "ok");
        assert_eq!(alt[1].label, "retry");
        assert_eq!(alt[2].label, "else");
    }

    #[test]
    fn while_maps_to_loop() {
        let d = parse_ok("zenuml\nwhile (more) {\n  A.next()\n}\n");
        assert!(matches!(
            d.items.first(),
            Some(SequenceItem::Loop(b)) if b.label == "more" && b.items.len() == 1
        ));
    }

    #[test]
    fn opt_and_par_frames() {
        let d = parse_ok("zenuml\nopt (cond) {\n  A.x()\n}\npar {\n  A.y()\n}\n");
        assert!(d
            .items
            .iter()
            .any(|i| matches!(i, SequenceItem::Opt(b) if b.label == "cond")));
        assert!(d
            .items
            .iter()
            .any(|i| matches!(i, SequenceItem::Par(b) if b.len() == 1)));
    }

    #[test]
    fn try_catch_finally_maps_to_critical() {
        let d = parse_ok(
            "zenuml\ntry {\n  A.risky()\n} catch (e) {\n  A.recover()\n} finally {\n  A.cleanup()\n}\n",
        );
        let crit = d
            .items
            .iter()
            .find_map(|i| match i {
                SequenceItem::Critical(b) => Some(b),
                _ => None,
            })
            .unwrap();
        assert_eq!(crit.len(), 3);
        assert_eq!(crit[1].label, "catch e");
        assert_eq!(crit[2].label, "finally");
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
    fn at_return_aliases_return_arrow() {
        let d = parse_ok("zenuml\nA -> B.handle() {\n  @return done\n}\n");
        let ret = d
            .items
            .iter()
            .filter_map(|i| match i {
                SequenceItem::Message(m) if m.arrow == ArrowKind::DashedArrow => Some(m),
                _ => None,
            })
            .find(|m| m.text == "done")
            .unwrap();
        assert_eq!((&*ret.from, &*ret.to), ("B", "A"));
    }

    #[test]
    fn stray_return_is_a_syntax_error() {
        let err = parse("zenuml\nA.b()\nreturn oops\n").unwrap_err();
        assert!(matches!(err, ParseError::Syntax { line: 3, .. }));
    }

    #[test]
    fn stray_at_return_is_a_syntax_error() {
        let err = parse("zenuml\n@reply nope\n").unwrap_err();
        assert!(matches!(err, ParseError::Syntax { line: 2, .. }));
    }

    #[test]
    fn rejects_missing_header() {
        assert!(matches!(
            parse("flowchart TD\n"),
            Err(ParseError::Syntax { line: 1, .. })
        ));
    }
}
