use super::*;
use crate::parse::ast::Cardinality;

#[test]
fn relations_basic() {
    let s = "erDiagram\nCUSTOMER ||--o{ ORDER : places\nORDER ||--|{ LINE-ITEM : contains\n";
    let d = parse(s).unwrap();
    assert_eq!(d.relations.len(), 2);
    let r0 = &d.relations[0];
    assert_eq!(r0.left, "CUSTOMER");
    assert_eq!(r0.right, "ORDER");
    assert_eq!(r0.left_card, Cardinality::ExactlyOne);
    assert_eq!(r0.right_card, Cardinality::ZeroOrMore);
    assert_eq!(r0.label, "places");
    assert!(r0.identifying);
}

#[test]
fn dotted_line_is_nonidentifying() {
    let s = "erDiagram\nA }|..|{ B : uses\n";
    let d = parse(s).unwrap();
    assert!(!d.relations[0].identifying);
    assert_eq!(d.relations[0].left_card, Cardinality::OneOrMore);
    assert_eq!(d.relations[0].right_card, Cardinality::OneOrMore);
}

#[test]
fn entity_block() {
    let s =
        "erDiagram\nCUSTOMER {\nstring name\nstring email PK\n}\nCUSTOMER ||--o{ ORDER : places\n";
    let d = parse(s).unwrap();
    let c = d.entities.iter().find(|e| e.name == "CUSTOMER").unwrap();
    assert_eq!(c.attributes.len(), 2);
    assert_eq!(c.attributes[1].key.as_deref(), Some("PK"));
}

#[test]
fn verbal_cardinalities() {
    let s = "erDiagram\nCAR only one to zero or more NAMED-DRIVER : allows\n";
    let d = parse(s).unwrap();
    let r = &d.relations[0];
    assert_eq!(r.left, "CAR");
    assert_eq!(r.right, "NAMED-DRIVER");
    assert_eq!(r.left_card, Cardinality::ExactlyOne);
    assert_eq!(r.right_card, Cardinality::ZeroOrMore);
    assert!(r.identifying);
    assert_eq!(r.label, "allows");
}

#[test]
fn optionally_to_is_nonidentifying() {
    let d = parse("erDiagram\nA one or many optionally to one or zero B : x\n").unwrap();
    let r = &d.relations[0];
    assert_eq!(r.left_card, Cardinality::OneOrMore);
    assert_eq!(r.right_card, Cardinality::ZeroOrOne);
    assert!(!r.identifying);
}

#[test]
fn numeric_cardinalities() {
    let d = parse("erDiagram\nPERSON 1--1 CAR : owns\n").unwrap();
    let r = &d.relations[0];
    assert_eq!(r.left, "PERSON");
    assert_eq!(r.right, "CAR");
    assert_eq!(r.left_card, Cardinality::ExactlyOne);
    assert_eq!(r.right_card, Cardinality::ExactlyOne);
}

#[test]
fn entity_alias_no_duplicate() {
    let s = "erDiagram\np[Person] {\nstring name\n}\np ||--o{ ORDER : places\n";
    let d = parse(s).unwrap();
    assert_eq!(d.entities.iter().filter(|e| e.name == "p").count(), 1);
    let p = d.entities.iter().find(|e| e.name == "p").unwrap();
    assert_eq!(p.label, "Person");
    assert_eq!(p.attributes.len(), 1);
}

#[test]
fn alias_upgrades_earlier_reference() {
    // Relation references `p` before its aliased block appears.
    let s = "erDiagram\np ||--o{ ORDER : places\np[Person] {\nstring name\n}\n";
    let d = parse(s).unwrap();
    assert_eq!(d.entities.iter().filter(|e| e.name == "p").count(), 1);
    assert_eq!(
        d.entities.iter().find(|e| e.name == "p").unwrap().label,
        "Person"
    );
}

#[test]
fn direction_keyword() {
    let d = parse("erDiagram\ndirection LR\nA ||--o{ B : x\n").unwrap();
    assert_eq!(d.direction, FlowDirection::LeftRight);
}

#[test]
fn multiple_key_constraints() {
    let d = parse("erDiagram\nORDER {\nstring id PK, FK\n}\n").unwrap();
    let o = d.entities.iter().find(|e| e.name == "ORDER").unwrap();
    assert_eq!(o.attributes[0].key.as_deref(), Some("PK, FK"));
}

#[test]
fn classdef_and_class_apply() {
    let s = "erDiagram\nCUSTOMER ||--o{ ORDER : places\nclassDef hot fill:#f00,stroke:#900\nclass CUSTOMER hot\n";
    let d = parse(s).unwrap();
    assert_eq!(d.class_defs.len(), 1);
    assert!(d.class_defs.contains_key("hot"));
    let c = d.entities.iter().find(|e| e.name == "CUSTOMER").unwrap();
    assert_eq!(c.classes, vec!["hot".to_string()]);
    // The other entity carries no class.
    let o = d.entities.iter().find(|e| e.name == "ORDER").unwrap();
    assert!(o.classes.is_empty());
}

#[test]
fn style_directive_on_entity() {
    let d = parse("erDiagram\nORDER {\nstring id\n}\nstyle ORDER fill:#0f0\n").unwrap();
    let o = d.entities.iter().find(|e| e.name == "ORDER").unwrap();
    assert_eq!(o.style, vec![("fill".to_string(), "#0f0".to_string())]);
}

#[test]
fn classdef_without_props_errors() {
    assert!(parse("erDiagram\nA ||--|| B : x\nclassDef foo\n").is_err());
}

#[test]
fn style_class_shorthand_on_relation() {
    // `:::class` on an entity ref must not hard-error or swallow the label.
    let d = parse("erDiagram\nA:::hot ||--o{ B : places\n").unwrap();
    let r = &d.relations[0];
    assert_eq!(r.left, "A");
    assert_eq!(r.right, "B");
    assert_eq!(r.label, "places");
    let a = d.entities.iter().find(|e| e.name == "A").unwrap();
    assert_eq!(a.classes, vec!["hot".to_string()]);
    // The undecorated endpoint keeps no class.
    assert!(d
        .entities
        .iter()
        .find(|e| e.name == "B")
        .unwrap()
        .classes
        .is_empty());
}

#[test]
fn style_class_shorthand_on_both_ends_and_bare() {
    let d = parse("erDiagram\nA:::hot ||--o{ B:::cold : x\nC:::warm\n").unwrap();
    assert_eq!(
        d.entities.iter().find(|e| e.name == "B").unwrap().classes,
        vec!["cold".to_string()]
    );
    assert_eq!(
        d.entities.iter().find(|e| e.name == "C").unwrap().classes,
        vec!["warm".to_string()]
    );
}

#[test]
fn quoted_entity_names_are_unquoted() {
    let d = parse("erDiagram\n\"HELLO WORLD\" ||--o{ ORDER : places\n").unwrap();
    let r = &d.relations[0];
    assert_eq!(r.left, "HELLO WORLD");
    assert_eq!(r.right, "ORDER");
    let e = d.entities.iter().find(|e| e.name == "HELLO WORLD").unwrap();
    assert_eq!(e.label, "HELLO WORLD");
}

#[test]
fn quoted_entity_block_and_bare_decl() {
    let d = parse("erDiagram\n\"HELLO WORLD\" {\nstring name\n}\n").unwrap();
    let e = d.entities.iter().find(|e| e.name == "HELLO WORLD").unwrap();
    assert_eq!(e.attributes.len(), 1);
}

#[test]
fn multi_id_style_no_ghost_entity() {
    let d = parse("erDiagram\nA ||--o{ B : x\nstyle A,B fill:#f9f\n").unwrap();
    assert!(d.entities.iter().all(|e| e.name != "A,B"));
    let a = d.entities.iter().find(|e| e.name == "A").unwrap();
    let b = d.entities.iter().find(|e| e.name == "B").unwrap();
    assert_eq!(a.style, vec![("fill".to_string(), "#f9f".to_string())]);
    assert_eq!(b.style, vec![("fill".to_string(), "#f9f".to_string())]);
}

#[test]
fn dash_dot_line_forms_are_nonidentifying() {
    for src in [
        "erDiagram\nA ||.-o{ B : uses\n",
        "erDiagram\nA ||-.o{ B : uses\n",
    ] {
        let d = parse(src).unwrap();
        let r = &d.relations[0];
        assert!(!r.identifying, "{src} should be non-identifying");
        assert_eq!(r.left_card, Cardinality::ExactlyOne);
        assert_eq!(r.right_card, Cardinality::ZeroOrMore);
        assert_eq!(r.right, "B");
    }
}

#[test]
fn attribute_comment_parsed() {
    let d = parse("erDiagram\nCUSTOMER {\nstring name \"the customer name\"\n}\n").unwrap();
    let c = d.entities.iter().find(|e| e.name == "CUSTOMER").unwrap();
    assert_eq!(
        c.attributes[0].comment.as_deref(),
        Some("the customer name")
    );
}
