//! Layout-quality regressions for #396 on the review fixtures: crossing
//! caps, no runs along frame borders, labels on their own line, settled
//! port sides, grid detours, and a time-boxed large diagram.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::invariants::{c4, check_scene, scene};
use super::*;

const ISSUE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/samples/c4/11-container-graph-layout.mmd"
));
const N1: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/c4/n1_component.mmd"
));
const N2: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/c4/n2_deploy.mmd"
));
const N3: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/c4/n3_dynamic.mmd"
));
const R_EMPTY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/c4/r_empty.mmd"
));

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/c4/",
            $name,
            ".mmd"
        ))
    };
}

/// The round-4 review probes, each also run left-right.
const PROBES: [&str; 17] = [
    fixture!("p_over"),
    fixture!("p_uni"),
    fixture!("p_uni_grid"),
    fixture!("p_dup"),
    fixture!("p_dup_g"),
    fixture!("p_bnd"),
    fixture!("p_bnd_grid"),
    fixture!("d1_context"),
    fixture!("d2_component"),
    fixture!("d3_nested"),
    fixture!("n3_deploy"),
    fixture!("r_grid8"),
    fixture!("r_8self"),
    fixture!("r_longtitle"),
    fixture!("q5"),
    fixture!("q6"),
    fixture!("qn6"),
];

fn issue_lr() -> String {
    format!("{ISSUE}\nLAYOUT_LEFT_RIGHT()\n")
}

type Seg = ((f64, f64), (f64, f64));

fn proper_cross((a, b): Seg, (c, d): Seg) -> bool {
    let cr = |o: (f64, f64), p: (f64, f64), q: (f64, f64)| {
        (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0)
    };
    cr(c, d, a) * cr(c, d, b) < 0.0 && cr(a, b, c) * cr(a, b, d) < 0.0
}

/// Line crossings between relations (segment pairs that properly cross).
fn crossings(s: &Scene) -> usize {
    let segs: Vec<Vec<Seg>> = s
        .rels
        .iter()
        .map(|p| p.line.windows(2).map(|w| (w[0], w[1])).collect())
        .collect();
    let mut n = 0;
    for i in 0..segs.len() {
        for j in i + 1..segs.len() {
            for &a in &segs[i] {
                n += segs[j].iter().filter(|&&b| proper_cross(a, b)).count();
            }
        }
    }
    n
}

#[test]
fn crossings_stay_under_their_caps() {
    // Measured when the caps were set (#396 round 3): issue 24, issue LR 27,
    // n1 6, n2 2 (from 48 / 47 / 8 / 5). Small headroom only. Issue LR is 32
    // since round 5: flow -> vulns keeps its frame crossing 12px off
    // vulns -> detect's instead of running 2px beside it, and jogs across it.
    for (name, src, cap) in [
        ("issue", ISSUE.to_string(), 28),
        ("issue LR", issue_lr(), 32),
        ("n1", N1.to_string(), 8),
        ("n2", N2.to_string(), 4),
    ] {
        let d = c4(&src);
        let n = crossings(&scene(&d));
        assert!(n <= cap, "{name}: {n} crossings (cap {cap})");
    }
}

#[test]
fn fixtures_hold_every_invariant() {
    for src in [
        ISSUE.to_string(),
        issue_lr(),
        N1.into(),
        N2.into(),
        N3.into(),
        R_EMPTY.into(),
    ] {
        check_scene(&c4(&src));
    }
}

#[test]
fn probes_hold_every_invariant_in_both_directions() {
    for src in PROBES {
        check_scene(&c4(src));
        check_scene(&c4(&format!("{src}\nLAYOUT_LEFT_RIGHT()\n")));
    }
}

#[test]
fn no_line_runs_along_a_frame_border() {
    // A line staying within 4px of one frame edge for more than 15px reads
    // as part of the border (#396 round 3: 17 such runs in the issue sample).
    for src in [ISSUE.to_string(), issue_lr(), N1.into(), N2.into()] {
        let d = c4(&src);
        let s = scene(&d);
        for p in &s.rels {
            for b in &s.placement.boundaries {
                let (x, y, w, h) = b.rect;
                let on = |q: (f64, f64)| {
                    let horiz =
                        |e: f64| (q.1 - e).abs() < 4.0 && q.0 > x - 2.0 && q.0 < x + w + 2.0;
                    let vert = |e: f64| (q.0 - e).abs() < 4.0 && q.1 > y - 2.0 && q.1 < y + h + 2.0;
                    [horiz(y), horiz(y + h), vert(x), vert(x + w)]
                };
                let (mut run, mut best) = (0.0, 0.0_f64);
                for seg in p.line.windows(2) {
                    let (a, c) = (on(seg[0]), on(seg[1]));
                    // Running *along* the edge: a crossing passes the band
                    // at a steep angle and is not a run.
                    let (dx, dy) = ((seg[1].0 - seg[0].0).abs(), (seg[1].1 - seg[0].1).abs());
                    let same = (0..2).any(|k| a[k] && c[k] && dy < dx)
                        || (2..4).any(|k| a[k] && c[k] && dx < dy);
                    run = if same {
                        run + (seg[1].0 - seg[0].0).hypot(seg[1].1 - seg[0].1)
                    } else {
                        0.0
                    };
                    best = best.max(run);
                }
                assert!(
                    best <= 15.0,
                    "{} -> {} runs {best:.0}px along {}",
                    p.rel.from,
                    p.rel.to,
                    b.alias
                );
            }
        }
    }
}

fn seg_dist(p: (f64, f64), (a, b): Seg) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 < 1e-12 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

#[test]
fn every_label_sits_on_its_own_line() {
    // Its own line passes under it, and no other line is nearer its centre —
    // or it sits off the line on a leader that ends on its own line.
    for src in [
        ISSUE.to_string(),
        issue_lr(),
        N1.into(),
        N2.into(),
        N3.into(),
        R_EMPTY.into(),
    ] {
        let d = c4(&src);
        let s = scene(&d);
        let dist = |c: (f64, f64), line: &[(f64, f64)]| {
            line.windows(2)
                .map(|w| seg_dist(c, (w[0], w[1])))
                .fold(f64::INFINITY, f64::min)
        };
        for (i, p) in s.rels.iter().enumerate() {
            let (Some(rect), Some((_, c))) = (p.label_rect(), p.label.as_ref()) else {
                continue;
            };
            if let Some((on_label, on_line)) = p.leader {
                assert!(dist(on_line, &p.line) < 1.0, "leader ends on its own line");
                let edge = (on_label.0 - rect.0)
                    .abs()
                    .min((on_label.0 - rect.0 - rect.2).abs())
                    .min((on_label.1 - rect.1).abs())
                    .min((on_label.1 - rect.1 - rect.3).abs());
                assert!(edge < 1e-6, "leader starts on the label's edge");
                continue;
            }
            let own = dist(*c, &p.line);
            let under = p
                .line
                .windows(2)
                .any(|w| crate::svg::label_layout::segment_rect_overlap(w[0], w[1], rect) > 0.0);
            assert!(
                under,
                "{} -> {}: its line does not pass under its label",
                p.rel.from, p.rel.to
            );
            for (j, o) in s.rels.iter().enumerate() {
                if j != i {
                    let other = dist(*c, &o.line);
                    assert!(
                        other + 1e-6 >= own,
                        "{} -> {}: {} -> {} is nearer its label ({other:.0} < {own:.0})",
                        p.rel.from,
                        p.rel.to,
                        o.rel.from,
                        o.rel.to
                    );
                }
            }
        }
    }
}

#[test]
fn grid_relations_detour_around_shapes_and_headers() {
    // n3 is a grid-mode diagram: "4:" used to run straight through the
    // Sign-in controller and "3:" through the API Application header.
    let d = c4(N3);
    assert!(!uses_graph_layout(&d));
    let s = scene(&d);
    assert!(s.rels.iter().any(|p| p.curved), "some relation detours");
    check_scene(&d);
    // Rows stay compact: the reversed pair no longer pushes them ~400px apart.
    assert!(s.height < 1400.0, "{}", s.height);
}

/// 200 relations among 60 containers, pseudo-randomly wired.
fn generated(n_rels: usize) -> String {
    let mut src = String::from("C4Container\n");
    for i in 0..60 {
        src.push_str(&format!(
            "Container(c{i}, \"Svc {i}\", \"Rust\", \"desc\")\n"
        ));
    }
    let mut x: u64 = 7;
    for k in 0..n_rels {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let a = (x >> 33) % 60;
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let b = (x >> 33) % 60;
        src.push_str(&format!("Rel(c{a}, c{b}, \"call {k}\")\n"));
    }
    src
}

#[test]
fn two_hundred_relations_render_in_bounded_time() {
    // #396: label placement used to be roughly cubic (3s in release for
    // 200 relations, 33s for 400). Time-boxed generously for debug CI.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let d = c4(&generated(200));
        let t = Instant::now();
        let svg = render(&d, &Theme::default());
        let _ = tx.send((svg.len(), t.elapsed()));
    });
    let (len, took) = rx
        .recv_timeout(Duration::from_secs(60))
        .expect("render finished");
    assert!(len > 0);
    assert!(took < Duration::from_secs(30), "{took:?}");
}

#[test]
fn no_text_is_wider_than_its_box() {
    // #396 round 4: names, technologies, descriptions and relation labels
    // with wide capitals, URLs, identifiers, CJK and emoji overflowed.
    use super::super::text::{inner_width, layout_shape_text, LineRole, RelLabel};
    use super::super::wrap::{measure, Font};
    for src in [
        fixture!("p_over"),
        fixture!("p_uni"),
        fixture!("p_uni_grid"),
    ] {
        for d in [c4(src), c4(&format!("{src}\nLAYOUT_LEFT_RIGHT()\n"))] {
            let s = scene(&d);
            for (el, rect) in &s.placement.leaves {
                for line in layout_shape_text(el, "", rect.2).lines {
                    let font = match line.role {
                        // Drawn at the description's size, never wrapped.
                        LineRole::Stereotype => Font::Descr,
                        LineRole::Name => Font::Name,
                        LineRole::Techn => Font::Techn,
                        LineRole::Descr => Font::Descr,
                    };
                    let w = measure(&line.text, font);
                    assert!(
                        w <= inner_width(rect.2),
                        "{:?}: {w:.0}px in {}",
                        line.text,
                        el.alias
                    );
                }
            }
            for p in &s.rels {
                let Some(l) = RelLabel::new(p.rel, s.placement.narrow_labels) else {
                    continue;
                };
                for line in &l.label {
                    assert!(measure(line, Font::Label) + 6.0 <= l.w + 1e-9);
                    assert!(l.w <= 210.0, "{:?} is {:.0}px", p.rel.label, l.w);
                }
            }
        }
    }
}
