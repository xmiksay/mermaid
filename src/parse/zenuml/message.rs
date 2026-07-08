//! Messages and method calls: the arrow forms (`A -> B`, `A ->> B`), the
//! context-relative method call (`Recv.method()` / `method()`), the `x = …`
//! assignment return, and the explicit `return` reply.

use super::super::ast::{ArrowKind, Message, SequenceItem};
use super::super::ParseError;
use super::{after_kw, Parser, Tok};

struct Call {
    from: String,
    to: String,
    text: String,
    arrow: ArrowKind,
}

impl Parser {
    /// A message or method call, optionally with a `{ … }` body (nesting) and an
    /// `x = …` assignment (which draws a dashed return arrow).
    pub(super) fn parse_call(
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
        // A method invocation (`Recv.method(...)`) carries a `(` in its text; a
        // plain `A -> B: label` message does not. Invocations (and any call that
        // opens a body) activate the receiver.
        let has_brace = self.peek() == Some(&Tok::Open);
        let activate = has_brace || text.contains('(');
        items.push(SequenceItem::Message(Message {
            from: from.clone(),
            to: to.clone(),
            text,
            arrow,
        }));

        // The receiver is activated for the duration of the call: a nested band
        // encodes the call depth (ZenUML draws these grey activation rectangles).
        if activate {
            items.push(SequenceItem::Activate(to.clone()));
        }
        if has_brace {
            // The body runs in the receiver's context; a `return` there replies
            // to this call's sender.
            let body = self.braced(Some(&to), Some(&from));
            items.extend(body);
        }
        if activate {
            items.push(SequenceItem::Deactivate(to.clone()));
        }
        // Only a *labeled* return is drawn: an `x = call()` assignment shows the
        // result on a dashed reply. Unlabeled synthesized returns are implied by
        // the activation ending, so ZenUML omits them (issue #266).
        if let Some(label) = assign {
            if from != to {
                items.push(SequenceItem::Message(Message {
                    from: to,
                    to: from,
                    text: label,
                    arrow: ArrowKind::DashedArrow,
                }));
            }
        }
    }

    /// `new A1` / `new A2(with, parameters)` — materialize the participant and
    /// draw a `«create»` creation message to it from the current context.
    pub(super) fn parse_new(&mut self, s: &str, ctx: Option<&str>, items: &mut Vec<SequenceItem>) {
        let rest = after_kw(s, "new");
        let paren = rest.find('(').unwrap_or(rest.len());
        let id = rest[..paren].trim().to_string();
        if id.is_empty() {
            // `new` with no target — treat the rest as an ordinary call.
            self.parse_call(&rest, ctx, None, items);
            return;
        }
        let from = self.source(ctx);
        self.ensure(&id);
        items.push(SequenceItem::Create(id.clone()));
        items.push(SequenceItem::Message(Message {
            from,
            to: id,
            text: "«create»".to_string(),
            arrow: ArrowKind::SolidArrow,
        }));
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

    pub(super) fn emit_return(
        &mut self,
        text: &str,
        line: usize,
        ctx: Option<&str>,
        ret: Option<&str>,
        items: &mut Vec<SequenceItem>,
    ) {
        let text = text.trim();
        // Explicit `From -> To: message` reply (upstream form 3) is directed on
        // its own, so it's valid even at top level with no enclosing caller.
        for sep in ["->>", "->"] {
            if let Some((left, right)) = text.split_once(sep) {
                let from = left.trim().to_string();
                let (to, msg) = match right.split_once(':') {
                    Some((t, m)) => (t.trim().to_string(), m.trim().to_string()),
                    None => (right.trim().to_string(), String::new()),
                };
                if from.is_empty() || to.is_empty() {
                    break;
                }
                self.ensure(&from);
                self.ensure(&to);
                items.push(SequenceItem::Message(Message {
                    from,
                    to,
                    text: msg,
                    arrow: ArrowKind::DashedArrow,
                }));
                return;
            }
        }
        // A bare-value `return` has no caller to reply to at top level — an
        // author error.
        let Some(target) = ret else {
            if self.error.is_none() {
                self.error = Some(ParseError::malformed(
                    line,
                    "`return` outside of a method-call body has no caller to reply to",
                ));
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

/// Split a leading `name = …` assignment. The left side is one or more plain
/// identifier words (`res` or a typed `SomeType a`); the trailing word is the
/// variable used as the return-arrow label. A message body containing `=`
/// (`A -> B: n = 5`) leaves the whole string untouched.
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
                let mut words = left.split_whitespace();
                if let Some(var) = words.next_back() {
                    if left
                        .split_whitespace()
                        .all(|w| w.chars().all(|c| c.is_alphanumeric() || c == '_'))
                    {
                        return (Some(var.to_string()), s[i + 1..].trim());
                    }
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
    use super::super::super::ast::{ArrowKind, SequenceItem};
    use super::super::super::ParseError;
    use super::super::{parse, DEFAULT_STARTER};

    fn parse_ok(src: &str) -> super::super::super::ast::SequenceDiagram {
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
    fn nesting_braces_body_and_return() {
        let d = parse_ok("zenuml\nA -> B.method() {\n  ret = process()\n}\n");
        // A -> B: method(), then B -> B: process() (self). The unlabeled return
        // for the closed brace is implied by B's activation ending, so it is not
        // drawn (issue #266); `process()` is a self-call so it has no return.
        let msgs: Vec<_> = d
            .items
            .iter()
            .filter_map(|i| match i {
                SequenceItem::Message(m) => Some(m),
                _ => None,
            })
            .collect();
        assert_eq!(msgs.len(), 2);
        assert_eq!((&*msgs[0].from, &*msgs[0].to), ("A", "B"));
        assert_eq!((&*msgs[1].from, &*msgs[1].to), ("B", "B"));
        // The braced call activates its receiver for the body's duration.
        assert!(d
            .items
            .iter()
            .any(|i| matches!(i, SequenceItem::Activate(id) if id == "B")));
        assert!(d
            .items
            .iter()
            .any(|i| matches!(i, SequenceItem::Deactivate(id) if id == "B")));
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
    fn typed_assignment_returns_variable() {
        let d = parse_ok("zenuml\nSomeType a = A.SyncMessage()\n");
        let msgs: Vec<_> = d
            .items
            .iter()
            .filter_map(|i| match i {
                SequenceItem::Message(m) => Some(m),
                _ => None,
            })
            .collect();
        // The call lands on A (not a participant named `SomeType a = A`) and the
        // return arrow is labeled with the variable.
        assert!(d.participants.iter().all(|p| p.id != "SomeType a = A"));
        assert_eq!(&*msgs[0].to, "A");
        assert_eq!(msgs[0].text, "SyncMessage()");
        assert_eq!(msgs[1].text, "a");
        assert_eq!(msgs[1].arrow, ArrowKind::DashedArrow);
    }

    #[test]
    fn top_level_directed_return_is_valid() {
        let d = parse_ok("zenuml\n@return A->B: result\n");
        let ret = d
            .items
            .iter()
            .find_map(|i| match i {
                SequenceItem::Message(m) => Some(m),
                _ => None,
            })
            .unwrap();
        assert_eq!((&*ret.from, &*ret.to), ("A", "B"));
        assert_eq!(ret.text, "result");
        assert_eq!(ret.arrow, ArrowKind::DashedArrow);
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
}
