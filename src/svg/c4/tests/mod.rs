use super::*;
use crate::parse::C4Kind;

mod boundaries;
mod layout;
mod relations;

fn person(alias: &str, label: &str) -> C4Element {
    C4Element {
        kind: C4ElementKind::Person,
        alias: alias.into(),
        label: label.into(),
        descr: None,
        technology: None,
        sprite: None,
        tags: None,
        link: None,
        external: false,
        boundary_alias: None,
        boundary_label: None,
        boundary_kind: None,
        boundary_type: None,
        members: vec![],
    }
}

fn boundary(alias: &str, label: &str, kind: C4BoundaryKind, members: Vec<C4Element>) -> C4Element {
    C4Element {
        kind: C4ElementKind::System,
        alias: alias.into(),
        label: label.into(),
        descr: None,
        technology: None,
        sprite: None,
        tags: None,
        link: None,
        external: false,
        boundary_alias: None,
        boundary_label: None,
        boundary_kind: Some(kind),
        boundary_type: None,
        members,
    }
}

#[test]
fn produces_svg() {
    let d = C4Diagram {
        kind: C4Kind::Context,
        title: Some("Sys".into()),
        elements: vec![person("u", "User")],
        relations: vec![],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">User<"));
    assert!(svg.contains(">Sys<"));
    // Title uses upstream regular weight, not bold (#332).
    assert!(svg.contains("font-size=\"18\">Sys</text>"));
    assert!(!svg.contains("font-weight=\"bold\">Sys<"));
}

#[test]
fn element_style_override_applies_colors() {
    let d = C4Diagram {
        kind: C4Kind::Context,
        title: None,
        elements: vec![C4Element {
            kind: C4ElementKind::System,
            alias: "s".into(),
            label: "Sys".into(),
            descr: None,
            technology: None,
            sprite: None,
            tags: None,
            link: None,
            external: false,
            boundary_alias: None,
            boundary_label: None,
            boundary_kind: None,
            boundary_type: None,
            members: vec![],
        }],
        relations: vec![],
        element_styles: vec![C4ElementStyle {
            alias: "s".into(),
            bg_color: Some("#ABCDEF".into()),
            font_color: None,
            border_color: Some("#123456".into()),
        }],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("fill=\"#ABCDEF\""));
    assert!(svg.contains("stroke=\"#123456\""));
}
