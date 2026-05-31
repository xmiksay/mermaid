//! gitGraph renderer. Horizontal commit lanes per branch.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::parse::{CommitKind, GitDirection, GitEvent, GitGraphDiagram};

use super::builder::{escape, fnum, SvgBuilder};
use super::theme::Theme;

const PAD: f64 = 30.0;
const COMMIT_R: f64 = 8.0;
const COMMIT_GAP: f64 = 50.0;
const LANE_GAP: f64 = 50.0;
const TITLE_GAP: f64 = 32.0;
const MAIN_BRANCH: &str = "main";

struct CommitNode {
    id: String,
    tag: Option<String>,
    kind: CommitKind,
    /// Column index (commit position along time axis).
    col: usize,
    /// Lane index (branch row).
    lane: usize,
    /// Parent commit ids (1 normal, 2 for merge).
    parents: Vec<String>,
}

pub(crate) fn render(d: &GitGraphDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;

    // Walk events building commits and branch state.
    let mut nodes: Vec<CommitNode> = Vec::new();
    let mut branches: Vec<String> = vec![MAIN_BRANCH.into()];
    let mut current_branch = MAIN_BRANCH.to_string();
    // last commit id per branch.
    let mut head: BTreeMap<String, String> = BTreeMap::new();
    let mut col: usize = 0;
    let mut auto_idx = 0usize;
    let next_id = |id: Option<String>, auto_idx: &mut usize| -> String {
        if let Some(i) = id {
            i
        } else {
            *auto_idx += 1;
            format!("c{}", *auto_idx)
        }
    };

    for ev in &d.events {
        match ev {
            GitEvent::Commit { id, tag, kind } => {
                let id = next_id(id.clone(), &mut auto_idx);
                let parents = head
                    .get(&current_branch)
                    .map(|p| vec![p.clone()])
                    .unwrap_or_default();
                head.insert(current_branch.clone(), id.clone());
                let lane = branches.iter().position(|b| b == &current_branch).unwrap();
                nodes.push(CommitNode {
                    id: id.clone(),
                    tag: tag.clone(),
                    kind: *kind,
                    col,
                    lane,
                    parents,
                });
                col += 1;
            }
            GitEvent::Branch { name } => {
                if !branches.contains(name) {
                    branches.push(name.clone());
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
                }
            }
            GitEvent::Merge { from, id, tag } => {
                let id = next_id(id.clone(), &mut auto_idx);
                let mut parents = Vec::new();
                if let Some(p) = head.get(&current_branch) {
                    parents.push(p.clone());
                }
                if let Some(p) = head.get(from) {
                    parents.push(p.clone());
                }
                head.insert(current_branch.clone(), id.clone());
                let lane = branches.iter().position(|b| b == &current_branch).unwrap();
                nodes.push(CommitNode {
                    id: id.clone(),
                    tag: tag.clone(),
                    kind: CommitKind::Highlight,
                    col,
                    lane,
                    parents,
                });
                col += 1;
            }
            GitEvent::CherryPick { commit_id } => {
                let new_id = format!("cp:{commit_id}");
                let parents = head
                    .get(&current_branch)
                    .map(|p| vec![p.clone(), commit_id.clone()])
                    .unwrap_or_default();
                head.insert(current_branch.clone(), new_id.clone());
                let lane = branches.iter().position(|b| b == &current_branch).unwrap();
                nodes.push(CommitNode {
                    id: new_id,
                    tag: None,
                    kind: CommitKind::Reverse,
                    col,
                    lane,
                    parents,
                });
                col += 1;
            }
        }
    }

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let horizontal = matches!(d.direction, GitDirection::LeftRight);
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
    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);

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

    let pos = |n: &CommitNode| -> (f64, f64) {
        if horizontal {
            (
                origin_x + n.col as f64 * COMMIT_GAP,
                origin_y + n.lane as f64 * LANE_GAP,
            )
        } else {
            (
                origin_x + n.lane as f64 * LANE_GAP,
                origin_y + n.col as f64 * COMMIT_GAP,
            )
        }
    };

    // Branch labels.
    for (i, b) in branches.iter().enumerate() {
        let (x, y) = if horizontal {
            (PAD, origin_y + i as f64 * LANE_GAP + 4.0)
        } else {
            (origin_x + i as f64 * LANE_GAP, PAD + title_h + 14.0)
        };
        let color = theme.pie_color(i);
        svg.text(
            x,
            y,
            &format!("fill=\"{color}\" font-size=\"12\" font-weight=\"bold\""),
            b,
        );
    }

    // Lane lines.
    for (i, _) in branches.iter().enumerate() {
        let color = theme.pie_color(i);
        if horizontal {
            let y = origin_y + i as f64 * LANE_GAP;
            svg.line(
                origin_x,
                y,
                origin_x + (cols.saturating_sub(1) as f64) * COMMIT_GAP,
                y,
                &format!("stroke=\"{color}\" stroke-width=\"2\""),
            );
        } else {
            let x = origin_x + i as f64 * LANE_GAP;
            svg.line(
                x,
                origin_y,
                x,
                origin_y + (cols.saturating_sub(1) as f64) * COMMIT_GAP,
                &format!("stroke=\"{color}\" stroke-width=\"2\""),
            );
        }
    }

    // Parent edges (for merges, draw curve to parent lane).
    for n in &nodes {
        let (nx, ny) = pos(n);
        for parent in &n.parents {
            if let Some(p) = nodes.iter().find(|m| &m.id == parent) {
                let (px, py) = pos(p);
                if p.lane == n.lane {
                    continue;
                }
                let mut path = String::new();
                let _ = write!(
                    path,
                    "M{} {}C{} {}, {} {}, {} {}",
                    fnum(px),
                    fnum(py),
                    fnum((px + nx) / 2.0),
                    fnum(py),
                    fnum((px + nx) / 2.0),
                    fnum(ny),
                    fnum(nx),
                    fnum(ny),
                );
                let color = theme.pie_color(n.lane);
                svg.path(
                    &path,
                    &format!("fill=\"none\" stroke=\"{color}\" stroke-width=\"1.5\""),
                );
            }
        }
    }

    // Commit nodes.
    for n in &nodes {
        let (x, y) = pos(n);
        let color = theme.pie_color(n.lane);
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
        }
        // Commit id label.
        svg.text(
            x,
            y + COMMIT_R + 12.0,
            &format!("text-anchor=\"middle\" fill=\"{fg_muted}\" font-size=\"10\""),
            &n.id,
        );
        if let Some(t) = &n.tag {
            svg.text(
                x,
                y - COMMIT_R - 6.0,
                &format!(
                    "text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\" font-weight=\"bold\""
                ),
                &format!("[{}]", escape(t)),
            );
        }
    }

    svg.finish()
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
                    tag: None,
                    kind: CommitKind::Normal,
                },
                GitEvent::Commit {
                    id: None,
                    tag: Some("v1".into()),
                    kind: CommitKind::Highlight,
                },
                GitEvent::Branch { name: "dev".into() },
                GitEvent::Commit {
                    id: None,
                    tag: None,
                    kind: CommitKind::Normal,
                },
            ],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">git<"));
        assert!(svg.contains(">main<"));
        assert!(svg.contains(">dev<"));
    }
}
