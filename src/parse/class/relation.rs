//! Class relationship parsing: relation-token detection, multiplicity
//! (cardinality) stripping, and marker-orientation.

use crate::parse::token::find_unquoted;
use crate::parse::ClassRelationKind;

const RELATIONS: &[(&str, ClassRelationKind)] = &[
    ("<|..", ClassRelationKind::Realization),
    ("..|>", ClassRelationKind::Realization),
    ("<|--", ClassRelationKind::Inheritance),
    ("--|>", ClassRelationKind::Inheritance),
    ("*--", ClassRelationKind::Composition),
    ("--*", ClassRelationKind::Composition),
    ("o--", ClassRelationKind::Aggregation),
    ("--o", ClassRelationKind::Aggregation),
    ("..>", ClassRelationKind::Dependency),
    ("<..", ClassRelationKind::Dependency),
    ("-->", ClassRelationKind::Association),
    ("<--", ClassRelationKind::Association),
    ("--", ClassRelationKind::Link),
    ("..", ClassRelationKind::LinkDashed),
];

/// Strip a trailing `"card"` multiplicity, e.g. `Customer "1"` → (`Customer`, `1`).
pub(super) fn split_trailing_card(s: &str) -> (String, Option<String>) {
    let s = s.trim();
    if let Some(inner) = s.strip_suffix('"') {
        if let Some(open) = inner.rfind('"') {
            let card = inner[open + 1..].to_string();
            let rest = inner[..open].trim_end().to_string();
            return (rest, (!card.is_empty()).then_some(card));
        }
    }
    (s.to_string(), None)
}

/// Strip a trailing `()` lollipop-interface marker, e.g. `bar ()` →
/// (`bar`, true). The `()` sits between the class name and the relation token
/// in the `()--` form.
pub(super) fn split_trailing_lollipop(s: &str) -> (String, bool) {
    let s = s.trim();
    match s.strip_suffix("()") {
        Some(rest) => (rest.trim_end().to_string(), true),
        None => (s.to_string(), false),
    }
}

/// Strip a leading `()` lollipop-interface marker, e.g. `() bar` →
/// (`bar`, true). The `()` sits between the relation token and the class name
/// in the `--()` form.
pub(super) fn split_leading_lollipop(s: &str) -> (String, bool) {
    let s = s.trim();
    match s.strip_prefix("()") {
        Some(rest) => (rest.trim_start().to_string(), true),
        None => (s.to_string(), false),
    }
}

/// Strip a leading `"card"` multiplicity, e.g. `"*" Order` → (`Order`, `*`).
pub(super) fn split_leading_card(s: &str) -> (String, Option<String>) {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('"') {
        if let Some(close) = inner.find('"') {
            let card = inner[..close].to_string();
            let rest = inner[close + 1..].trim_start().to_string();
            return (rest, (!card.is_empty()).then_some(card));
        }
    }
    (s.to_string(), None)
}

pub(super) fn find_relation(line: &str) -> Option<(usize, &'static str, ClassRelationKind)> {
    let mut best: Option<(usize, &'static str, ClassRelationKind)> = None;
    for (tok, kind) in RELATIONS {
        if let Some(pos) = find_unquoted(line, tok) {
            let candidate = (pos, *tok, *kind);
            best = match best {
                Some((bp, bt, _)) if bp < pos => Some((bp, bt, best.unwrap().2)),
                Some((bp, bt, _)) if bp == pos && bt.len() > tok.len() => {
                    Some((bp, bt, best.unwrap().2))
                }
                _ => Some(candidate),
            };
        }
    }
    best
}

/// A relation token is "reversed" when its decorated end (triangle/diamond/
/// circle/arrow) is on the left — attached to the `from` class — i.e. it opens
/// with `<`, `*`, or `o` (`<|--`, `<|..`, `*--`, `o--`, `<--`, `<..`). The
/// marker is then drawn at the `from` end instead of `to`. Plain links (`--`,
/// `..`) have no marker, so the flag is irrelevant for them.
pub(super) fn is_reversed_token(tok: &str) -> bool {
    tok.starts_with(['<', '*', 'o'])
}

/// Detect a **two-way** relation (`relationType lineType relationType`, e.g.
/// `<|--|>`, `*--*`, `o--o`, `<-->`, `<..>`): a right-side marker glued to the
/// base token with no separating whitespace. `after` is the source right after
/// the base token; `base` is the matched token (its `..` telling solid from
/// dotted line). Only a left-decorated (reversed) base can carry a mirror
/// marker. Returns the `to`-end kind and the byte length the marker consumed,
/// so the caller can skip past it before reading the right class name.
pub(super) fn detect_two_way(
    after: &str,
    base: &str,
    reversed: bool,
) -> (Option<ClassRelationKind>, usize) {
    use ClassRelationKind::*;
    if !reversed {
        return (None, 0);
    }
    let dotted = base.contains("..");
    if after.starts_with("|>") {
        return (Some(if dotted { Realization } else { Inheritance }), 2);
    }
    match after.as_bytes().first() {
        Some(b'>') => (Some(if dotted { Dependency } else { Association }), 1),
        Some(b'*') => (Some(Composition), 1),
        Some(b'o') => (Some(Aggregation), 1),
        _ => (None, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse;
    use crate::parse::ClassRelationKind;

    #[test]
    fn relations() {
        let s = "classDiagram\nAnimal <|-- Dog\nCar *-- Wheel\nUser ..|> Service\n";
        let d = parse(s).unwrap();
        assert_eq!(d.relations.len(), 3);
        assert_eq!(d.relations[0].kind, ClassRelationKind::Inheritance);
        assert_eq!(d.relations[1].kind, ClassRelationKind::Composition);
        assert_eq!(d.relations[2].kind, ClassRelationKind::Realization);
    }

    #[test]
    fn reversed_tokens_flag_the_from_end() {
        let s = "classDiagram\n\
                 Animal <|-- Dog\n\
                 Dog --|> Animal\n\
                 A --* B\n\
                 A *-- B\n\
                 A --o B\n\
                 A <-- B\n\
                 A <.. B\n\
                 A -- B\n";
        let d = parse(s).unwrap();
        // from/to ordering (and thus layout) is preserved; only the marker end
        // moves. `<|--`/`*--`/`o--`/`<--`/`<..` are reversed (marker at `from`).
        assert!(d.relations[0].reversed); // Animal <|-- Dog
        assert_eq!(d.relations[0].from, "Animal");
        assert_eq!(d.relations[0].to, "Dog");
        assert!(!d.relations[1].reversed); // Dog --|> Animal
        assert!(!d.relations[2].reversed); // A --* B
        assert!(d.relations[3].reversed); // A *-- B
        assert!(!d.relations[4].reversed); // A --o B
        assert!(d.relations[5].reversed); // A <-- B
        assert!(d.relations[6].reversed); // A <.. B
        assert!(!d.relations[7].reversed); // A -- B (plain link)
    }

    #[test]
    fn cardinality_labels() {
        let d = parse("classDiagram\nCustomer \"1\" --> \"*\" Order\n").unwrap();
        assert_eq!(d.classes.len(), 2);
        assert_eq!(d.classes[0].name, "Customer");
        assert_eq!(d.classes[1].name, "Order");
        let r = &d.relations[0];
        assert_eq!(r.from, "Customer");
        assert_eq!(r.to, "Order");
        assert_eq!(r.from_card.as_deref(), Some("1"));
        assert_eq!(r.to_card.as_deref(), Some("*"));
        assert_eq!(r.kind, ClassRelationKind::Association);
    }

    #[test]
    fn cardinality_with_range_and_label() {
        let d = parse("classDiagram\nStudent \"1..*\" o-- \"0..1\" Course : enrolls\n").unwrap();
        let r = &d.relations[0];
        assert_eq!(r.from, "Student");
        assert_eq!(r.to, "Course");
        assert_eq!(r.from_card.as_deref(), Some("1..*"));
        assert_eq!(r.to_card.as_deref(), Some("0..1"));
        assert_eq!(r.label.as_deref(), Some("enrolls"));
        assert_eq!(r.kind, ClassRelationKind::Aggregation);
    }

    #[test]
    fn single_side_cardinality() {
        let d = parse("classDiagram\nA \"1\" --> B\nC --> \"*\" D\n").unwrap();
        assert_eq!(d.relations[0].from_card.as_deref(), Some("1"));
        assert_eq!(d.relations[0].to_card, None);
        assert_eq!(d.relations[0].to, "B");
        assert_eq!(d.relations[1].from_card, None);
        assert_eq!(d.relations[1].to_card.as_deref(), Some("*"));
        assert_eq!(d.relations[1].from, "C");
        assert_eq!(d.relations[1].to, "D");
    }

    #[test]
    fn lollipop_interface_tokens() {
        let d = parse(
            "classDiagram\n\
             bar ()-- foo\n\
             foo --() baz\n\
             classA ()--|> classB\n",
        )
        .unwrap();
        // The `()` glues into the relation, not the class name.
        assert!(d.classes.iter().any(|c| c.name == "bar"));
        assert!(d.classes.iter().any(|c| c.name == "foo"));
        assert!(d.classes.iter().any(|c| c.name == "classA"));
        assert!(!d.classes.iter().any(|c| c.name.contains("()")));

        // `bar ()-- foo`: lollipop on the `from` (bar) end, plain link.
        let r0 = &d.relations[0];
        assert_eq!(r0.from, "bar");
        assert_eq!(r0.to, "foo");
        assert!(r0.lollipop_from);
        assert!(!r0.lollipop_to);
        assert_eq!(r0.kind, ClassRelationKind::Link);

        // `foo --() baz`: lollipop on the `to` (baz) end.
        let r1 = &d.relations[1];
        assert_eq!(r1.from, "foo");
        assert_eq!(r1.to, "baz");
        assert!(!r1.lollipop_from);
        assert!(r1.lollipop_to);

        // `classA ()--|> classB`: lollipop at `from`, inheritance triangle at `to`.
        let r2 = &d.relations[2];
        assert_eq!(r2.from, "classA");
        assert_eq!(r2.to, "classB");
        assert!(r2.lollipop_from);
        assert_eq!(r2.kind, ClassRelationKind::Inheritance);
    }

    #[test]
    fn two_way_relations_do_not_spawn_phantom_class() {
        // `relationType lineType relationType` decorates both ends; the mirror
        // marker must not glue onto the right class name.
        let d = parse(
            "classDiagram\n\
             Animal <|--|> Zebra\n\
             A *--* B\n\
             C o--o D\n\
             E <--> F\n\
             G <..> H\n",
        )
        .unwrap();
        // Exactly the ten real classes, no `|> Zebra` / `* B` phantoms.
        assert_eq!(d.classes.len(), 10);
        assert!(!d
            .classes
            .iter()
            .any(|c| c.name.contains(['<', '>', '*', 'o', '|'])));

        let r = &d.relations[0]; // Animal <|--|> Zebra
        assert_eq!(r.from, "Animal");
        assert_eq!(r.to, "Zebra");
        assert_eq!(r.kind, ClassRelationKind::Inheritance);
        assert!(r.reversed);
        assert_eq!(r.to_kind, Some(ClassRelationKind::Inheritance));

        assert_eq!(d.relations[1].to_kind, Some(ClassRelationKind::Composition));
        assert_eq!(d.relations[2].to_kind, Some(ClassRelationKind::Aggregation));
        assert_eq!(d.relations[3].to_kind, Some(ClassRelationKind::Association));
        assert_eq!(d.relations[4].to_kind, Some(ClassRelationKind::Dependency));
    }

    #[test]
    fn single_ended_relation_has_no_second_marker() {
        let d = parse("classDiagram\nAnimal <|-- Dog\nA --* B\n").unwrap();
        assert_eq!(d.relations[0].to_kind, None);
        assert_eq!(d.relations[1].to_kind, None);
    }

    #[test]
    fn relation_with_label() {
        let s = "classDiagram\nCar --> Engine : has\n";
        let d = parse(s).unwrap();
        assert_eq!(d.relations[0].label.as_deref(), Some("has"));
    }
}
