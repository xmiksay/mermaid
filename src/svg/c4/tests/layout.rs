use super::*;
use crate::parse::{C4Kind, C4RelDirection, C4Relation};

fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

fn system(alias: &str, label: &str, external: bool) -> C4Element {
    C4Element {
        kind: C4ElementKind::System,
        alias: alias.into(),
        label: label.into(),
        descr: None,
        technology: None,
        sprite: None,
        tags: None,
        link: None,
        external,
        boundary_alias: None,
        boundary_label: None,
        boundary_kind: None,
        boundary_type: None,
        members: vec![],
    }
}

fn rel(from: &str, to: &str, label: &str) -> C4Relation {
    C4Relation {
        from: from.into(),
        to: to.into(),
        label: label.into(),
        technology: None,
        sprite: None,
        tags: None,
        link: None,
        direction: C4RelDirection::Default,
        bidirectional: false,
        reversed: false,
    }
}

/// The `samples/c4.mmd` System Context diagram (#258).
pub(super) fn context_sample() -> C4Diagram {
    C4Diagram {
        kind: C4Kind::Context,
        title: Some("System Context diagram".into()),
        elements: vec![
            person("customerA", "Banking Customer"),
            person("customerB", "Bank Employee"),
            system("SystemAA", "Internet Banking", false),
            system("SystemB", "Email", true),
            system("SystemC", "Mainframe", true),
        ],
        relations: vec![
            rel("customerA", "SystemAA", "Uses"),
            rel("customerB", "SystemAA", "Supports"),
            rel("SystemAA", "SystemB", "Sends emails"),
            rel("SystemAA", "SystemC", "Reads/writes"),
        ],
        ..Default::default()
    }
}

#[test]
fn sibling_boundaries_do_not_overlap() {
    // Four Deployment_Node boundaries (as in the CyberScore repro), each
    // holding a couple of shapes. None of the frames may overlap.
    let elements = vec![
        boundary(
            "app17",
            "app17",
            C4BoundaryKind::Deployment,
            vec![person("a1", "A1"), person("a2", "A2")],
        ),
        boundary(
            "app06",
            "app06",
            C4BoundaryKind::Deployment,
            vec![person("b1", "B1"), person("b2", "B2")],
        ),
        boundary(
            "app14",
            "app14",
            C4BoundaryKind::Deployment,
            vec![person("c1", "C1")],
        ),
        boundary(
            "app16",
            "app16",
            C4BoundaryKind::Deployment,
            vec![person("d1", "D1")],
        ),
    ];

    let boundaries = grid_of(elements, None).boundaries;
    assert_eq!(boundaries.len(), 4);
    for (i, a) in boundaries.iter().enumerate() {
        for b in &boundaries[i + 1..] {
            let (ra, rb) = (a.rect, b.rect);
            assert!(
                !overlaps(ra, rb),
                "boundary frames overlap: {ra:?} vs {rb:?}"
            );
        }
    }
}

#[test]
fn boundary_contains_its_members() {
    let elements = vec![boundary(
        "app",
        "app",
        C4BoundaryKind::Deployment,
        vec![person("x", "X"), person("y", "Y")],
    )];
    let p = grid_of(elements, None);
    let (bx, by, bw, bh) = p.boundaries[0].rect;
    for (_, (x, y, w, h)) in &p.leaves {
        assert!(*x >= bx && *x + *w <= bx + bw, "member escapes boundary x");
        assert!(*y >= by && *y + *h <= by + bh, "member escapes boundary y");
    }
}

/// #327: the flat System Context diagram uses the upstream row-flow — the
/// external Email system sits to the right of Internet Banking on the same row
/// (not symmetrically below it), and only Mainframe wraps to the next row.
#[test]
fn context_uses_upstream_row_flow() {
    let p = grid_of(context_sample().elements, None);
    let by_alias = |a: &str| {
        let r = rect_of(&p, a);
        (r.0, r.1)
    };
    let (bank_x, bank_y) = by_alias("SystemAA");
    let (email_x, email_y) = by_alias("SystemB");
    let (main_x, main_y) = by_alias("SystemC");
    // Email shares Internet Banking's row, to its right.
    assert_eq!(email_y, bank_y, "Email must share Internet Banking's row");
    assert!(
        email_x > bank_x,
        "Email must sit to the right of the system"
    );
    // Mainframe wraps to the next row.
    assert!(main_y > bank_y, "Mainframe must wrap below");
    let _ = main_x;
}

/// #370: upstream's own row-flow wraps a row once it outgrows the render
/// surface, not just after `SHAPE_IN_ROW` shapes — so the two persons tier
/// onto their own row above the systems, instead of all four leading shapes
/// (2 persons + 2 systems) cramming into one wide row with Mainframe reached
/// by a long diagonal.
#[test]
fn context_tiers_persons_above_systems() {
    let p = grid_of(context_sample().elements, None);
    let by_alias = |a: &str| {
        let r = rect_of(&p, a);
        (r.0, r.1)
    };
    let (customer_x, customer_y) = by_alias("customerA");
    let (employee_x, employee_y) = by_alias("customerB");
    let (_, bank_y) = by_alias("SystemAA");

    // Both persons share a row, distinct from the systems below.
    assert_eq!(customer_y, employee_y, "the two persons must share a row");
    assert!(
        employee_x > customer_x,
        "Bank Employee must sit to the right of Banking Customer"
    );
    assert!(
        bank_y > customer_y,
        "Internet Banking must wrap below the persons row, not share it"
    );
}

#[test]
fn layout_config_controls_shapes_per_row() {
    // With shape_in_row = 1 the two shapes stack vertically; default (4)
    // would place them on the same row. Verify the override changes layout.
    let elements = vec![person("a", "A"), person("b", "B")];
    let p = grid_of(elements, Some(1));
    let (a, b) = (rect_of(&p, "a"), rect_of(&p, "b"));
    assert!(b.1 > a.1, "second shape should wrap to the next row");
    assert_eq!(a.0, b.0, "wrapped shapes share the left edge");
}

#[test]
fn grid_row_takes_its_tallest_shape() {
    // A long description makes `b` taller; its row-mate `a` is stretched to
    // match and the next row starts below both.
    let mut b = system("b", "B", false);
    b.descr = Some("word ".repeat(30));
    let elements = vec![system("a", "A", false), b, system("c", "C", false)];
    let p = grid_of(elements, Some(2));
    let (ra, rb, rc) = (rect_of(&p, "a"), rect_of(&p, "b"), rect_of(&p, "c"));
    assert_eq!(ra.1, rb.1);
    assert_eq!(ra.3, rb.3, "row-mates share the row height");
    assert!(rb.3 > text::shape_size(&system("a", "A", false)).1);
    assert!(
        rc.1 >= rb.1 + rb.3,
        "next row starts below the tallest shape"
    );
}
