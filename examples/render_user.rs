use mermaid_svg::render;

fn main() {
    let src = r#"graph TD
    A[Start] --> B{Decision}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D

    subgraph SubProcess
        C[Execute Task]
    end
"#;
    let svg = render(src).unwrap();
    std::fs::write("/tmp/user_diagram.svg", &svg).unwrap();
    println!("{}", svg);
}
