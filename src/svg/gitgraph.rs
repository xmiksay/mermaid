//! gitGraph renderer. Horizontal commit lanes per branch.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::parse::{CommitKind, GitDirection, GitEvent, GitGraphDiagram};

use super::builder::{fnum, SvgBuilder};
use super::metrics;
use super::theme::Theme;

const PAD: f64 = 30.0;
const COMMIT_R: f64 = 10.0;
const COMMIT_GAP: f64 = 50.0;
const LANE_GAP: f64 = 50.0;
const TITLE_GAP: f64 = 32.0;
/// Thick branch trunk / join line weight (upstream `.arrow` stroke-width).
const LINE_W: f64 = 8.0;
/// Corner radius of a rounded right-angle branch/merge join.
const ELBOW_R: f64 = 10.0;
/// Upstream default tag fill / border (a yellow luggage tag).
const TAG_FILL: &str = "#fff5ad";
const TAG_STROKE: &str = "#aaaa33";

struct CommitNode {
    id: String,
    tags: Vec<String>,
    kind: CommitKind,
    /// Column index (commit position along time axis).
    col: usize,
    /// Lane index (branch row).
    lane: usize,
    /// Parent commit ids (1 normal, 2 for merge).
    parents: Vec<String>,
}

/// Column for the next commit. With `parallelCommits`, it sits one past its
/// deepest parent so independent branches can share a column; otherwise commits
/// advance a global counter (time flows strictly left-to-right).
fn assign_col(
    parallel: bool,
    parents: &[String],
    col_of: &BTreeMap<String, usize>,
    col: &mut usize,
) -> usize {
    if parallel {
        parents
            .iter()
            .filter_map(|p| col_of.get(p))
            .map(|&c| c + 1)
            .max()
            .unwrap_or(0)
    } else {
        let c = *col;
        *col += 1;
        c
    }
}

pub(crate) fn render(d: &GitGraphDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let commit_label = theme.commit_label();
    let tag_label = theme.tag_label();

    let main_branch = d.config.main_branch_name.as_str();

    // Walk events building commits and branch state.
    let mut nodes: Vec<CommitNode> = Vec::new();
    let mut branches: Vec<String> = vec![main_branch.to_string()];
    // Explicit `order:` per branch (parallel to `branches`); None keeps
    // insertion order. Main takes `mainBranchOrder` so it can sit among the
    // ordered branches instead of always claiming lane 0.
    let mut branch_orders: Vec<Option<usize>> = vec![d.config.main_branch_order];
    let mut current_branch = main_branch.to_string();
    // last commit id per branch.
    let mut head: BTreeMap<String, String> = BTreeMap::new();
    // Column per commit id — used to resolve parent depth for parallelCommits.
    let mut col_of: BTreeMap<String, usize> = BTreeMap::new();
    let parallel = d.config.parallel_commits;
    let mut col: usize = 0;
    // Sequence counter across every commit node. Auto ids hash it into an
    // upstream-style `<seq>-<hash>` id (deterministic, not a real RNG).
    let mut seq = 0usize;
    let next_id = |id: Option<String>, seq: usize| -> String {
        id.unwrap_or_else(|| format!("{seq}-{}", seq_hash(seq)))
    };

    for ev in &d.events {
        match ev {
            GitEvent::Commit { id, tags, kind } => {
                let id = next_id(id.clone(), seq);
                seq += 1;
                let parents = head
                    .get(&current_branch)
                    .map(|p| vec![p.clone()])
                    .unwrap_or_default();
                head.insert(current_branch.clone(), id.clone());
                let lane = branches.iter().position(|b| b == &current_branch).unwrap();
                let c = assign_col(parallel, &parents, &col_of, &mut col);
                col_of.insert(id.clone(), c);
                nodes.push(CommitNode {
                    id: id.clone(),
                    tags: tags.clone(),
                    kind: *kind,
                    col: c,
                    lane,
                    parents,
                });
            }
            GitEvent::Branch { name, order } => {
                if let Some(pos) = branches.iter().position(|b| b == name) {
                    if order.is_some() {
                        branch_orders[pos] = *order;
                    }
                } else {
                    branches.push(name.clone());
                    branch_orders.push(*order);
                }
                if let Some(h) = head.get(&current_branch).cloned() {
                    head.insert(name.clone(), h);
                }
                current_branch = name.clone();
            }
            GitEvent::Checkout { name } => {
                current_branch = name.clone();
                if !branches.contains(name) {
                    branches.push(name.clone());
                    branch_orders.push(None);
                }
            }
            GitEvent::Merge {
                from,
                id,
                tags,
                kind,
            } => {
                let id = next_id(id.clone(), seq);
                seq += 1;
                let mut parents = Vec::new();
                if let Some(p) = head.get(&current_branch) {
                    parents.push(p.clone());
                }
                if let Some(p) = head.get(from) {
                    parents.push(p.clone());
                }
                head.insert(current_branch.clone(), id.clone());
                let lane = branches.iter().position(|b| b == &current_branch).unwrap();
                let c = assign_col(parallel, &parents, &col_of, &mut col);
                col_of.insert(id.clone(), c);
                nodes.push(CommitNode {
                    id: id.clone(),
                    tags: tags.clone(),
                    kind: *kind,
                    col: c,
                    lane,
                    parents,
                });
            }
            GitEvent::CherryPick { commit_id, tag, .. } => {
                seq += 1;
                let new_id = format!("cp:{commit_id}");
                let parents = head
                    .get(&current_branch)
                    .map(|p| vec![p.clone(), commit_id.clone()])
                    .unwrap_or_default();
                head.insert(current_branch.clone(), new_id.clone());
                let lane = branches.iter().position(|b| b == &current_branch).unwrap();
                let c = assign_col(parallel, &parents, &col_of, &mut col);
                col_of.insert(new_id.clone(), c);
                nodes.push(CommitNode {
                    id: new_id,
                    tags: tag.iter().cloned().collect(),
                    kind: CommitKind::CherryPick,
                    col: c,
                    lane,
                    parents,
                });
            }
        }
    }

    // Lane order: explicit `order:` wins, otherwise insertion order. Nodes and
    // labels are keyed by insertion index during the walk, then remapped here.
    let lane_of_seq: Vec<usize> = {
        let key = |i: usize| (branch_orders[i].unwrap_or(i), i);
        let mut idxs: Vec<usize> = (0..branches.len()).collect();
        idxs.sort_by_key(|&i| key(i));
        let mut lane = vec![0usize; branches.len()];
        for (rank, &i) in idxs.iter().enumerate() {
            lane[i] = rank;
        }
        lane
    };
    for n in &mut nodes {
        n.lane = lane_of_seq[n.lane];
    }

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let horizontal = matches!(d.direction, GitDirection::LeftRight);
    let bottom_top = matches!(d.direction, GitDirection::BottomTop);
    let cols = nodes.iter().map(|n| n.col).max().unwrap_or(0) + 1;
    let lanes = branches.len();
    let (chart_w, chart_h) = if horizontal {
        (
            cols as f64 * COMMIT_GAP + 80.0,
            lanes as f64 * LANE_GAP + 40.0,
        )
    } else {
        (
            lanes as f64 * LANE_GAP + 120.0,
            cols as f64 * COMMIT_GAP + 40.0,
        )
    };
    let width = PAD * 2.0 + chart_w + 80.0;
    let height = PAD * 2.0 + title_h + chart_h;
    let mut svg = SvgBuilder::new(width, height).theme(theme);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    let origin_x = PAD + 60.0;
    let origin_y = PAD + title_h + 30.0;

    // Position of an arbitrary (col, lane) cell; `pos` specializes it to a node.
    let point = |col: usize, lane: usize| -> (f64, f64) {
        if horizontal {
            (
                origin_x + col as f64 * COMMIT_GAP,
                origin_y + lane as f64 * LANE_GAP,
            )
        } else {
            // BT flows bottom-to-top: newer commits sit higher up the axis.
            let row = if bottom_top {
                (cols - 1 - col) as f64
            } else {
                col as f64
            };
            (
                origin_x + lane as f64 * LANE_GAP,
                origin_y + row * COMMIT_GAP,
            )
        }
    };
    let pos = |n: &CommitNode| -> (f64, f64) { point(n.col, n.lane) };
    // End of the trailing dotted continuation for `lane`: a little past the
    // newest commit column, along the time axis.
    let axis_end = |lane: usize| -> (f64, f64) {
        if horizontal {
            (
                origin_x + (cols.saturating_sub(1) as f64 + 0.75) * COMMIT_GAP,
                origin_y + lane as f64 * LANE_GAP,
            )
        } else {
            let row = if bottom_top {
                -0.75
            } else {
                cols as f64 - 0.25
            };
            (
                origin_x + lane as f64 * LANE_GAP,
                origin_y + row * COMMIT_GAP,
            )
        }
    };

    // Per-lane commit column extent — the span of the thick trunk line.
    let mut lane_min: BTreeMap<usize, usize> = BTreeMap::new();
    let mut lane_max: BTreeMap<usize, usize> = BTreeMap::new();
    for n in &nodes {
        lane_min
            .entry(n.lane)
            .and_modify(|c| *c = (*c).min(n.col))
            .or_insert(n.col);
        lane_max
            .entry(n.lane)
            .and_modify(|c| *c = (*c).max(n.col))
            .or_insert(n.col);
    }

    // Thick branch trunks + trailing dotted continuation.
    for (&lane, &minc) in &lane_min {
        let maxc = lane_max[&lane];
        let color = theme.git_color(lane);
        let (x0, y0) = point(minc, lane);
        let (x1, y1) = point(maxc, lane);
        if minc != maxc {
            svg.line(
                x0,
                y0,
                x1,
                y1,
                &format!(
                    "stroke=\"{color}\" stroke-width=\"{}\" stroke-linecap=\"round\"",
                    fnum(LINE_W)
                ),
            );
        }
        if d.config.show_branches {
            let (ex, ey) = axis_end(lane);
            svg.line(
                x1,
                y1,
                ex,
                ey,
                &format!("stroke=\"{color}\" stroke-width=\"2\" stroke-dasharray=\"4 4\" stroke-linecap=\"round\""),
            );
        }
    }

    // Cross-lane joins: branch starts and merges as rounded right-angle elbows.
    for n in &nodes {
        let (nx, ny) = pos(n);
        for parent in &n.parents {
            if let Some(p) = nodes.iter().find(|m| &m.id == parent) {
                if p.lane == n.lane {
                    continue;
                }
                let (px, py) = pos(p);
                // A merge arrow carries the incoming (source) branch color; a
                // branch start carries the new (child) branch color.
                let color = if matches!(n.kind, CommitKind::Merge) {
                    theme.git_color(p.lane)
                } else {
                    theme.git_color(n.lane)
                };
                let path = elbow_path(px, py, nx, ny, horizontal);
                svg.path(
                    &path,
                    &format!(
                        "fill=\"none\" stroke=\"{color}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"",
                        fnum(LINE_W)
                    ),
                );
            }
        }
    }

    // Branch label pills (suppressed by `showBranches: false`).
    if d.config.show_branches {
        for (i, b) in branches.iter().enumerate() {
            let lane = lane_of_seq[i];
            let fill = theme.git_color(lane);
            let w = metrics::text_width(b, 7.0, theme.font_size).max(10.0) + 16.0;
            let h = 20.0;
            let (cx, cy) = if horizontal {
                (origin_x - 14.0 - w / 2.0, origin_y + lane as f64 * LANE_GAP)
            } else {
                (origin_x + lane as f64 * LANE_GAP, PAD + title_h + 12.0)
            };
            svg.rect(
                cx - w / 2.0,
                cy - h / 2.0,
                w,
                h,
                &format!("fill=\"{fill}\" rx=\"10\" ry=\"10\""),
            );
            let tc = label_text_color(fill);
            svg.text(
                cx,
                cy + 4.0,
                &format!(
                    "text-anchor=\"middle\" fill=\"{tc}\" font-size=\"12\" font-weight=\"bold\""
                ),
                b,
            );
        }
    }

    // Commit nodes.
    for n in &nodes {
        let (x, y) = pos(n);
        let color = theme.git_color(n.lane);
        match n.kind {
            CommitKind::Normal => {
                svg.circle(
                    x,
                    y,
                    COMMIT_R,
                    &format!("fill=\"{color}\" stroke=\"#fff\" stroke-width=\"2\""),
                );
            }
            CommitKind::Highlight => {
                svg.circle(
                    x,
                    y,
                    COMMIT_R + 2.0,
                    &format!("fill=\"{color}\" stroke=\"{fg}\" stroke-width=\"2.5\""),
                );
            }
            CommitKind::Reverse => {
                svg.rect(x - COMMIT_R, y - COMMIT_R, COMMIT_R * 2.0, COMMIT_R * 2.0,
                    &format!("fill=\"{color}\" stroke=\"#fff\" stroke-width=\"2\" transform=\"rotate(45 {} {})\"", fnum(x), fnum(y)));
            }
            CommitKind::Merge => draw_merge_glyph(&mut svg, x, y, color),
            CommitKind::CherryPick => draw_cherry_pick_glyph(&mut svg, x, y, color, fg),
        }
        // Commit id label, nudged clear of the (now larger) dot.
        if d.config.show_commit_label {
            let mut attrs =
                format!("text-anchor=\"middle\" fill=\"{commit_label}\" font-size=\"10\"");
            let ly = y + COMMIT_R + 16.0;
            if d.config.rotate_commit_label && horizontal {
                let _ = write!(attrs, " transform=\"rotate(-45 {} {})\"", fnum(x), fnum(ly));
            }
            svg.text(x, ly, &attrs, &n.id);
        }
        // Tag-shaped labels stack upward from the node (upstream `tags+=STRING`).
        for (ti, t) in n.tags.iter().enumerate() {
            let ty = y - COMMIT_R - 16.0 - ti as f64 * 26.0;
            draw_tag(&mut svg, x, ty, t, tag_label, fg, theme.font_size);
        }
    }

    svg.finish()
}

/// Merge commit: two concentric circles (an outer disc with an inner ring),
/// distinct from a plain commit.
fn draw_merge_glyph(svg: &mut SvgBuilder, x: f64, y: f64, color: &str) {
    svg.circle(
        x,
        y,
        COMMIT_R,
        &format!("fill=\"{color}\" stroke=\"#fff\" stroke-width=\"2\""),
    );
    svg.circle(
        x,
        y,
        COMMIT_R / 2.0,
        &format!("fill=\"#fff\" stroke=\"{color}\" stroke-width=\"1.5\""),
    );
}

/// Deterministic 7-hex digest of a commit's sequence number — mimics upstream's
/// `<seq>-<hash>` auto commit ids (e.g. `0-f56b5f2`) without a real RNG.
fn seq_hash(seq: usize) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in (seq as u64).to_le_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:07x}", h & 0x0fff_ffff)
}

/// Readable text color for a pill filled with `hex` — dark on light fills,
/// white on dark ones (the default git palette is pastel, so most read dark).
fn label_text_color(hex: &str) -> &'static str {
    let h = hex.trim_start_matches('#');
    if h.len() >= 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&h[0..2], 16),
            u8::from_str_radix(&h[2..4], 16),
            u8::from_str_radix(&h[4..6], 16),
        ) {
            let lum = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
            return if lum > 140.0 { "#333333" } else { "#ffffff" };
        }
    }
    "#333333"
}

/// Rounded right-angle join from parent `(px,py)` to child `(nx,ny)`. In the
/// horizontal layout it drops (changes lane) at the parent column, then runs to
/// the child; the vertical layout runs to the child lane, then advances in time.
fn elbow_path(px: f64, py: f64, nx: f64, ny: f64, horizontal: bool) -> String {
    let mut s = String::new();
    let r = ELBOW_R;
    if horizontal {
        let vdir = (ny - py).signum();
        let hdir = (nx - px).signum();
        let _ = write!(
            s,
            "M{} {}L{} {}Q{} {} {} {}L{} {}",
            fnum(px),
            fnum(py),
            fnum(px),
            fnum(ny - r * vdir),
            fnum(px),
            fnum(ny),
            fnum(px + r * hdir),
            fnum(ny),
            fnum(nx),
            fnum(ny),
        );
    } else {
        let hdir = (nx - px).signum();
        let vdir = (ny - py).signum();
        let _ = write!(
            s,
            "M{} {}L{} {}Q{} {} {} {}L{} {}",
            fnum(px),
            fnum(py),
            fnum(nx - r * hdir),
            fnum(py),
            fnum(nx),
            fnum(py),
            fnum(nx),
            fnum(py + r * vdir),
            fnum(nx),
            fnum(ny),
        );
    }
    s
}

/// A tag-shaped label (upstream's yellow luggage tag) centered at `(cx, cy)`:
/// a rounded body with a pointed left edge, a punch hole, and the tag text.
fn draw_tag(
    svg: &mut SvgBuilder,
    cx: f64,
    cy: f64,
    label: &str,
    text_color: &str,
    hole: &str,
    font_size: f64,
) {
    let tw = metrics::text_width(label, 7.0, font_size).max(8.0);
    let body_w = tw + 14.0;
    let point_w = 8.0;
    let th = 18.0;
    let total = body_w + point_w;
    let tip = cx - total / 2.0;
    let body_l = tip + point_w;
    let body_r = tip + total;
    let top = cy - th / 2.0;
    let bot = cy + th / 2.0;
    let mut path = String::new();
    let _ = write!(
        path,
        "M{} {}L{} {}L{} {}L{} {}L{} {}Z",
        fnum(tip),
        fnum(cy),
        fnum(body_l),
        fnum(top),
        fnum(body_r),
        fnum(top),
        fnum(body_r),
        fnum(bot),
        fnum(body_l),
        fnum(bot),
    );
    svg.path(
        &path,
        &format!("fill=\"{TAG_FILL}\" stroke=\"{TAG_STROKE}\" stroke-width=\"1\""),
    );
    svg.circle(body_l + 4.0, cy, 2.0, &format!("fill=\"{hole}\""));
    svg.text(
        (body_l + body_r) / 2.0 + 2.0,
        cy + 3.5,
        &format!("text-anchor=\"middle\" fill=\"{text_color}\" font-size=\"11\""),
        label,
    );
}

/// Cherry-pick commit: a disc carrying the two-cherry glyph (upstream's
/// dedicated cherry-pick marker).
fn draw_cherry_pick_glyph(svg: &mut SvgBuilder, x: f64, y: f64, color: &str, fg: &str) {
    svg.circle(
        x,
        y,
        COMMIT_R,
        &format!("fill=\"{color}\" stroke=\"#fff\" stroke-width=\"2\""),
    );
    let cherry = "fill=\"#fff\"";
    svg.circle(x - 3.0, y + 2.0, 2.5, cherry);
    svg.circle(x + 3.0, y + 2.0, 2.5, cherry);
    let stem = &format!("stroke=\"{fg}\" stroke-width=\"1\"");
    svg.line(x - 3.0, y + 2.0, x + 4.0, y - 4.0, stem);
    svg.line(x + 3.0, y + 2.0, x - 4.0, y - 4.0, stem);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::CommitKind;

    #[test]
    fn renders_simple_graph() {
        let d = GitGraphDiagram {
            title: Some("git".into()),
            direction: GitDirection::LeftRight,
            events: vec![
                GitEvent::Commit {
                    id: None,
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
                GitEvent::Commit {
                    id: None,
                    tags: vec!["v1".into()],
                    kind: CommitKind::Highlight,
                },
                GitEvent::Branch {
                    name: "dev".into(),
                    order: None,
                },
                GitEvent::Commit {
                    id: None,
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
            ],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">git<"));
        assert!(svg.contains(">main<"));
        assert!(svg.contains(">dev<"));
    }

    fn linear(direction: GitDirection) -> GitGraphDiagram {
        GitGraphDiagram {
            title: None,
            direction,
            events: vec![
                GitEvent::Commit {
                    id: Some("a".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
                GitEvent::Commit {
                    id: Some("b".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn bt_flips_the_commit_axis() {
        // BT must not render identically to TB (issue #61, bug 4).
        let tb = render(&linear(GitDirection::TopDown), &Theme::default());
        let bt = render(&linear(GitDirection::BottomTop), &Theme::default());
        assert_ne!(tb, bt);
    }

    #[test]
    fn branch_order_reorders_lanes() {
        // `low` is declared *after* `high` but its smaller order must place it
        // in the earlier lane (issue #61, bug 2).
        let d = GitGraphDiagram {
            title: None,
            direction: GitDirection::TopDown,
            events: vec![
                GitEvent::Commit {
                    id: Some("a".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
                GitEvent::Branch {
                    name: "high".into(),
                    order: Some(5),
                },
                GitEvent::Branch {
                    name: "low".into(),
                    order: Some(1),
                },
            ],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        let low_x = lane_x_before(&svg, svg.find(">low<").unwrap());
        let high_x = lane_x_before(&svg, svg.find(">high<").unwrap());
        assert!(low_x < high_x, "lower order should claim the earlier lane");
    }

    #[test]
    fn main_branch_name_honored() {
        let d = GitGraphDiagram {
            events: vec![GitEvent::Commit {
                id: None,
                tags: Vec::new(),
                kind: CommitKind::Normal,
            }],
            config: crate::parse::GitGraphConfig {
                main_branch_name: "master".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">master<"));
        assert!(!svg.contains(">main<"));
    }

    #[test]
    fn show_branches_and_commit_label_suppressed() {
        let d = GitGraphDiagram {
            events: vec![GitEvent::Commit {
                id: Some("only".into()),
                tags: Vec::new(),
                kind: CommitKind::Normal,
            }],
            config: crate::parse::GitGraphConfig {
                show_branches: false,
                show_commit_label: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        // No branch label and no commit id label.
        assert!(!svg.contains(">main<"));
        assert!(!svg.contains(">only<"));
    }

    #[test]
    fn merge_and_cherry_pick_use_distinct_glyphs() {
        // A merge and a cherry-pick must not reuse the highlight/reverse glyphs.
        let d = GitGraphDiagram {
            events: vec![
                GitEvent::Commit {
                    id: Some("a".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
                GitEvent::Branch {
                    name: "dev".into(),
                    order: None,
                },
                GitEvent::Commit {
                    id: Some("b".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
                GitEvent::Checkout {
                    name: "main".into(),
                },
                GitEvent::Merge {
                    from: "dev".into(),
                    id: Some("m".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Merge,
                },
                GitEvent::CherryPick {
                    commit_id: "b".into(),
                    parent: None,
                    tag: Some("cp".into()),
                },
            ],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        // Cherry-pick tag renders inside a tag shape (plain text, no brackets).
        assert!(svg.contains(">cp<"));
        // The cherry glyph draws small r="2.5" circles; the merge glyph an
        // inner r="5" ring — neither is a rotated square (reverse glyph).
        assert!(svg.contains("r=\"2.5\""));
        assert!(svg.contains("r=\"5\""));
        assert!(!svg.contains("rotate(45"));
    }

    #[test]
    fn merge_type_override_draws_highlight_glyph() {
        // `merge dev type: HIGHLIGHT` must not draw the merge glyph.
        let d = GitGraphDiagram {
            events: vec![
                GitEvent::Commit {
                    id: Some("a".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
                GitEvent::Branch {
                    name: "dev".into(),
                    order: None,
                },
                GitEvent::Commit {
                    id: Some("b".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
                GitEvent::Checkout {
                    name: "main".into(),
                },
                GitEvent::Merge {
                    from: "dev".into(),
                    id: Some("m".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Highlight,
                },
            ],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        // The highlight glyph is an r="12" circle (COMMIT_R + 2); the merge
        // glyph's inner ring is r="5" — absent here.
        assert!(svg.contains("r=\"12\""));
        assert!(!svg.contains("r=\"5\""));
    }

    #[test]
    fn multiple_tags_all_render() {
        let d = GitGraphDiagram {
            events: vec![GitEvent::Commit {
                id: Some("a".into()),
                tags: vec!["v1".into(), "v2".into()],
                kind: CommitKind::Normal,
            }],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">v1<"));
        assert!(svg.contains(">v2<"));
    }

    #[test]
    fn main_branch_order_positions_main_among_lanes() {
        // With mainBranchOrder=2, `main` (order 2) sits after `dev` (insertion
        // lane 1) instead of claiming the first lane.
        let d = GitGraphDiagram {
            direction: GitDirection::TopDown,
            events: vec![
                GitEvent::Commit {
                    id: Some("a".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
                GitEvent::Branch {
                    name: "dev".into(),
                    order: None,
                },
            ],
            config: crate::parse::GitGraphConfig {
                main_branch_order: Some(2),
                ..Default::default()
            },
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        let main_x = lane_x_before(&svg, svg.find(">main<").unwrap());
        let dev_x = lane_x_before(&svg, svg.find(">dev<").unwrap());
        assert!(dev_x < main_x, "dev should claim the earlier lane");
    }

    #[test]
    fn branch_labels_render_as_filled_pills() {
        let d = GitGraphDiagram {
            events: vec![
                GitEvent::Commit {
                    id: Some("a".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
                GitEvent::Branch {
                    name: "dev".into(),
                    order: None,
                },
            ],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        // A rounded pill rect precedes the "main" label text.
        let label = svg.find(">main<").unwrap();
        let rect = svg[..label].rfind("<rect").unwrap();
        assert!(svg[rect..label].contains("rx=\"10\""));
    }

    #[test]
    fn tags_render_as_tag_shapes() {
        let d = GitGraphDiagram {
            events: vec![GitEvent::Commit {
                id: Some("a".into()),
                tags: vec!["v1.0".into()],
                kind: CommitKind::Normal,
            }],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        // Tag body path + yellow luggage-tag fill, no bracketed text.
        assert!(svg.contains(TAG_FILL));
        assert!(svg.contains(">v1.0<"));
        assert!(!svg.contains("[v1.0]"));
    }

    #[test]
    fn thick_trunk_and_trailing_dash() {
        let d = GitGraphDiagram {
            events: vec![
                GitEvent::Commit {
                    id: Some("a".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
                GitEvent::Commit {
                    id: Some("b".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
            ],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        // The branch trunk is thick, and a dotted continuation trails the last
        // commit.
        assert!(svg.contains(&format!("stroke-width=\"{}\"", fnum(LINE_W))));
        assert!(svg.contains("stroke-dasharray=\"4 4\""));
    }

    #[test]
    fn auto_ids_are_seq_hash_style() {
        // Auto ids look like upstream's `0-<hash>`, not `c1`.
        let d = GitGraphDiagram {
            events: vec![GitEvent::Commit {
                id: None,
                tags: Vec::new(),
                kind: CommitKind::Normal,
            }],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">0-"));
        assert!(!svg.contains(">c1<"));
    }

    #[test]
    fn merge_join_is_a_rounded_elbow() {
        // Cross-lane joins use a quadratic-cornered elbow, not an S-curve.
        let d = GitGraphDiagram {
            events: vec![
                GitEvent::Commit {
                    id: Some("a".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
                GitEvent::Branch {
                    name: "dev".into(),
                    order: None,
                },
                GitEvent::Commit {
                    id: Some("b".into()),
                    tags: Vec::new(),
                    kind: CommitKind::Normal,
                },
            ],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        // A rounded elbow path uses an `L…Q…L` shape (no cubic `C`).
        assert!(svg.contains("Q"));
    }

    /// Reads the `x="…"` of the `<text>` element ending just before byte `end`.
    fn lane_x_before(svg: &str, end: usize) -> f64 {
        let text_start = svg[..end].rfind("<text").unwrap();
        let seg = &svg[text_start..end];
        let x_pos = seg.find("x=\"").unwrap() + 3;
        let rest = &seg[x_pos..];
        let x_end = rest.find('"').unwrap();
        rest[..x_end].parse().unwrap()
    }
}
