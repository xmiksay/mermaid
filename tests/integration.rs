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
fn journey_end_to_end() {
    let src = "journey\n    title My day\n    section Morning\n      Wake up: 3: Me\n      Coffee: 5: Me, Cat\n    section Evening\n      Dinner: 4: Me\n";
    let svg = render(src).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">My day<"));
    assert!(svg.contains(">Coffee<"));
    fs::write(samples_dir().join("journey_day.svg"), &svg).unwrap();
}

#[test]
fn timeline_end_to_end() {
    let src = "timeline\n    title History\n    section 2000s\n      2002 : LinkedIn\n      2004 : Facebook : Google\n    section 2010s\n      2010 : Instagram\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">History<"));
    assert!(svg.contains(">LinkedIn<"));
    fs::write(samples_dir().join("timeline_history.svg"), &svg).unwrap();
}

#[test]
fn sankey_end_to_end() {
    let src = "sankey-beta\nsource,target,value\nElectricity,Industry,250\nElectricity,Transport,80\nGas,Heating,120\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">Electricity<"));
    fs::write(samples_dir().join("sankey_energy.svg"), &svg).unwrap();
}

#[test]
fn quadrant_end_to_end() {
    let src = "quadrantChart\n    title Campaigns\n    x-axis Low Reach --> High Reach\n    y-axis Low Engagement --> High Engagement\n    quadrant-1 Expand\n    quadrant-2 Promote\n    quadrant-3 Re-evaluate\n    quadrant-4 Improve\n    Campaign A: [0.3, 0.6]\n    Campaign B: [0.7, 0.8]\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">Campaigns<"));
    fs::write(samples_dir().join("quadrant_campaigns.svg"), &svg).unwrap();
}

#[test]
fn xychart_end_to_end() {
    let src = "xychart-beta\n    title \"Sales\"\n    x-axis [jan, feb, mar, apr, may]\n    y-axis \"Revenue\" 0 --> 1000\n    bar [200, 400, 600, 800, 700]\n    line [200, 400, 600, 800, 700]\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">Sales<"));
    fs::write(samples_dir().join("xychart_sales.svg"), &svg).unwrap();
}

#[test]
fn radar_end_to_end() {
    let src = "radar-beta\n    title \"Skills\"\n    axis A[\"Power\"], B[\"Speed\"], C[\"Endurance\"]\n    curve a[\"Athlete A\"]{85, 90, 80}\n    curve b[\"Athlete B\"]{75, 85, 95}\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">Skills<"));
    fs::write(samples_dir().join("radar_skills.svg"), &svg).unwrap();
}

#[test]
fn packet_end_to_end() {
    let src = "packet-beta\n    title TCP header\n    0-15: \"Source Port\"\n    16-31: \"Destination Port\"\n    32-63: \"Sequence Number\"\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">TCP header<"));
    fs::write(samples_dir().join("packet_tcp.svg"), &svg).unwrap();
}

#[test]
fn mindmap_end_to_end() {
    let src = "mindmap\nroot((mindmap))\n  Origins\n    Long history\n    Popularisation\n  Research\n    Effectiveness\n  Tools\n    Pen and paper\n    Mermaid\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">mindmap<"));
    assert!(svg.contains(">Origins<"));
    fs::write(samples_dir().join("mindmap_tree.svg"), &svg).unwrap();
}

#[test]
fn gitgraph_end_to_end() {
    let src = "gitGraph\n   commit\n   commit\n   branch develop\n   checkout develop\n   commit\n   commit\n   checkout main\n   merge develop\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">main<"));
    assert!(svg.contains(">develop<"));
    fs::write(samples_dir().join("gitgraph_branches.svg"), &svg).unwrap();
}

#[test]
fn requirement_end_to_end() {
    let src = "requirementDiagram\nrequirement test_req {\n    id: 1\n    text: the test text\n    risk: high\n    verifymethod: test\n}\nelement test_entity {\n    type: simulation\n}\ntest_entity - satisfies -> test_req\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">test_req<"));
    fs::write(samples_dir().join("requirement_test.svg"), &svg).unwrap();
}

#[test]
fn c4_end_to_end() {
    let src = "C4Context\n    title System Context\n    Person(customerA, \"Banking Customer\", \"A customer\")\n    System(SystemAA, \"Internet Banking\", \"Allows customers\")\n    System_Ext(SystemB, \"Email\", \"Microsoft Exchange\")\n    Rel(customerA, SystemAA, \"Uses\")\n    Rel(SystemAA, SystemB, \"Sends emails\", \"SMTP\")\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">System Context<"));
    fs::write(samples_dir().join("c4_context.svg"), &svg).unwrap();
}

#[test]
fn block_end_to_end() {
    let src = "block-beta\n    columns 3\n    a b c\n    d[\"Wide\"]:2 e\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">Wide<"));
    fs::write(samples_dir().join("block_grid.svg"), &svg).unwrap();
}

#[test]
fn architecture_end_to_end() {
    let src = "architecture-beta\n    group api(cloud)[API]\n    service db(database)[Database] in api\n    service disk1(disk)[Storage] in api\n    db:L -- R:disk1\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">Database<"));
    assert!(svg.contains(">API<"));
    fs::write(samples_dir().join("architecture_api.svg"), &svg).unwrap();
}

#[test]
fn kanban_end_to_end() {
    let src =
        "kanban\n  Todo\n    [Task A]\n    [Task B]\n  Doing\n    [Task C]\n  Done\n    [Task D]\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">Todo<"));
    assert!(svg.contains(">Task A<"));
    fs::write(samples_dir().join("kanban_board.svg"), &svg).unwrap();
}

#[test]
fn treemap_end_to_end() {
    let src = "treemap-beta\n    title Drinks\n    \"Hot\"\n      \"Coffee\": 30\n      \"Tea\": 20\n    \"Cold\": 50\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">Drinks<"));
    fs::write(samples_dir().join("treemap_drinks.svg"), &svg).unwrap();
}

#[test]
fn zenuml_end_to_end() {
    let src = "zenuml\n    title Auth\n    Alice -> Bob: Login\n    Bob ->> Alice: Welcome\n";
    let svg = render(src).unwrap();
    assert!(svg.contains(">Alice<"));
    fs::write(samples_dir().join("zenuml_auth.svg"), &svg).unwrap();
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
