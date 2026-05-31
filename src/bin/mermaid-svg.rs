//! `mermaid-svg` CLI: convert a Mermaid diagram on stdin/file to SVG on
//! stdout/file.

use std::io::{Read, Write};
use std::process::ExitCode;

use mermaid_svg::{render_with, Theme};

const HELP: &str = "\
mermaid-svg — convert Mermaid diagram text to SVG.

USAGE:
    mermaid-svg [OPTIONS] [INPUT] [OUTPUT]

ARGS:
    INPUT     Input file (default: stdin, or '-')
    OUTPUT    Output file (default: stdout, or '-')

OPTIONS:
    -t, --theme <NAME>    Theme: default, dark, forest, neutral [default: default]
    -h, --help            Print help
    -V, --version         Print version

EXAMPLES:
    mermaid-svg < diagram.mmd > diagram.svg
    mermaid-svg diagram.mmd diagram.svg
    mermaid-svg --theme dark diagram.mmd > diagram.svg
    echo 'pie\\n\"A\":1\\n\"B\":2' | mermaid-svg
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mermaid-svg: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut theme_name = "default".to_string();
    let mut positional: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(());
            }
            "-V" | "--version" => {
                println!("mermaid-svg {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "-t" | "--theme" => {
                theme_name = args
                    .next()
                    .ok_or_else(|| "--theme requires a value".to_string())?;
            }
            s if s.starts_with("--theme=") => {
                theme_name = s["--theme=".len()..].to_string();
            }
            s if s.starts_with('-') && s != "-" => {
                return Err(format!("unknown option: {s} (try --help)"));
            }
            _ => positional.push(a),
        }
    }

    let theme = Theme::by_name(&theme_name).ok_or_else(|| {
        format!(
            "unknown theme '{theme_name}' (valid: default, dark, forest, neutral)"
        )
    })?;

    let input_path = positional.first().map(String::as_str);
    let output_path = positional.get(1).map(String::as_str);

    let input = read_input(input_path)?;
    let svg = render_with(&input, &theme).map_err(|e| e.to_string())?;
    write_output(output_path, &svg)?;
    Ok(())
}

fn read_input(path: Option<&str>) -> Result<String, String> {
    match path {
        None | Some("-") => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| format!("reading stdin: {e}"))?;
            Ok(buf)
        }
        Some(p) => std::fs::read_to_string(p).map_err(|e| format!("reading {p}: {e}")),
    }
}

fn write_output(path: Option<&str>, svg: &str) -> Result<(), String> {
    match path {
        None | Some("-") => {
            let mut out = std::io::stdout().lock();
            out.write_all(svg.as_bytes())
                .map_err(|e| format!("writing stdout: {e}"))?;
            out.write_all(b"\n")
                .map_err(|e| format!("writing stdout: {e}"))?;
            Ok(())
        }
        Some(p) => std::fs::write(p, svg).map_err(|e| format!("writing {p}: {e}")),
    }
}
