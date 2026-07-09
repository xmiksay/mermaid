//! Leaf/section value formatting: the supported `valueFormat` d3-format subset
//! (`$` prefix, `,` thousands grouping, `.N` decimals, `%` percentage).

use crate::svg::builder::fnum;

/// Format a leaf value through the supported `valueFormat` subset: `$` prefix,
/// `,` thousands grouping, `.N` decimal places, `%` percentage. Upstream
/// defaults `valueFormat` to `,` (thousands grouping) when unset.
pub(super) fn format_value(v: f64, fmt: Option<&str>) -> String {
    let fmt = fmt.unwrap_or(",");
    let currency = fmt.contains('$');
    let percent = fmt.contains('%');
    let thousands = fmt.contains(',');
    let decimals = fmt.split_once('.').map(|(_, rest)| {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse::<usize>().unwrap_or(0)
    });

    let val = if percent { v * 100.0 } else { v };
    let mut body = match decimals {
        Some(d) => format!("{val:.d$}"),
        None => fnum(val),
    };
    if thousands {
        body = group_thousands(&body);
    }
    let mut out = String::new();
    if currency {
        out.push('$');
    }
    out.push_str(&body);
    if percent {
        out.push('%');
    }
    out
}

/// Insert `,` thousands separators into the integer part of a numeric string,
/// preserving any sign and fractional part.
fn group_thousands(s: &str) -> String {
    let (sign, rest) = match s.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", s),
    };
    let (int_part, frac) = match rest.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (rest, None),
    };
    let len = int_part.chars().count();
    let mut grouped = String::with_capacity(len + len / 3);
    for (idx, ch) in int_part.chars().enumerate() {
        if idx > 0 && (len - idx) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let mut out = format!("{sign}{grouped}");
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}
