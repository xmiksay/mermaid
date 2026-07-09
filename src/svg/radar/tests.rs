use super::*;
use crate::parse::{RadarAxis, RadarCurve};

#[test]
fn produces_svg() {
    let d = RadarDiagram {
        title: Some("Skills".into()),
        axes: vec![
            RadarAxis {
                id: "a".into(),
                label: "Power".into(),
            },
            RadarAxis {
                id: "b".into(),
                label: "Speed".into(),
            },
            RadarAxis {
                id: "c".into(),
                label: "Endurance".into(),
            },
        ],
        curves: vec![RadarCurve {
            id: "x".into(),
            label: "A".into(),
            values: vec![3.0, 4.0, 2.0],
        }],
        max: Some(5.0),
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">Power<"));
    assert!(svg.contains(">A<"));
    // Title uses upstream regular weight, not bold (#332).
    assert!(svg.contains("font-size=\"18\">Skills</text>"));
    assert!(!svg.contains("font-weight=\"bold\">Skills<"));
}

#[test]
fn default_graticule_is_circles() {
    let d = RadarDiagram {
        axes: vec![
            RadarAxis {
                id: "a".into(),
                label: "A".into(),
            },
            RadarAxis {
                id: "b".into(),
                label: "B".into(),
            },
            RadarAxis {
                id: "c".into(),
                label: "C".into(),
            },
        ],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // Default graticule draws concentric circles, not polygon rings.
    assert!(svg.contains("<circle"));
}

#[test]
fn graticule_has_filled_disc_subtle_rings_and_spoke_ticks() {
    let svg = render(&sample(), &Theme::default());
    // Filled light-gray disc behind the rings.
    assert!(svg.contains("fill-opacity=\"0.12\""));
    // Rings are faint outlines, not solid strokes.
    assert!(svg.contains("stroke-opacity=\"0.35\""));
    // Dark ticks cap each spoke (the only 1.5-wide strokes in the output).
    assert!(svg.contains("stroke-width=\"1.5\""));
}

#[test]
fn polygon_graticule_disc_is_a_filled_path() {
    let svg = render(
        &RadarDiagram {
            graticule: RadarGraticule::Polygon,
            ..sample()
        },
        &Theme::default(),
    );
    // No circle rings for the polygon graticule; the disc is a filled path.
    assert!(!svg.contains("<circle"));
    assert!(svg.contains("fill-opacity=\"0.12\""));
}

#[test]
fn show_legend_false_omits_swatches() {
    let base = RadarDiagram {
        axes: vec![
            RadarAxis {
                id: "a".into(),
                label: "A".into(),
            },
            RadarAxis {
                id: "b".into(),
                label: "B".into(),
            },
        ],
        curves: vec![RadarCurve {
            id: "x".into(),
            label: "Legendary".into(),
            values: vec![1.0, 2.0],
        }],
        ..Default::default()
    };
    let with_legend = render(&base, &Theme::default());
    let without = render(
        &RadarDiagram {
            show_legend: Some(false),
            ..base
        },
        &Theme::default(),
    );
    assert!(with_legend.contains(">Legendary<"));
    assert!(!without.contains(">Legendary<"));
}

fn sample() -> RadarDiagram {
    RadarDiagram {
        axes: vec![
            RadarAxis {
                id: "a".into(),
                label: "A".into(),
            },
            RadarAxis {
                id: "b".into(),
                label: "B".into(),
            },
            RadarAxis {
                id: "c".into(),
                label: "C".into(),
            },
        ],
        curves: vec![RadarCurve {
            id: "x".into(),
            label: "X".into(),
            values: vec![3.0, 4.0, 2.0],
        }],
        max: Some(5.0),
        ..Default::default()
    }
}

/// Extract the `d` attribute of the curve path (the one with the
/// `fill-opacity="0.5"` translucent fill).
fn curve_path_d(svg: &str) -> String {
    for seg in svg.split("<path ").skip(1) {
        let el = &seg[..seg.find("/>").expect("closed path")];
        if el.contains("fill-opacity=\"0.5\"") {
            let d = el.strip_prefix("d=\"").expect("d attr first");
            return d[..d.find('"').expect("closed d")].to_string();
        }
    }
    panic!("no curve path found");
}

#[test]
fn circle_graticule_draws_curved_spline() {
    let d = curve_path_d(&render(&sample(), &Theme::default()));
    // The curve path uses cubic bezier segments (Catmull-Rom), not `L`.
    assert!(d.starts_with('M'));
    assert!(d.contains('C'));
    assert!(!d.contains('L'));
}

#[test]
fn polygon_graticule_draws_straight_curve() {
    let d = curve_path_d(&render(
        &RadarDiagram {
            graticule: RadarGraticule::Polygon,
            ..sample()
        },
        &Theme::default(),
    ));
    assert!(d.contains('L'));
    assert!(!d.contains('C'));
}

#[test]
fn axis_labels_anchor_toward_their_outer_side() {
    // Four axes give a horizontal pair: index 1 sits due-right (anchors
    // `start`), index 3 due-left (anchors `end`), so long labels grow away
    // from the disc instead of over it (#330). Top/bottom stay `middle`.
    let d = RadarDiagram {
        axes: vec![
            RadarAxis {
                id: "t".into(),
                label: "Top".into(),
            },
            RadarAxis {
                id: "r".into(),
                label: "Right".into(),
            },
            RadarAxis {
                id: "b".into(),
                label: "Bottom".into(),
            },
            RadarAxis {
                id: "l".into(),
                label: "Left".into(),
            },
        ],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("text-anchor=\"start\" fill=\"#333\" font-size=\"12\">Right<"));
    assert!(svg.contains("text-anchor=\"end\" fill=\"#333\" font-size=\"12\">Left<"));
    assert!(svg.contains("text-anchor=\"middle\" fill=\"#333\" font-size=\"12\">Top<"));
}

#[test]
fn width_height_config_override_svg_size() {
    let svg = render(
        &RadarDiagram {
            width: Some(500.0),
            height: Some(500.0),
            ..sample()
        },
        &Theme::default(),
    );
    assert!(svg.contains("viewBox=\"0 0 500 500\""));
}

#[test]
fn cardinal_spline_is_closed() {
    let pts = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
    let path = cardinal_closed_path(&pts, DEFAULT_TENSION);
    assert!(path.starts_with("M0 0"));
    assert!(path.ends_with('Z'));
    // One cubic per point around the closed ring.
    assert_eq!(path.matches('C').count(), pts.len());
}
