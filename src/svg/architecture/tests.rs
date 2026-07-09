use super::*;
use crate::parse::{ArchEdge, ArchGroup, ArchService, ArchSide};

#[test]
fn produces_svg() {
    let d = ArchitectureDiagram {
        groups: vec![ArchGroup {
            id: "api".into(),
            icon: Some("cloud".into()),
            label: Some("API".into()),
            parent: None,
        }],
        services: vec![
            ArchService {
                id: "db".into(),
                icon: Some("database".into()),
                label: Some("DB".into()),
                parent: Some("api".into()),
            },
            ArchService {
                id: "disk".into(),
                icon: Some("disk".into()),
                label: Some("Disk".into()),
                parent: Some("api".into()),
            },
        ],
        junctions: vec![],
        edges: vec![ArchEdge {
            from: "db".into(),
            from_side: ArchSide::Left,
            from_arrow: false,
            to: "disk".into(),
            to_side: ArchSide::Right,
            to_arrow: false,
            group: false,
            label: None,
        }],
        aligns: vec![],
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">DB<"));
    assert!(svg.contains(">API<"));
    // Group labels use upstream regular weight, not bold (#332).
    assert!(!svg.contains("font-weight=\"bold\">API<"));
}

#[test]
fn service_renders_bare_icon_no_container_box() {
    // #326: a service is a large bare blue icon square (80px) with the label
    // below — no lavender container box (theme flow_node_fill).
    let d = arch("architecture-beta\nservice db(database)[Database]\n");
    let svg = render(&d, &Theme::default());
    assert!(
        svg.contains("width=\"80\""),
        "service icon square (80px) missing"
    );
    assert!(
        !svg.contains("fill=\"#ECECFF\""),
        "service must not draw the lavender container box"
    );
    assert!(svg.contains(ICON_TILE), "blue icon fill missing");
}

#[test]
fn unknown_icon_renders_name_caption() {
    // Iconify pack names can't be fetched by a static renderer; instead of
    // silently drawing a blank box, the name is shown as a caption.
    let src = "\
architecture-beta
    service lambda(logos:aws-lambda)[Lambda]
";
    let d = match crate::parse::parse(src).unwrap() {
        crate::parse::Diagram::Architecture(d) => d,
        _ => panic!("expected architecture diagram"),
    };
    let svg = render(&d, &Theme::default());
    // Caption keeps the segment after the last ':', label stays intact.
    assert!(svg.contains(">aws-lambda<"), "icon-name caption missing");
    assert!(svg.contains(">Lambda<"), "service label missing");
}

#[test]
fn truncate_icon_name_shortens() {
    assert_eq!(truncate_icon_name("logos:aws-lambda"), "aws-lambda");
    assert_eq!(truncate_icon_name("cloud"), "cloud");
    assert_eq!(
        truncate_icon_name("mdi:application-braces-outline"),
        "application-bra…"
    );
}

#[test]
fn edge_title_renders() {
    // The `-[title]-` connector draws the title text on the edge (#184).
    let src = "\
architecture-beta
    service db(database)[DB]
    service server(server)[Srv]
    db:R -[Queries]- L:server
";
    let d = match crate::parse::parse(src).unwrap() {
        crate::parse::Diagram::Architecture(d) => d,
        _ => panic!("expected architecture diagram"),
    };
    assert_eq!(d.edges[0].label.as_deref(), Some("Queries"));
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">Queries<"), "edge title missing");
}

/// Top-left `(x, y)` of every service icon square (`width="80"`) in source
/// order.
fn service_boxes(svg: &str) -> Vec<(f64, f64)> {
    svg.split("<rect ")
        .filter(|chunk| chunk.contains("width=\"80\""))
        .filter_map(|chunk| {
            let x = attr(chunk, "x=\"")?;
            let y = attr(chunk, "y=\"")?;
            Some((x, y))
        })
        .collect()
}

fn attr(chunk: &str, key: &str) -> Option<f64> {
    let start = chunk.find(key)? + key.len();
    let end = chunk[start..].find('"')? + start;
    chunk[start..end].parse().ok()
}

fn arch(src: &str) -> ArchitectureDiagram {
    match crate::parse::parse(src).unwrap() {
        crate::parse::Diagram::Architecture(d) => d,
        _ => panic!("expected architecture diagram"),
    }
}

#[test]
fn align_column_stacks_services_vertically() {
    // `align column a b` puts a and b in a shared column: same x, distinct y
    // with a above b (#227).
    let d =
        arch("architecture-beta\nservice a(server)[A]\nservice b(server)[B]\nalign column a b\n");
    let svg = render(&d, &Theme::default());
    let boxes = service_boxes(&svg);
    assert_eq!(boxes.len(), 2);
    assert!((boxes[0].0 - boxes[1].0).abs() < 0.01, "column shares x");
    assert!(boxes[0].1 < boxes[1].1, "a stacks above b");
}

#[test]
fn align_row_lines_services_horizontally() {
    // `align row a b` puts a and b in a shared row: same y, distinct x with a
    // left of b (#227).
    let d = arch("architecture-beta\nservice a(server)[A]\nservice b(server)[B]\nalign row a b\n");
    let svg = render(&d, &Theme::default());
    let boxes = service_boxes(&svg);
    assert_eq!(boxes.len(), 2);
    assert!((boxes[0].1 - boxes[1].1).abs() < 0.01, "row shares y");
    assert!(boxes[0].0 < boxes[1].0, "a sits left of b");
}

#[test]
fn group_edge_draws_path() {
    // Regression for #62: an `id{group}` edge between two group boxes must
    // resolve its endpoints and draw a (dashed) connector, not vanish.
    let src = "\
architecture-beta
    group left(cloud)[Left]
    group right(cloud)[Right]
    service a(server)[A] in left
    service b(server)[B] in right
    left{group}:R -- L:right{group}
";
    let d = match crate::parse::parse(src).unwrap() {
        crate::parse::Diagram::Architecture(d) => d,
        _ => panic!("expected architecture diagram"),
    };
    assert!(d.edges[0].group);
    let svg = render(&d, &Theme::default());
    assert!(
        svg.contains("stroke-dasharray=\"5 3\""),
        "group edge path missing"
    );
}

#[test]
fn port_hints_pin_grid_layout() {
    // #257: L/R pairs share a row, T/B pairs share a column. From the
    // reference: server left of db (db:L -- R:server), disk1 below server,
    // disk2 below db.
    let d = arch(
        "\
architecture-beta
    group api(cloud)[API]
    service db(database)[Database] in api
    service disk1(disk)[Storage 1] in api
    service disk2(disk)[Storage 2] in api
    service server(server)[Server] in api
    db:L -- R:server
    disk1:T -- B:server
    disk2:T -- B:db
",
    );
    let ids: Vec<String> = d.services.iter().map(|s| s.id.clone()).collect();
    let g = grid_place(&ids, &d.edges);
    let (sc, sr) = g["server"];
    let (dc, dr) = g["db"];
    // Server and db side by side (shared row), server left of db.
    assert_eq!(sr, dr, "server/db share a row");
    assert!(sc < dc, "server sits left of db");
    // Disks hang below their parents (shared column, one row down).
    assert_eq!(g["disk1"].0, sc, "disk1 under server");
    assert_eq!(g["disk1"].1, sr + 1, "disk1 one row below server");
    assert_eq!(g["disk2"].0, dc, "disk2 under db");
    assert_eq!(g["disk2"].1, dr + 1, "disk2 one row below db");

    // The whole diagram still renders and the db↔server edge is a straight
    // horizontal segment (no diagonal): endpoints share a y.
    assert!(render(&d, &Theme::default()).contains("<svg"));
}

#[test]
fn orthogonal_route_has_no_diagonal_segments() {
    // Every segment of a routed edge is axis-aligned (horizontal or vertical).
    let pa = (0.0, 0.0);
    let pb = (100.0, 40.0);
    let pts = ortho_route(pa, ArchSide::Right, pb, ArchSide::Bottom);
    for w in pts.windows(2) {
        let (x0, y0) = w[0];
        let (x1, y1) = w[1];
        let axis_aligned = (x0 - x1).abs() < 1e-9 || (y0 - y1).abs() < 1e-9;
        assert!(axis_aligned, "segment {:?}->{:?} is diagonal", w[0], w[1]);
    }
}
