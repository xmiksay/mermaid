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
    -t, --theme <NAME>    Theme: default, base, dark, forest, neutral [default: default]
    -f, --font <FAMILY>   CSS font-family for all text [default: sans-serif]
        --font-size <PX>  Base font size in pixels [default: 14]
    -h, --help            Print help
    -V, --version         Print version

EXAMPLES:
    mermaid-svg < diagram.mmd > diagram.svg
    mermaid-svg diagram.mmd diagram.svg
    mermaid-svg --theme dark diagram.mmd > diagram.svg
    mermaid-svg --font 'Inter, sans-serif' diagram.mmd > diagram.svg
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
    let mut font: Option<String> = None;
    let mut font_size: Option<f64> = None;
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
            "-f" | "--font" => {
                font = Some(
                    args.next()
                        .ok_or_else(|| "--font requires a value".to_string())?,
                );
            }
            s if s.starts_with("--font=") => {
                font = Some(s["--font=".len()..].to_string());
            }
            "--font-size" => {
                font_size =
                    Some(parse_font_size(&args.next().ok_or_else(|| {
                        "--font-size requires a value".to_string()
                    })?)?);
            }
            s if s.starts_with("--font-size=") => {
                font_size = Some(parse_font_size(&s["--font-size=".len()..])?);
            }
            s if s.starts_with('-') && s != "-" => {
                return Err(format!("unknown option: {s} (try --help)"));
            }
            _ => positional.push(a),
        }
    }

    let mut theme = Theme::by_name(&theme_name).ok_or_else(|| {
        format!("unknown theme '{theme_name}' (valid: default, base, dark, forest, neutral)")
    })?;
    if let Some(font) = font {
        theme = theme.with_font(font);
    }
    if let Some(size) = font_size {
        theme = theme.with_font_size(size);
    }

    let input_path = positional.first().map(String::as_str);
    let output_path = positional.get(1).map(String::as_str);

    let input = read_input(input_path)?;
    let svg = render_with(&input, &theme).map_err(|e| e.to_string())?;
    write_output(output_path, &svg)?;
    Ok(())
}

fn parse_font_size(s: &str) -> Result<f64, String> {
    let v: f64 = s
        .parse()
        .map_err(|_| format!("invalid --font-size '{s}' (expected a number)"))?;
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(format!("invalid --font-size '{s}' (must be positive)"))
    }
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
