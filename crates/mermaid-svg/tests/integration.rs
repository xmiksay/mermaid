use std::fs;
use std::path::PathBuf;

use mermaid_svg::render;

fn samples_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target/test-samples");
    fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn pie_end_to_end() {
    let src = r#"pie showData title Browser usage
"Chrome" : 65.3
"Firefox" : 12.1
"Safari" : 18.0
"Other" : 4.6
"#;
    let svg = render(src).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("Browser usage"));
    assert!(svg.contains("Chrome"));
    let p = samples_dir().join("pie_browsers.svg");
    fs::write(&p, &svg).unwrap();
}

#[test]
fn sequence_end_to_end() {
    let src = r#"sequenceDiagram
title API call
actor user as User
participant api as API
participant db as DB
user->>api: GET /users
api->>db: SELECT
db-->>api: rows
api-->>user: 200 OK
"#;
    let svg = render(src).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("User"));
    assert!(svg.contains("API"));
    assert!(svg.contains("DB"));
    assert!(svg.contains("GET /users"));
    let p = samples_dir().join("sequence_api.svg");
    fs::write(&p, &svg).unwrap();
}

#[test]
fn flowchart_end_to_end() {
    let src = r#"flowchart TD
    Start([Start]) --> Input[/Read input/]
    Input --> Valid{Valid?}
    Valid -->|yes| Process[Process]
    Valid -->|no| Error[Error]
    Process --> End([End])
    Error --> End
"#;
    // The /text/ asymmetric shape isn't supported in the v0.1 parser; replace.
    let src = src.replace("[/Read input/]", "[Read input]");
    let svg = render(&src).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("Start"));
    assert!(svg.contains("Process"));
    assert!(svg.contains(">yes<"));
    let p = samples_dir().join("flowchart_td.svg");
    fs::write(&p, &svg).unwrap();
}

#[test]
fn flowchart_lr_end_to_end() {
    let src = r#"flowchart LR
    A((Start)) --> B[Step 1]
    B --> C[Step 2]
    C --> D{Decide}
    D -->|true| E[(Save)]
    D -->|false| F((Stop))
    E --> F
"#;
    let svg = render(src).unwrap();
    let p = samples_dir().join("flowchart_lr.svg");
    fs::write(&p, &svg).unwrap();
    assert!(svg.contains("Decide"));
}

#[test]
fn render_propagates_parse_errors() {
    let err = render("erDiagram\n").unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("unknown diagram type"));
}
