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
    fn relation_with_label() {
        let s = "classDiagram\nCar --> Engine : has\n";
        let d = parse(s).unwrap();
        assert_eq!(d.relations[0].label.as_deref(), Some("has"));
    }
}
