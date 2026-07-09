use super::*;
use crate::parse::{XyAxis, XyAxisKind, XySeries, XySeriesKind};

#[test]
fn nice_ticks_produce_round_steps() {
    // The 4000–11000 sample range: d3-style nice steps of 500.
    let (lo, hi, ticks) = nice_ticks(4000.0, 11000.0);
    assert_eq!(lo, 4000.0);
    assert_eq!(hi, 11000.0);
    assert_eq!(
        ticks,
        vec![
            4000.0, 4500.0, 5000.0, 5500.0, 6000.0, 6500.0, 7000.0, 7500.0, 8000.0, 8500.0, 9000.0,
            9500.0, 10000.0, 10500.0, 11000.0
        ]
    );

    // A domain that needs extending: 3..97 → nice 0..100 step 10.
    let (lo, hi, ticks) = nice_ticks(3.0, 97.0);
    assert_eq!((lo, hi), (0.0, 100.0));
    assert_eq!(ticks.first(), Some(&0.0));
    assert_eq!(ticks.last(), Some(&100.0));
    assert!(ticks.windows(2).all(|w| (w[1] - w[0] - 10.0).abs() < 1e-9));

    // A small span picks a fractional step.
    let (_, _, ticks) = nice_ticks(0.0, 1.0);
    assert!(ticks.windows(2).all(|w| (w[1] - w[0] - 0.1).abs() < 1e-9));
}

#[test]
fn renders_nice_value_ticks_in_svg() {
    let d = XyChartDiagram {
        y_axis: Some(XyAxis {
            title: None,
            kind: XyAxisKind::Range {
                min: 4000.0,
                max: 11000.0,
            },
        }),
        series: vec![XySeries {
            kind: XySeriesKind::Bar,
            title: None,
            values: vec![5000.0, 9500.0],
            labels: Vec::new(),
        }],
        ..XyChartDiagram::default()
    };
    let svg = render(&d, &Theme::default());
    // Round tick labels, not the old 1/5-range divisions (e.g. 5400).
    assert!(svg.contains(">4500<"));
    assert!(svg.contains(">10500<"));
    assert!(!svg.contains(">5400<"));
}

#[test]
fn title_uses_regular_weight() {
    // Upstream renders the chart title at regular weight, not bold (#332).
    let d = XyChartDiagram {
        title: Some("Sales".into()),
        ..XyChartDiagram::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("font-size=\"18\">Sales</text>"));
    assert!(!svg.contains("font-weight=\"bold\">Sales<"));
}

#[test]
fn produces_svg() {
    let d = XyChartDiagram {
        horizontal: false,
        title: Some("Sales".into()),
        x_axis: Some(XyAxis {
            title: None,
            kind: XyAxisKind::Categories(vec!["Jan".into(), "Feb".into()]),
        }),
        y_axis: Some(XyAxis {
            title: Some("$".into()),
            kind: XyAxisKind::Range {
                min: 0.0,
                max: 100.0,
            },
        }),
        series: vec![XySeries {
            kind: XySeriesKind::Bar,
            title: None,
            values: vec![40.0, 80.0],
            labels: Vec::new(),
        }],
        ..XyChartDiagram::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">Sales<"));
    assert!(svg.contains(">Jan<"));
}

// Extract the `width`/`height` of every <rect> bar (skip other rects).
fn bar_dims(svg: &str) -> Vec<(f64, f64)> {
    svg.match_indices("<rect ")
        .filter_map(|(i, _)| {
            let tag = &svg[i..svg[i..].find("/>").map(|e| i + e).unwrap_or(svg.len())];
            let attr = |name: &str| -> Option<f64> {
                let key = format!("{name}=\"");
                let start = tag.find(&key)? + key.len();
                let end = tag[start..].find('"')? + start;
                tag[start..end].parse().ok()
            };
            Some((attr("width")?, attr("height")?))
        })
        .collect()
}

#[test]
fn horizontal_bars_grow_in_width() {
    let make = |horizontal: bool| XyChartDiagram {
        horizontal,
        title: None,
        x_axis: Some(XyAxis {
            title: None,
            kind: XyAxisKind::Categories(vec!["A".into(), "B".into()]),
        }),
        y_axis: Some(XyAxis {
            title: None,
            kind: XyAxisKind::Range {
                min: 0.0,
                max: 100.0,
            },
        }),
        series: vec![XySeries {
            kind: XySeriesKind::Bar,
            title: None,
            values: vec![40.0, 80.0],
            labels: Vec::new(),
        }],
        ..XyChartDiagram::default()
    };

    // Horizontal: value maps to bar width; the 80 bar is wider than 40,
    // and both share a constant height (the bar thickness).
    let h = bar_dims(&render(&make(true), &Theme::default()));
    assert_eq!(h.len(), 2);
    assert!(h[1].0 > h[0].0, "width should grow with value: {h:?}");
    assert!((h[0].1 - h[1].1).abs() < 1e-9, "bar height constant: {h:?}");

    // Vertical: the same values map to height instead; widths are equal.
    let v = bar_dims(&render(&make(false), &Theme::default()));
    assert_eq!(v.len(), 2);
    assert!(v[1].1 > v[0].1, "height should grow with value: {v:?}");
    assert!((v[0].0 - v[1].0).abs() < 1e-9, "bar width constant: {v:?}");
}

// Extract the `x` of every <rect> bar in document order.
fn bar_xs(svg: &str) -> Vec<f64> {
    svg.match_indices("<rect ")
        .filter_map(|(i, _)| {
            let tag = &svg[i..svg[i..].find("/>").map(|e| i + e).unwrap_or(svg.len())];
            let key = "x=\"";
            let start = tag.find(key)? + key.len();
            let end = tag[start..].find('"')? + start;
            tag[start..end].parse().ok()
        })
        .collect()
}

#[test]
fn bar_auto_range_baselines_at_zero() {
    // No y-axis: an auto range with a bar series must include zero, so the
    // 40 and 80 bars stand in a 1:2 height ratio (not the 0:full a
    // data-only 40..80 range would give).
    let d = XyChartDiagram {
        horizontal: false,
        title: None,
        x_axis: None,
        y_axis: None,
        series: vec![XySeries {
            kind: XySeriesKind::Bar,
            title: None,
            values: vec![40.0, 80.0],
            labels: Vec::new(),
        }],
        ..XyChartDiagram::default()
    };
    let h = bar_dims(&render(&d, &Theme::default()));
    assert_eq!(h.len(), 2);
    assert!(h[0].1 > 1.0, "smaller bar must be zero-baselined: {h:?}");
    assert!((h[1].1 / h[0].1 - 2.0).abs() < 1e-6, "1:2 ratio: {h:?}");
}

#[test]
fn numeric_x_axis_positions_and_ticks() {
    let d = XyChartDiagram {
        horizontal: false,
        title: None,
        x_axis: Some(XyAxis {
            title: None,
            kind: XyAxisKind::Range {
                min: 0.0,
                max: 10.0,
            },
        }),
        y_axis: None,
        series: vec![XySeries {
            kind: XySeriesKind::Bar,
            title: None,
            values: vec![3.0, 6.0],
            labels: Vec::new(),
        }],
        ..XyChartDiagram::default()
    };
    let svg = render(&d, &Theme::default());
    // Numeric ticks are emitted along the x-axis (the range max is a tick).
    assert!(svg.contains(">10<"), "numeric x tick missing");
    // Points map through the range, not to category centers: the two bars
    // (x = 1 and x = 2 over 0..10) sit in the left tenth of the chart, one
    // step apart — not the ~half-chart spacing category centers would give.
    let xs = bar_xs(&svg);
    assert_eq!(xs.len(), 2);
    let gap = xs[1] - xs[0];
    assert!(
        (gap - CHART_W / 10.0).abs() < 1e-6,
        "points one x-unit apart: {xs:?}"
    );
}

#[test]
fn renders_legend_and_palette() {
    let d = XyChartDiagram {
        series: vec![
            XySeries {
                kind: XySeriesKind::Bar,
                title: Some("Revenue".into()),
                values: vec![40.0, 80.0],
                labels: Vec::new(),
            },
            XySeries {
                kind: XySeriesKind::Line,
                title: Some("Trend".into()),
                values: vec![40.0, 80.0],
                labels: Vec::new(),
            },
        ],
        plot_color_palette: vec!["#111111".into(), "#222222".into()],
        ..XyChartDiagram::default()
    };
    let svg = render(&d, &Theme::default());
    // Legend shows both series titles and the palette drives the fills.
    assert!(svg.contains(">Revenue<"));
    assert!(svg.contains(">Trend<"));
    assert!(svg.contains("fill=\"#111111\""));
    assert!(svg.contains("fill=\"#222222\""));
}

#[test]
fn default_palette_and_no_extra_elements() {
    // Upstream: pale-lavender bars, dark gray-blue line; no dotted gridlines
    // across the plot and no circular point markers (#319).
    let d = XyChartDiagram {
        series: vec![
            XySeries {
                kind: XySeriesKind::Bar,
                title: None,
                values: vec![40.0, 80.0],
                labels: Vec::new(),
            },
            XySeries {
                kind: XySeriesKind::Line,
                title: None,
                values: vec![40.0, 80.0],
                labels: Vec::new(),
            },
        ],
        ..XyChartDiagram::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("fill=\"#ECECFF\""), "bar uses pale lavender");
    assert!(
        svg.contains("stroke=\"#8493A6\""),
        "line uses dark gray-blue"
    );
    assert!(!svg.contains("stroke-dasharray"), "no dotted gridlines");
    assert!(!svg.contains("<circle"), "no point markers");
}

#[test]
fn hides_legend_when_disabled() {
    let d = XyChartDiagram {
        series: vec![XySeries {
            kind: XySeriesKind::Bar,
            title: Some("Revenue".into()),
            values: vec![40.0, 80.0],
            labels: Vec::new(),
        }],
        show_legend: Some(false),
        ..XyChartDiagram::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(!svg.contains(">Revenue<"));
}

#[test]
fn renders_point_labels() {
    let d = XyChartDiagram {
        series: vec![XySeries {
            kind: XySeriesKind::Line,
            title: None,
            values: vec![40.0, 80.0],
            labels: vec![Some("low".into()), Some("high".into())],
        }],
        ..XyChartDiagram::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">low<"));
    assert!(svg.contains(">high<"));
}

#[test]
fn width_height_config_resizes_plot() {
    let base = XyChartDiagram {
        series: vec![XySeries {
            kind: XySeriesKind::Bar,
            title: None,
            values: vec![40.0, 80.0],
            labels: Vec::new(),
        }],
        ..XyChartDiagram::default()
    };
    let wide = XyChartDiagram {
        width: Some(CHART_W * 2.0),
        ..base.clone()
    };
    let root_width = |svg: &str| -> f64 {
        let key = "viewBox=\"0 0 ";
        let start = svg.find(key).unwrap() + key.len();
        let rest = &svg[start..];
        rest[..rest.find(' ').unwrap()].parse().unwrap()
    };
    assert!(
        root_width(&render(&wide, &Theme::default()))
            > root_width(&render(&base, &Theme::default()))
    );
}
