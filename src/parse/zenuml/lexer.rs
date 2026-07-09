//! Line-comment stripping and brace/statement tokenization for the ZenUML body.

/// A structural token: an opening/closing brace, or a logical statement string
/// tagged with the 1-based source line it started on (for error reporting).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Tok {
    Open,
    Close,
    Stmt(String, usize),
}

/// Strip a `//` or `%%` line comment. Only a `//` run (two slashes) counts, so a
/// single `/` inside a path like `GET /login` survives.
pub(super) fn strip_line_comment(line: &str) -> &str {
    let cut = [line.find("//"), line.find("%%")]
        .into_iter()
        .flatten()
        .min();
    match cut {
        Some(pos) => &line[..pos],
        None => line,
    }
}

/// Split a body into brace/statement tokens. Braces inside parentheses or
/// quotes are kept literal; newlines and `;` terminate statements. Each
/// statement is tagged with the 1-based source line it started on.
pub(super) fn tokenize(body: &str, base_line: usize) -> Vec<Tok> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    let mut paren = 0u32;
    let mut in_quote = false;
    let mut line = base_line;
    let mut stmt_line = base_line;
    let flush = |cur: &mut String, toks: &mut Vec<Tok>, stmt_line: usize| {
        let s = cur.trim();
        if !s.is_empty() {
            toks.push(Tok::Stmt(s.to_string(), stmt_line));
        }
        cur.clear();
    };
    for ch in body.chars() {
        if !ch.is_whitespace() && cur.trim().is_empty() {
            stmt_line = line;
        }
        if ch == '\n' {
            line += 1;
        }
        if in_quote {
            cur.push(ch);
            if ch == '"' {
                in_quote = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_quote = true;
                cur.push(ch);
            }
            '(' => {
                paren += 1;
                cur.push(ch);
            }
            ')' => {
                paren = paren.saturating_sub(1);
                cur.push(ch);
            }
            '{' if paren == 0 => {
                flush(&mut cur, &mut toks, stmt_line);
                toks.push(Tok::Open);
            }
            '}' if paren == 0 => {
                flush(&mut cur, &mut toks, stmt_line);
                toks.push(Tok::Close);
            }
            '\n' | ';' if paren == 0 => flush(&mut cur, &mut toks, stmt_line),
            _ => cur.push(ch),
        }
    }
    flush(&mut cur, &mut toks, stmt_line);
    toks
}
