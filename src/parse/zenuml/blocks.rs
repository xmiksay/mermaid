//! Control-structure blocks: `if/else`, `try/catch/finally`. `while`/`opt`/
//! `par` are handled inline in [`super::Parser::handle_stmt`]; these two need a
//! chain-consuming loop, so they live here.

use super::super::ast::{AltBranch, SequenceItem};
use super::{after_kw, first_word, strip_kw_cond, Parser, Tok};

impl Parser {
    /// `if (c) { … } else if (c2) { … } else { … }` → an `alt` frame.
    pub(super) fn parse_if(
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
    pub(super) fn parse_try(
        &mut self,
        ctx: Option<&str>,
        ret: Option<&str>,
        items: &mut Vec<SequenceItem>,
    ) {
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
}

#[cfg(test)]
mod tests {
    use super::super::super::ast::SequenceItem;
    use super::super::parse;

    fn parse_ok(src: &str) -> super::super::super::ast::SequenceDiagram {
        parse(src).unwrap()
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
            // A.next() is a method call: message + activate/deactivate band.
            Some(SequenceItem::Loop(b)) if b.label == "more"
                && matches!(b.items.first(), Some(SequenceItem::Message(_)))
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
}
