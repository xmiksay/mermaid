use std::fs;
use std::path::PathBuf;

use mermaid_svg::ast::{ArrowKind, ParticipantKind, SequenceItem};
use mermaid_svg::{parse, render, Diagram, ParseError};

// Provides `SAMPLES` (title, stem, source) and `build_gallery()`, shared with
// the `gen-doc-diagrams` example.
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/gallery_build.rs"));

fn samples_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target/test-samples");
    fs::create_dir_all(&p).unwrap();
    p
}

/// Every shared sample parses and renders to a well-formed SVG document, and the
/// output is written to `target/test-samples/<stem>.svg` for manual inspection.
#[test]
fn every_sample_renders() {
    let out = samples_dir();
    for (_title, stem, src) in SAMPLES {
        let svg = render(src).unwrap_or_else(|e| panic!("render {stem}: {e}"));
        assert!(svg.starts_with("<svg"), "{stem}: missing <svg> prefix");
        assert!(svg.ends_with("</svg>"), "{stem}: missing </svg> suffix");
        assert!(svg.len() > 100, "{stem}: suspiciously small output");
        fs::write(out.join(format!("{stem}.svg")), &svg).unwrap();
    }
}

/// The committed doc gallery must match a fresh render of the samples.
#[test]
fn doc_gallery_up_to_date() {
    let current = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/gallery.md"));
    assert_eq!(
        build_gallery(),
        current,
        "assets/gallery.md is stale; regenerate with `cargo run --example gen-doc-diagrams`"
    );
}

#[test]
fn render_propagates_parse_errors() {
    let err = render("notADiagram\n").unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("unknown diagram type"));
}

#[test]
fn dispatches_pie() {
    let d = parse("pie\n\"A\" : 1\n").unwrap();
    match d {
        Diagram::Pie(p) => assert_eq!(p.entries.len(), 1),
        _ => panic!("expected pie"),
    }
}

#[test]
fn dispatches_sequence() {
    let d = parse("sequenceDiagram\nA->>B: hi\n").unwrap();
    match d {
        Diagram::Sequence(s) => {
            assert_eq!(s.items.len(), 1);
            if let SequenceItem::Message(m) = &s.items[0] {
                assert_eq!(m.arrow, ArrowKind::SolidArrow);
            } else {
                panic!("first item must be a message");
            }
        }
        _ => panic!("expected sequence"),
    }
}

#[test]
fn rejects_empty() {
    assert_eq!(parse("").unwrap_err(), ParseError::Empty);
    assert_eq!(parse("\n\n\n").unwrap_err(), ParseError::Empty);
    assert_eq!(parse("%% only a comment\n").unwrap_err(), ParseError::Empty);
}

#[test]
fn rejects_unknown_diagram() {
    let err = parse("notADiagram\n").unwrap_err();
    match err {
        ParseError::UnknownDiagramType(s) => assert_eq!(s, "notADiagram"),
        e => panic!("unexpected: {e:?}"),
    }
}

#[test]
fn real_world_sequence() {
    let s = r#"sequenceDiagram
    title API call
    actor user as User
    participant api as API
    participant db as DB
    user->>api: GET /users
    api->>db: SELECT * FROM users
    db-->>api: rows
    api-->>user: 200 OK
"#;
    let Diagram::Sequence(d) = parse(s).unwrap() else {
        panic!("expected sequence");
    };
    assert_eq!(d.title.as_deref(), Some("API call"));
    assert_eq!(d.participants.len(), 3);
    assert_eq!(d.participants[0].kind, ParticipantKind::Actor);
    let n_msgs = d
        .items
        .iter()
        .filter(|i| matches!(i, SequenceItem::Message(_)))
        .count();
    assert_eq!(n_msgs, 4);
}

#[test]
fn real_world_pie() {
    let s = r#"pie showData title Browser usage
    "Chrome" : 65.3
    "Firefox" : 12.1
    "Safari" : 18.0
    "Other"  : 4.6
"#;
    let Diagram::Pie(p) = parse(s).unwrap() else {
        panic!("expected pie");
    };
    assert!(p.show_data);
    assert_eq!(p.title.as_deref(), Some("Browser usage"));
    assert_eq!(p.entries.len(), 4);
    let total: f64 = p.entries.iter().map(|e| e.value).sum();
    assert!((total - 100.0).abs() < 1e-6);
}
