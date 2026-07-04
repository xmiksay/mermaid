//! packet-beta renderer. Fixed-width rows of 32 bits with bit ruler.

use crate::parse::PacketDiagram;

use super::builder::SvgBuilder;
use super::theme::Theme;

const TITLE_GAP: f64 = 32.0;
const RULER_H: f64 = 14.0;

pub(crate) fn render(d: &PacketDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let fg_muted = &theme.fg_muted;
    let fill = &theme.flow_node_fill;
    let stroke = &theme.flow_node_stroke;

    let bits_per_row = d.config.bits_per_row;
    let bit_w = d.config.bit_width;
    let row_h = d.config.row_height;
    let pad_x = d.config.padding_x;
    let pad_y = d.config.padding_y;
    let ruler_h = if d.config.show_bits { RULER_H } else { 0.0 };

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let last_bit = d.fields.iter().map(|f| f.end).max().unwrap_or(0);
    let rows: u32 = last_bit / bits_per_row + 1;

    let chart_w = bit_w * bits_per_row as f64;
    let width = pad_x * 2.0 + chart_w;
    let height = pad_y * 2.0 + title_h + ruler_h + rows as f64 * row_h + 10.0;

    let mut svg = SvgBuilder::new(width, height).theme(theme);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            pad_y + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    let chart_left = pad_x;
    let chart_top = pad_y + title_h + ruler_h;

    // Ruler.
    if d.config.show_bits {
        for i in 0..bits_per_row {
            if i % 4 == 0 || i == bits_per_row - 1 {
                let x = chart_left + i as f64 * bit_w + bit_w / 2.0;
                svg.text(
                    x,
                    pad_y + title_h + 10.0,
                    &format!("text-anchor=\"middle\" fill=\"{fg_muted}\" font-size=\"9\""),
                    &i.to_string(),
                );
            }
        }
    }

    // Background: draw a plain cell only where no field covers the bit. Cells
    // drawn under a field would bleed per-bit gridlines through the field's
    // outline; upstream fields are undivided rectangles (issue #248).
    for row in 0..rows {
        for bit in 0..bits_per_row {
            let abs = row * bits_per_row + bit;
            if d.fields.iter().any(|f| abs >= f.start && abs <= f.end) {
                continue;
            }
            let x = chart_left + bit as f64 * bit_w;
            let y = chart_top + row as f64 * row_h;
            svg.rect(
                x,
                y,
                bit_w,
                row_h,
                &format!("fill=\"{fill}\" fill-opacity=\"0.15\" stroke=\"{fg_muted}\" stroke-width=\"0.5\""),
            );
        }
    }

    // Fields.
    for (fi, f) in d.fields.iter().enumerate() {
        let color = theme.pie_color(fi);
        let mut cur = f.start;
        while cur <= f.end {
            let row = cur / bits_per_row;
            let row_start = cur % bits_per_row;
            let row_end = ((row + 1) * bits_per_row - 1).min(f.end);
            let w = (row_end - cur + 1) as f64 * bit_w;
            let x = chart_left + row_start as f64 * bit_w;
            let y = chart_top + row as f64 * row_h;
            svg.rect(
                x,
                y,
                w,
                row_h,
                &format!(
                    "fill=\"{color}\" fill-opacity=\"0.45\" stroke=\"{stroke}\" stroke-width=\"1\""
                ),
            );
            svg.text(
                x + w / 2.0,
                y + row_h / 2.0 + 4.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
                &f.label,
            );
            cur = row_end + 1;
        }
    }

    svg.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{PacketConfig, PacketField};

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
            config: PacketConfig::default(),
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">TCP<"));
        assert!(svg.contains(">src<"));
    }

    #[test]
    fn bits_per_row_config_wraps_rows() {
        let d = PacketDiagram {
            title: None,
            fields: vec![PacketField {
                start: 0,
                end: 31,
                label: "x".into(),
            }],
            config: PacketConfig {
                bits_per_row: 16,
                ..PacketConfig::default()
            },
        };
        // 32 bits over 16-bit rows → two rows, so the field splits into two rects.
        let svg = render(&d, &Theme::default());
        // A 16-bit row is 16 * bit_width = 256px wide plus paddings.
        assert!(svg.contains("width=\"100%\""));
        // Height must exceed a single-row default render.
        let single = render(
            &PacketDiagram {
                title: None,
                fields: d.fields.clone(),
                config: PacketConfig::default(),
            },
            &Theme::default(),
        );
        let vb = |s: &str| {
            let start = s.find("viewBox=\"").unwrap() + 9;
            let end = s[start..].find('"').unwrap() + start;
            s[start..end].to_string()
        };
        assert_ne!(vb(&svg), vb(&single));
    }

    #[test]
    fn covered_bits_draw_no_background_cell() {
        // A full 32-bit field leaves no gap, so the muted background cells
        // (fill-opacity="0.15") — the source of intra-field gridlines — must
        // not be emitted under it (issue #248).
        let full = render(
            &PacketDiagram {
                title: None,
                fields: vec![PacketField {
                    start: 0,
                    end: 31,
                    label: "all".into(),
                }],
                config: PacketConfig::default(),
            },
            &Theme::default(),
        );
        assert!(!full.contains("fill-opacity=\"0.15\""));

        // A single-bit field in a 32-bit row leaves 31 uncovered cells.
        let sparse = render(
            &PacketDiagram {
                title: None,
                fields: vec![PacketField {
                    start: 0,
                    end: 0,
                    label: "b".into(),
                }],
                config: PacketConfig::default(),
            },
            &Theme::default(),
        );
        assert_eq!(sparse.matches("fill-opacity=\"0.15\"").count(), 31);
    }

    #[test]
    fn one_bit_cells_are_32px_wide() {
        let svg = render(
            &PacketDiagram {
                title: None,
                fields: vec![PacketField {
                    start: 0,
                    end: 0,
                    label: "URG".into(),
                }],
                config: PacketConfig::default(),
            },
            &Theme::default(),
        );
        // The field rect is one bit wide; default bit width is now 32px so the
        // three-letter flag label fits inside its own cell.
        assert!(svg.contains("width=\"32\""));
    }

    #[test]
    fn show_bits_false_omits_ruler() {
        let fields = vec![PacketField {
            start: 0,
            end: 3,
            label: "a".into(),
        }];
        let with_ruler = render(
            &PacketDiagram {
                title: None,
                fields: fields.clone(),
                config: PacketConfig::default(),
            },
            &Theme::default(),
        );
        let no_ruler = render(
            &PacketDiagram {
                title: None,
                fields,
                config: PacketConfig {
                    show_bits: false,
                    ..PacketConfig::default()
                },
            },
            &Theme::default(),
        );
        // The ruler prints the bit index "0" as a muted <text>; hiding it drops
        // those tick labels, shrinking the document.
        assert!(with_ruler.contains("font-size=\"9\""));
        assert!(!no_ruler.contains("font-size=\"9\""));
    }
}
