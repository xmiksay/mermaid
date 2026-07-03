use super::*;

#[test]
fn parses_context() {
    let src = "C4Context\ntitle My system\nPerson(c, \"Customer\", \"A customer\")\nSystem(s, \"Banking\", \"App\")\nRel(c, s, \"Uses\")\n";
    let d = parse(src).unwrap();
    assert_eq!(d.kind, C4Kind::Context);
    assert_eq!(d.elements.len(), 2);
    assert_eq!(d.relations.len(), 1);
    assert_eq!(d.elements[0].label, "Customer");
    assert_eq!(d.relations[0].label, "Uses");
}

#[test]
fn parses_ext_db_and_queue_variants() {
    let src = "C4Container\n\
        SystemQueue_Ext(sq, \"Bus\")\n\
        ContainerDb_Ext(cd, \"DB\", \"Postgres\")\n\
        ContainerQueue_Ext(cq, \"Q\", \"Kafka\")\n\
        ComponentDb_Ext(pd, \"Store\", \"Redis\")\n\
        ComponentQueue_Ext(pq, \"MQ\", \"NATS\")\n";
    let d = parse(src).unwrap();
    assert_eq!(d.elements.len(), 5);
    assert!(d.elements.iter().all(|e| e.external));
    let kinds: Vec<_> = d.elements.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        vec![
            C4ElementKind::SystemQueue,
            C4ElementKind::ContainerDb,
            C4ElementKind::ContainerQueue,
            C4ElementKind::ComponentDb,
            C4ElementKind::ComponentQueue,
        ]
    );
}

#[test]
fn ext_element_keeps_its_relations() {
    let src = "C4Context\n\
        System(s, \"S\")\n\
        SystemQueue_Ext(q, \"Bus\")\n\
        Rel(s, q, \"publishes\")\n";
    let d = parse(src).unwrap();
    assert_eq!(d.elements.len(), 2);
    assert_eq!(d.relations.len(), 1);
}

#[test]
fn parses_update_directives() {
    let src = "C4Context\n\
        Person(c, \"Customer\")\n\
        System(s, \"Banking\")\n\
        Rel(c, s, \"Uses\")\n\
        UpdateElementStyle(c, $bgColor=\"red\", $fontColor=\"white\", $borderColor=\"black\")\n\
        UpdateRelStyle(c, s, $textColor=\"blue\", $lineColor=\"green\", $offsetX=\"10\", $offsetY=\"-5\")\n\
        UpdateLayoutConfig($c4ShapeInRow=\"3\", $c4BoundaryInRow=\"2\")\n";
    let d = parse(src).unwrap();
    assert_eq!(d.element_styles.len(), 1);
    let es = &d.element_styles[0];
    assert_eq!(es.alias, "c");
    assert_eq!(es.bg_color.as_deref(), Some("red"));
    assert_eq!(es.font_color.as_deref(), Some("white"));
    assert_eq!(es.border_color.as_deref(), Some("black"));

    assert_eq!(d.rel_styles.len(), 1);
    let rs = &d.rel_styles[0];
    assert_eq!((rs.from.as_str(), rs.to.as_str()), ("c", "s"));
    assert_eq!(rs.text_color.as_deref(), Some("blue"));
    assert_eq!(rs.line_color.as_deref(), Some("green"));
    assert_eq!(rs.offset_x, Some(10.0));
    assert_eq!(rs.offset_y, Some(-5.0));

    assert_eq!(d.layout.shape_in_row, Some(3));
    assert_eq!(d.layout.boundary_in_row, Some(2));
}

#[test]
fn parses_boundary() {
    let src = "C4Context\nSystem_Boundary(b, \"Boundary\") {\n  System(s, \"S\", \"d\")\n}\n";
    let d = parse(src).unwrap();
    assert_eq!(d.elements.len(), 1);
    assert!(matches!(
        d.elements[0].boundary_kind,
        Some(C4BoundaryKind::System)
    ));
    assert_eq!(d.elements[0].members.len(), 1);
}

#[test]
fn parses_deployment_node_aliases() {
    let src = "C4Deployment\n\
        Node(dc, \"Datacenter\", \"region\") {\n\
          Node_L(l, \"Left\") {\n\
            Container(a, \"App\")\n\
          }\n\
          Node_R(r, \"Right\") {\n\
            Container(b, \"Db\")\n\
          }\n\
        }\n";
    let d = parse(src).unwrap();
    assert_eq!(d.elements.len(), 1);
    let dc = &d.elements[0];
    assert!(matches!(dc.boundary_kind, Some(C4BoundaryKind::Deployment)));
    assert_eq!(dc.alias, "dc");
    assert_eq!(dc.label, "Datacenter");
    assert_eq!(dc.members.len(), 2);
    assert!(dc
        .members
        .iter()
        .all(|m| matches!(m.boundary_kind, Some(C4BoundaryKind::Deployment))));
    assert_eq!(dc.members[0].alias, "l");
    assert_eq!(dc.members[0].members.len(), 1);
    assert_eq!(dc.members[1].alias, "r");
    assert_eq!(dc.members[1].members.len(), 1);
}
