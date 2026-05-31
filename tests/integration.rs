use std::fs;
use std::path::PathBuf;

use mermaid_svg::ast::{ArrowKind, ParticipantKind, SequenceItem};
use mermaid_svg::{parse, render, Diagram, ParseError};

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
title API call with blocks
autonumber
actor user as User
participant api as API
participant db as DB
user->>api: GET /users
activate api
api->>db: SELECT
db-->>api: rows
deactivate api
alt cached
    api-->>user: 200 OK (cache)
else miss
    Note over api,db: warm cache
    api-->>user: 200 OK
end
loop every 30s
    api->>db: heartbeat
end
"#;
    let svg = render(src).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("User"));
    assert!(svg.contains(">alt<"));
    assert!(svg.contains(">loop<"));
    assert!(svg.contains(">warm cache<"));
    assert!(svg.contains("1. GET /users"));
    let p = samples_dir().join("sequence_api.svg");
    fs::write(&p, &svg).unwrap();
}

#[test]
fn flowchart_end_to_end() {
    let src = r#"flowchart TD
    Start([Start]) --> Input[/Read input/]
    Input --> Valid{Valid?}
    Valid -->|yes| Process[Process] & Audit[(Audit log)]
    Valid -->|no| Error>Error]
    subgraph Cleanup [Cleanup phase]
        Process --> End([End])
        Audit --> End
    end
    Error --> End
"#;
    let svg = render(src).unwrap();
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
fn state_end_to_end() {
    let src = r#"stateDiagram-v2
    [*] --> Idle
    Idle --> Running : start
    Running --> Idle : stop
    Running --> [*] : crash
    state choose <<choice>>
    Idle --> choose
    state Workflow {
        [*] --> Step1
        Step1 --> Step2
        Step2 --> [*]
    }
    Idle --> Workflow
    note right of Idle: waiting state
"#;
    let svg = render(src).unwrap();
    assert!(svg.contains(">Running<"));
    assert!(svg.contains(">Workflow<"));
    assert!(svg.contains(">waiting state<"));
    let p = samples_dir().join("state_lifecycle.svg");
    fs::write(&p, &svg).unwrap();
}

#[test]
fn class_end_to_end() {
    let src = r#"classDiagram
    namespace Domain {
        class Animal {
            <<abstract>>
            +String name
            +int age
            +eat()
            +sleep()
        }
        class Dog {
            +String breed
            +bark()
        }
    }
    class Cat {
        +bool indoor
        +purr()
    }
    Animal <|-- Dog
    Animal <|-- Cat
    Dog *-- Collar
"#;
    let svg = render(src).unwrap();
    assert!(svg.contains(">Animal<"));
    assert!(svg.contains(">Domain<"));
    let p = samples_dir().join("class_uml.svg");
    fs::write(&p, &svg).unwrap();
}

#[test]
fn er_end_to_end() {
    let src = r#"erDiagram
    CUSTOMER {
        string name "customer full name"
        string email PK "primary identity"
        string phone
    }
    ORDER {
        int id PK
        date created
        int customer_id FK
    }
    LINE_ITEM {
        int id PK
        int order_id FK
        int qty
    }
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE_ITEM : contains
"#;
    let svg = render(src).unwrap();
    assert!(svg.contains(">CUSTOMER<"));
    assert!(svg.contains(">places<"));
    let p = samples_dir().join("er_customer.svg");
    fs::write(&p, &svg).unwrap();
}

#[test]
fn gantt_end_to_end() {
    let src = r#"gantt
    title Release plan
    dateFormat YYYY-MM-DD
    excludes weekends
    todayMarker 2026-01-10
    section Design
    Spec : a1, 2026-01-01, 5d
    Review : after a1, 2d
    section Build
    Backend : crit, b1, 2026-01-08, 1w
    Frontend : active, 2026-01-08, 1w
    Integration : after b1, 3d
"#;
    let svg = render(src).unwrap();
    assert!(svg.contains(">Release plan<"));
    assert!(svg.contains(">Backend<"));
    assert!(svg.contains(">today<"));
    let p = samples_dir().join("gantt_release.svg");
    fs::write(&p, &svg).unwrap();
}

#[test]
fn render_propagates_parse_errors() {
    let err = render("journey\n").unwrap_err();
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
    let err = parse("journey\n").unwrap_err();
    match err {
        ParseError::UnknownDiagramType(s) => assert_eq!(s, "journey"),
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
