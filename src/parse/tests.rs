use super::*;

/// The class of a syntax failure is exposed on the error so callers can
/// branch without string-matching the human-readable `message`.
fn kind_of(input: &str) -> SyntaxKind {
    match parse(input).unwrap_err() {
        ParseError::Syntax { kind, .. } => kind,
        e => panic!("expected Syntax error, got {e:?}"),
    }
}

#[test]
fn classifies_missing_header() {
    // The top-level dispatcher pre-validates the header (yielding
    // `UnknownDiagramType`), so a per-parser header re-check only fires
    // when that parser is called directly with the wrong opener.
    let err = pie::parse("notpie\n").unwrap_err();
    assert!(matches!(
        err,
        ParseError::Syntax {
            kind: SyntaxKind::MissingHeader,
            ..
        }
    ));
}

#[test]
fn leading_bom_is_stripped() {
    // #187: a UTF-8 BOM from a Windows editor must not glue onto the header.
    let d = parse("\u{feff}flowchart TD\nA-->B\n").unwrap();
    assert!(matches!(d, Diagram::Flowchart(_)));
}

#[test]
fn classifies_unknown_statement() {
    assert_eq!(
        kind_of("stateDiagram-v2\n??? garbage\n"),
        SyntaxKind::UnknownStatement
    );
}

#[test]
fn classifies_invalid_number() {
    assert_eq!(
        kind_of("pie\n\"A\" : not-a-number\n"),
        SyntaxKind::InvalidNumber
    );
}

#[test]
fn classifies_unclosed_delimiter() {
    assert_eq!(
        kind_of("quadrantChart\nPoint: [0.1, 0.2\n"),
        SyntaxKind::Unclosed
    );
}

#[test]
fn classifies_malformed_statement() {
    assert_eq!(kind_of("pie\n : 3\n"), SyntaxKind::Malformed);
}

#[test]
fn packet_config_overlays_from_frontmatter() {
    let src = "---\nconfig:\n  packet:\n    bitsPerRow: 16\n    rowHeight: 24\n    showBits: false\n---\npacket-beta\n0-15: \"Src\"\n";
    let (d, _) = parse_with_meta(src).unwrap();
    let Diagram::Packet(p) = d else {
        panic!("expected packet diagram")
    };
    assert_eq!(p.config.bits_per_row, 16);
    assert_eq!(p.config.row_height, 24.0);
    assert!(!p.config.show_bits);
    // Unset knobs keep their defaults.
    assert_eq!(p.config.bit_width, 32.0);
}

#[test]
fn packet_config_defaults_when_absent() {
    let (d, _) = parse_with_meta("packet-beta\n0-15: \"Src\"\n").unwrap();
    let Diagram::Packet(p) = d else {
        panic!("expected packet diagram")
    };
    assert_eq!(p.config, ast::PacketConfig::default());
}

#[test]
fn flowchart_config_curve_reaches_diagram() {
    let src = "%%{init: {\"flowchart\": {\"curve\": \"linear\"}}}%%\nflowchart TD\nA --> B\n";
    let (d, _) = parse_with_meta(src).unwrap();
    let Diagram::Flowchart(f) = d else {
        panic!("expected flowchart diagram")
    };
    assert_eq!(f.config_curve, Some(ast::EdgeCurve::Linear));
}

#[test]
fn pie_config_reaches_diagram() {
    let src = "%%{init: {\"pie\": {\"textPosition\": 0.2, \"donutHole\": 0.5, \"legendPosition\": \"bottom\"}}}%%\npie\n\"A\": 1\n";
    let (d, _) = parse_with_meta(src).unwrap();
    match d {
        Diagram::Pie(p) => {
            assert_eq!(p.text_position, Some(0.2));
            assert_eq!(p.donut_hole, Some(0.5));
            assert_eq!(p.legend_position.as_deref(), Some("bottom"));
        }
        other => panic!("expected pie, got {other:?}"),
    }
}
