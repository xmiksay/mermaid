//! HSL color math for the timeline block fills. Upstream renders its timeline
//! `cScale` colors darkened 10% in lightness relative to the shared pastel
//! palette (which journey/kanban render pale); [`darken10`] reproduces that so
//! period/section/event blocks read as the saturated fills upstream shows,
//! carrying white or dark ink by contrast the same way upstream does.

/// Darken a `#rrggbb` color by 10 percentage points of HSL lightness. Inputs
/// that are not 6-digit hex pass through unchanged.
pub(super) fn darken10(color: &str) -> String {
    let Some((r, g, b)) = parse_hex6(color) else {
        return color.to_string();
    };
    let (h, s, l) = rgb_to_hsl(r, g, b);
    let (r, g, b) = hsl_to_rgb(h, s, (l - 0.10).max(0.0));
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn parse_hex6(color: &str) -> Option<(u8, u8, u8)> {
    let h = color.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&h[0..2], 16).ok()?,
        u8::from_str_radix(&h[2..4], 16).ok()?,
        u8::from_str_radix(&h[4..6], 16).ok()?,
    ))
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < f64::EPSILON {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to(r1), to(g1), to(b1))
}

#[cfg(test)]
mod tests {
    use super::darken10;

    #[test]
    fn darkens_by_ten_percent_lightness() {
        // Matches upstream's timeline cScale: the shared pastel scale, 10% darker.
        assert_eq!(darken10("#B9B9FF"), "#8686FF"); // purple → white ink upstream
        assert_eq!(darken10("#FFFFAB"), "#FFFF78"); // yellow → dark ink upstream
    }

    #[test]
    fn passthrough_non_hex() {
        assert_eq!(darken10("black"), "black");
        assert_eq!(darken10("#333"), "#333");
    }
}
