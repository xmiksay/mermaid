use super::draw::text_color;
use super::format::format_value;
use super::*;

fn leaf(label: &str, value: f64) -> TreemapNode {
    TreemapNode {
        label: label.into(),
        value: Some(value),
        children: vec![],
        class_name: None,
    }
}

#[test]
fn produces_svg() {
    let d = TreemapDiagram {
        title: Some("Tree".into()),
        root: vec![TreemapNode {
            label: "A".into(),
            value: None,
            children: vec![leaf("A1", 3.0), leaf("A2", 7.0)],
            class_name: None,
        }],
        class_defs: HashMap::new(),
        value_format: None,
        show_values: None,
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    // Title uses upstream regular weight, not bold (#332).
    assert!(svg.contains("font-size=\"18\">Tree</text>"));
    assert!(!svg.contains("font-weight=\"bold\">Tree<"));
    assert!(svg.contains(">Tree<"));
    assert!(svg.contains(">A1<"));
}

#[test]
fn class_fill_overrides_palette() {
    let mut class_defs = HashMap::new();
    class_defs.insert(
        "hot".to_string(),
        vec![("fill".to_string(), "#ff0000".to_string())],
    );
    let d = TreemapDiagram {
        title: None,
        root: vec![TreemapNode {
            label: "A".into(),
            value: Some(5.0),
            children: vec![],
            class_name: Some("hot".into()),
        }],
        class_defs,
        value_format: None,
        show_values: None,
    };
    let svg = render(&d, &Theme::default());
    assert!(
        svg.contains("fill=\"#ff0000\""),
        "class fill not applied: {svg}"
    );
    assert!(!svg.contains(":::"));
}

#[test]
fn squarify_tiles_the_whole_area() {
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    };
    let values = vec![6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0];
    let rects = squarify(&values, area);
    assert_eq!(rects.len(), values.len());
    let covered: f64 = rects.iter().map(|r| r.w * r.h).sum();
    assert!((covered - 100.0 * 100.0).abs() < 1e-6, "area mismatch");
    // Every rect stays inside the area.
    for r in &rects {
        assert!(r.x >= -1e-6 && r.y >= -1e-6);
        assert!(r.x + r.w <= 100.0 + 1e-6 && r.y + r.h <= 100.0 + 1e-6);
    }
}

#[test]
fn squarify_keeps_reasonable_aspect_ratios() {
    // Slice-and-dice would give each equal value a 100x(100/8) sliver
    // (ratio 8); squarify must do far better.
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    };
    let values = vec![1.0; 8];
    let rects = squarify(&values, area);
    for r in &rects {
        let ratio = (r.w / r.h).max(r.h / r.w);
        assert!(ratio < 3.0, "sliver aspect ratio {ratio}");
    }
}

#[test]
fn value_format_subset() {
    assert_eq!(format_value(1234.0, Some("$0,0")), "$1,234");
    assert_eq!(format_value(1234.567, Some(",.2f")), "1,234.57");
    assert_eq!(format_value(0.42, Some(".1%")), "42.0%");
    assert_eq!(format_value(0.42, Some("%")), "42%");
    assert_eq!(format_value(1000.0, Some("$,.2f")), "$1,000.00");
    assert_eq!(format_value(-1234.0, Some(",")), "-1,234");
    // No format → upstream default ',' thousands grouping.
    assert_eq!(format_value(12.0, None), "12");
    assert_eq!(format_value(1234567.0, None), "1,234,567");
}

#[test]
fn show_values_false_hides_leaf_value() {
    let d = TreemapDiagram {
        title: None,
        root: vec![leaf("Big", 1234.0)],
        class_defs: HashMap::new(),
        value_format: None,
        show_values: Some(false),
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">Big<"), "label should still render: {svg}");
    assert!(
        !svg.contains(">1,234<"),
        "leaf value should be hidden: {svg}"
    );
}

#[test]
fn orders_siblings_by_value_desc() {
    // Source order Hot(60) then Cold(65); sorted rank puts Cold first.
    let nodes = vec![leaf("Hot", 60.0), leaf("Cold", 65.0)];
    assert_eq!(order_by_value(&nodes), vec![1, 0]);
}

#[test]
fn text_color_flips_on_luminance() {
    let t = Theme::default();
    // Light pastel → dark theme ink; dark fill → white.
    assert_eq!(text_color("#B9B9FF", &t), t.fg);
    assert_eq!(text_color("#101010", &t), "#ffffff");
    // Non-hex passes through to the theme foreground.
    assert_eq!(text_color("url(#g)", &t), t.fg);
}

#[test]
fn each_section_gets_its_own_hue_leaves_uniform() {
    let section = |label: &str, kids: Vec<TreemapNode>| TreemapNode {
        label: label.into(),
        value: None,
        children: kids,
        class_name: None,
    };
    let d = TreemapDiagram {
        title: None,
        root: vec![
            // Cold(65) sorts before Hot(60); Hot nests a Tea section.
            section("Cold", vec![leaf("Water", 40.0), leaf("Soda", 25.0)]),
            section(
                "Hot",
                vec![
                    leaf("Coffee", 35.0),
                    section("Tea", vec![leaf("Black", 12.0), leaf("Green", 8.0)]),
                ],
            ),
        ],
        class_defs: HashMap::new(),
        value_format: None,
        show_values: None,
    };
    let svg = render(&d, &Theme::default());
    // Cold = cScale0, Hot = cScale1, nested Tea takes its own cScale2 hue
    // rather than inheriting Hot's yellow.
    assert!(svg.contains("fill=\"#B9B9FF\""), "Cold hue missing: {svg}");
    assert!(svg.contains("fill=\"#FFFFAB\""), "Hot hue missing: {svg}");
    assert!(
        svg.contains("fill=\"#E8FFB9\""),
        "nested Tea hue missing: {svg}"
    );
    // No progressive darkening: siblings never step to an off-palette shade.
    assert!(
        !svg.contains("fill=\"#A6A6E6"),
        "sibling darkening leaked: {svg}"
    );
}

#[test]
fn section_header_shows_total() {
    let d = TreemapDiagram {
        title: None,
        root: vec![TreemapNode {
            label: "Cold".into(),
            value: None,
            children: vec![leaf("Water", 40.0), leaf("Soda", 25.0)],
            class_name: None,
        }],
        class_defs: HashMap::new(),
        value_format: None,
        show_values: None,
    };
    let svg = render(&d, &Theme::default());
    assert!(
        svg.contains("font-style=\"italic\""),
        "header total not italic: {svg}"
    );
    assert!(svg.contains(">65<"), "section total 65 missing: {svg}");
}

#[test]
fn default_value_format_groups_thousands() {
    let d = TreemapDiagram {
        title: None,
        root: vec![leaf("Big", 1234567.0)],
        class_defs: HashMap::new(),
        value_format: None,
        show_values: None,
    };
    let svg = render(&d, &Theme::default());
    assert!(
        svg.contains(">1,234,567<"),
        "default valueFormat should group thousands: {svg}"
    );
}
