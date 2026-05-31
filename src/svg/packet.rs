//! packet-beta renderer. Fixed-width rows of 32 bits with bit ruler.

use crate::parse::PacketDiagram;

use super::builder::SvgBuilder;
use super::theme::Theme;

const PAD: f64 = 30.0;
const TITLE_GAP: f64 = 32.0;
const BIT_W: f64 = 16.0;
const ROW_H: f64 = 40.0;
const RULER_H: f64 = 14.0;
const BITS_PER_ROW: u32 = 32;

pub(crate) fn render(d: &PacketDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;
    let fill = theme.flow_node_fill;
    let stroke = theme.flow_node_stroke;

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let last_bit = d.fields.iter().map(|f| f.end).max().unwrap_or(0);
    let rows: u32 = if last_bit == 0 {
        1
    } else {
        last_bit / BITS_PER_ROW + 1
    };

    let chart_w = BIT_W * BITS_PER_ROW as f64;
    let width = PAD * 2.0 + chart_w;
    let height = PAD * 2.0 + title_h + RULER_H + rows as f64 * ROW_H + 10.0;

    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    let chart_left = PAD;
    let chart_top = PAD + title_h + RULER_H;

    // Ruler.
    for i in 0..BITS_PER_ROW {
        if i % 4 == 0 || i == BITS_PER_ROW - 1 {
            let x = chart_left + i as f64 * BIT_W + BIT_W / 2.0;
            svg.text(
                x,
                PAD + title_h + 10.0,
                &format!("text-anchor=\"middle\" fill=\"{fg_muted}\" font-size=\"9\""),
                &i.to_string(),
            );
        }
    }

    // Rows background (empty cells get muted color).
    for row in 0..rows {
        for bit in 0..BITS_PER_ROW {
            let x = chart_left + bit as f64 * BIT_W;
            let y = chart_top + row as f64 * ROW_H;
            svg.rect(
                x,
                y,
                BIT_W,
                ROW_H,
                &format!("fill=\"{fill}\" fill-opacity=\"0.15\" stroke=\"{fg_muted}\" stroke-width=\"0.5\""),
            );
        }
    }

    // Fields.
    for (fi, f) in d.fields.iter().enumerate() {
        let color = theme.pie_color(fi);
        let mut cur = f.start;
        while cur <= f.end {
            let row = cur / BITS_PER_ROW;
            let row_start = cur % BITS_PER_ROW;
            let row_end = ((row + 1) * BITS_PER_ROW - 1).min(f.end);
            let width_bits = row_end - row_start.min(row_end) + 1
                - (row * BITS_PER_ROW).saturating_sub(row * BITS_PER_ROW);
            let w = (row_end - cur + 1) as f64 * BIT_W;
            let x = chart_left + row_start as f64 * BIT_W;
            let y = chart_top + row as f64 * ROW_H;
            svg.rect(
                x,
                y,
                w,
                ROW_H,
                &format!(
                    "fill=\"{color}\" fill-opacity=\"0.45\" stroke=\"{stroke}\" stroke-width=\"1\""
                ),
            );
            svg.text(
                x + w / 2.0,
                y + ROW_H / 2.0 + 4.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
                &f.label,
            );
            let _ = width_bits;
            cur = row_end + 1;
        }
    }

    svg.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::PacketField;

    #[test]
    fn produces_svg() {
        let d = PacketDiagram {
            title: Some("TCP".into()),
            fields: vec![
                PacketField {
                    start: 0,
                    end: 15,
                    label: "src".into(),
                },
                PacketField {
                    start: 16,
                    end: 31,
                    label: "dst".into(),
                },
            ],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">TCP<"));
        assert!(svg.contains(">src<"));
    }
}
