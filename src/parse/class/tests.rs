use super::*;
use crate::parse::{MemberKind, UmlClass, Visibility};

#[test]
fn v2_header_alias() {
    // `classDiagram-v2` is an upstream alias for `classDiagram`.
    let d = parse("classDiagram-v2\nAnimal <|-- Dog\n").unwrap();
    assert_eq!(d.relations.len(), 1);
    // The dispatcher accepts the alias too.
    assert!(matches!(
        crate::parse("classDiagram-v2\nAnimal <|-- Dog\n").unwrap(),
        crate::Diagram::Class(_)
    ));
}

#[test]
fn block_class_members() {
    let s = "classDiagram\n\
             class Animal {\n\
             +String name\n\
             +int age\n\
             +sleep()\n\
             }\n";
    let d = parse(s).unwrap();
    assert_eq!(d.classes.len(), 1);
    let a = &d.classes[0];
    assert_eq!(a.name, "Animal");
    assert_eq!(a.members.len(), 3);
    assert_eq!(a.members[0].visibility, Visibility::Public);
    assert_eq!(a.members[0].kind, MemberKind::Attribute);
    assert_eq!(a.members[2].kind, MemberKind::Method);
}

#[test]
fn shorthand_members() {
    let s = "classDiagram\n\
             Animal : +String name\n\
             Animal : -age int\n\
             Animal : +sleep()\n";
    let d = parse(s).unwrap();
    assert_eq!(d.classes[0].members.len(), 3);
}

#[test]
fn stereotype_recognized() {
    let s = "classDiagram\nclass Logger {\n<<interface>>\n+log()\n}\n";
    let d = parse(s).unwrap();
    assert_eq!(d.classes[0].stereotype.as_deref(), Some("interface"));
}

fn class<'a>(d: &'a ClassDiagram, name: &str) -> &'a UmlClass {
    d.classes.iter().find(|c| c.name == name).unwrap()
}

#[test]
fn classdef_style_and_cssclass() {
    let s = "classDiagram\nAnimal --> Dog\nclassDef foo fill:#0f0\ncssClass \"Animal,Dog\" foo\nstyle Dog stroke:#333\n";
    let d = parse(s).unwrap();
    assert!(d.class_defs.contains_key("foo"));
    assert_eq!(class(&d, "Animal").classes, vec!["foo".to_string()]);
    assert_eq!(class(&d, "Dog").classes, vec!["foo".to_string()]);
    assert_eq!(
        class(&d, "Dog").style,
        vec![("stroke".to_string(), "#333".to_string())]
    );
}

#[test]
fn class_label_sets_display_not_name() {
    let d = parse("classDiagram\nclass Animal[\"Animal with a label\"]\n").unwrap();
    // Exactly one class, named `Animal`, with the bracket text as its label.
    assert_eq!(d.classes.len(), 1);
    assert_eq!(d.classes[0].name, "Animal");
    assert_eq!(d.classes[0].label.as_deref(), Some("Animal with a label"));
}

#[test]
fn class_label_with_body() {
    let d = parse("classDiagram\nclass Animal[\"A label\"] {\n+eat()\n}\n").unwrap();
    assert_eq!(d.classes.len(), 1);
    assert_eq!(d.classes[0].name, "Animal");
    assert_eq!(d.classes[0].label.as_deref(), Some("A label"));
    assert_eq!(d.classes[0].members.len(), 1);
}

#[test]
fn namespace_label_and_nesting() {
    let d = parse(
        "classDiagram\n\
         namespace Auth[\"Authentication Service\"] {\n\
         class Login\n\
         namespace Inner {\n\
         class Token\n\
         }\n\
         }\n",
    )
    .unwrap();
    let auth = d.namespaces.iter().find(|n| n.name == "Auth").unwrap();
    // Bracket text is the display label; the id stays clean.
    assert_eq!(auth.label.as_deref(), Some("Authentication Service"));
    assert_eq!(auth.depth, 0);
    // The outer namespace encloses the nested namespace's class too.
    assert!(auth.class_names.contains(&"Login".to_string()));
    assert!(auth.class_names.contains(&"Token".to_string()));

    let inner = d.namespaces.iter().find(|n| n.name == "Inner").unwrap();
    assert_eq!(inner.depth, 1);
    assert_eq!(inner.class_names, vec!["Token".to_string()]);
}

#[test]
fn one_line_body_closes_and_does_not_swallow() {
    // `class Duck { +swim() }` opens and closes on one line; the following
    // relation must not become a member row of Duck.
    let d = parse("classDiagram\nclass Duck { +swim() }\nDuck <|-- Goose\n").unwrap();
    let duck = class(&d, "Duck");
    assert_eq!(duck.members.len(), 1);
    assert_eq!(duck.members[0].text, "swim()");
    assert_eq!(d.relations.len(), 1);
    assert_eq!(d.relations[0].from, "Duck");
    assert_eq!(d.relations[0].to, "Goose");
    // Goose is a real class, not a phantom member.
    assert!(d.classes.iter().any(|c| c.name == "Goose"));
}

#[test]
fn empty_one_line_body_closes() {
    let d = parse("classDiagram\nclass Foo {}\nFoo --> Bar\n").unwrap();
    assert!(class(&d, "Foo").members.is_empty());
    assert_eq!(d.relations.len(), 1);
}

#[test]
fn inline_class_on_decl_and_relation() {
    let s = "classDiagram\nclass Animal:::foo\nAnimal --> Dog:::bar : owns\n";
    let d = parse(s).unwrap();
    assert_eq!(class(&d, "Animal").classes, vec!["foo".to_string()]);
    assert_eq!(class(&d, "Dog").classes, vec!["bar".to_string()]);
    assert_eq!(d.relations[0].label.as_deref(), Some("owns"));
}
