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
fn keyword_args_are_position_independent() {
    // `$descr` placed before other keyword args must not shift the label, and
    // `$sprite`/`$tags`/`$link` are captured instead of corrupting descr.
    let src = "C4Context\n\
        Person(a, \"Alice\", $descr=\"A customer\", $sprite=\"person\", $tags=\"v1\", $link=\"https://x\")\n";
    let d = parse(src).unwrap();
    let e = &d.elements[0];
    assert_eq!(e.alias, "a");
    assert_eq!(e.label, "Alice");
    assert_eq!(e.descr.as_deref(), Some("A customer"));
    assert_eq!(e.sprite.as_deref(), Some("person"));
    assert_eq!(e.tags.as_deref(), Some("v1"));
    assert_eq!(e.link.as_deref(), Some("https://x"));
}

#[test]
fn container_keyword_techn_and_descr() {
    // A `$sprite` before the positional technology/description must not shift
    // them; `$techn`/`$descr` override the positional slots.
    let src = "C4Container\n\
        Container(c, \"Api\", $sprite=\"go\", \"REST\", \"The API\")\n\
        Container(c2, \"Web\", \"Ignored\", \"desc\", $techn=\"Vue\")\n";
    let d = parse(src).unwrap();
    assert_eq!(d.elements[0].technology.as_deref(), Some("REST"));
    assert_eq!(d.elements[0].descr.as_deref(), Some("The API"));
    assert_eq!(d.elements[0].sprite.as_deref(), Some("go"));
    // $techn overrides the positional technology.
    assert_eq!(d.elements[1].technology.as_deref(), Some("Vue"));
    assert_eq!(d.elements[1].descr.as_deref(), Some("desc"));
}

#[test]
fn rel_keyword_args() {
    let src = "C4Context\n\
        System(a, \"A\")\n\
        System(b, \"B\")\n\
        Rel(a, b, \"uses\", $techn=\"HTTPS\", $link=\"https://x\", $tags=\"t\")\n";
    let d = parse(src).unwrap();
    let r = &d.relations[0];
    assert_eq!(r.label, "uses");
    assert_eq!(r.technology.as_deref(), Some("HTTPS"));
    assert_eq!(r.link.as_deref(), Some("https://x"));
    assert_eq!(r.tags.as_deref(), Some("t"));
}

#[test]
fn parses_show_legend_and_boundary_style() {
    let src = "C4Context\n\
        System_Boundary(b, \"Group\") {\n\
          System(s, \"S\")\n\
        }\n\
        UpdateBoundaryStyle(b, $bgColor=\"#eee\", $borderColor=\"#333\", $fontColor=\"#000\")\n\
        SHOW_LEGEND()\n";
    let d = parse(src).unwrap();
    assert!(d.show_legend);
    assert_eq!(d.boundary_styles.len(), 1);
    let bs = &d.boundary_styles[0];
    assert_eq!(bs.alias, "b");
    assert_eq!(bs.bg_color.as_deref(), Some("#eee"));
    assert_eq!(bs.border_color.as_deref(), Some("#333"));
    assert_eq!(bs.font_color.as_deref(), Some("#000"));
}

#[test]
fn rel_back_reverses_endpoints() {
    let src = "C4Context\n\
        System(a, \"A\")\n\
        System(b, \"B\")\n\
        Rel_Back(a, b, \"depends on\")\n";
    let d = parse(src).unwrap();
    assert_eq!(d.relations.len(), 1);
    let r = &d.relations[0];
    // Reversed: the arrow now points from `b` back to `a`.
    assert_eq!((r.from.as_str(), r.to.as_str()), ("b", "a"));
    assert_eq!(r.label, "depends on");
}

#[test]
fn rel_index_shifts_positional_args() {
    let src = "C4Dynamic\n\
        System(a, \"A\")\n\
        System(b, \"B\")\n\
        RelIndex(1, a, b, \"Requests\", \"HTTPS\")\n";
    let d = parse(src).unwrap();
    assert_eq!(d.relations.len(), 1);
    let r = &d.relations[0];
    assert_eq!((r.from.as_str(), r.to.as_str()), ("a", "b"));
    // The index prefixes the label; the technology comes from the shifted slot.
    assert_eq!(r.label, "1: Requests");
    assert_eq!(r.technology.as_deref(), Some("HTTPS"));
}

#[test]
fn boundary_type_argument_is_kept() {
    let src = "C4Deployment\n\
        Deployment_Node(n1, \"Web Server\", \"Ubuntu 16.04 LTS\") {\n\
          Container(a, \"App\")\n\
        }\n";
    let d = parse(src).unwrap();
    assert_eq!(d.elements.len(), 1);
    assert_eq!(
        d.elements[0].boundary_type.as_deref(),
        Some("Ubuntu 16.04 LTS")
    );
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
