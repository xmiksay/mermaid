//! Block-frame parsing: the `alt`/`par`/`critical`/`loop`/`opt`/`break`/
//! `rect`/`box` keyword stack and its collapse into `SequenceItem`s.

use crate::parse::ast::{
    AltBranch, SequenceBlock, SequenceBox, SequenceDiagram, SequenceItem, SequenceRect,
};
use crate::parse::ParseError;

pub(super) enum BlockFrame {
    Alt {
        branches: Vec<AltBranch>,
        current_label: String,
        current_items: Vec<SequenceItem>,
    },
    Par {
        branches: Vec<AltBranch>,
        current_label: String,
        current_items: Vec<SequenceItem>,
    },
    Critical {
        branches: Vec<AltBranch>,
        current_label: String,
        current_items: Vec<SequenceItem>,
    },
    Loop {
        label: String,
        items: Vec<SequenceItem>,
    },
    Opt {
        label: String,
        items: Vec<SequenceItem>,
    },
    Break {
        label: String,
        items: Vec<SequenceItem>,
    },
    Rect {
        color: Option<String>,
        items: Vec<SequenceItem>,
    },
    Box {
        color: Option<String>,
        label: String,
        participant_ids: Vec<String>,
    },
}

pub(super) fn handle_block_keyword(
    line: &str,
    stack: &mut Vec<BlockFrame>,
    diag: &mut SequenceDiagram,
) -> Result<bool, ParseError> {
    // `end` closes the topmost frame.
    if line == "end" {
        if let Some(frame) = stack.pop() {
            let item = close_frame(frame);
            push_item(diag, stack, item);
        }
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("alt ") {
        stack.push(BlockFrame::Alt {
            branches: Vec::new(),
            current_label: rest.trim().to_string(),
            current_items: Vec::new(),
        });
        return Ok(true);
    }
    if line == "alt" {
        stack.push(BlockFrame::Alt {
            branches: Vec::new(),
            current_label: String::new(),
            current_items: Vec::new(),
        });
        return Ok(true);
    }
    if let Some(rest) = line.strip_prefix("else") {
        if let Some(BlockFrame::Alt {
            branches,
            current_label,
            current_items,
        }) = stack.last_mut()
        {
            let label = std::mem::take(current_label);
            let items = std::mem::take(current_items);
            branches.push(AltBranch { label, items });
            *current_label = rest.trim().to_string();
        }
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("opt ") {
        stack.push(BlockFrame::Opt {
            label: rest.trim().to_string(),
            items: Vec::new(),
        });
        return Ok(true);
    }
    if line == "opt" {
        stack.push(BlockFrame::Opt {
            label: String::new(),
            items: Vec::new(),
        });
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("loop ") {
        stack.push(BlockFrame::Loop {
            label: rest.trim().to_string(),
            items: Vec::new(),
        });
        return Ok(true);
    }
    if line == "loop" {
        stack.push(BlockFrame::Loop {
            label: String::new(),
            items: Vec::new(),
        });
        return Ok(true);
    }

    // `par_over` is upstream's overlapping-par frame; it shares the `par`/`and`
    // branch structure, so it reuses the same frame.
    if let Some(rest) = line
        .strip_prefix("par_over ")
        .or_else(|| line.strip_prefix("par "))
    {
        stack.push(BlockFrame::Par {
            branches: Vec::new(),
            current_label: rest.trim().to_string(),
            current_items: Vec::new(),
        });
        return Ok(true);
    }
    if line == "par_over" {
        stack.push(BlockFrame::Par {
            branches: Vec::new(),
            current_label: String::new(),
            current_items: Vec::new(),
        });
        return Ok(true);
    }
    if let Some(rest) = line.strip_prefix("and ") {
        if let Some(BlockFrame::Par {
            branches,
            current_label,
            current_items,
        }) = stack.last_mut()
        {
            let label = std::mem::take(current_label);
            let items = std::mem::take(current_items);
            branches.push(AltBranch { label, items });
            *current_label = rest.trim().to_string();
        }
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("critical ") {
        stack.push(BlockFrame::Critical {
            branches: Vec::new(),
            current_label: rest.trim().to_string(),
            current_items: Vec::new(),
        });
        return Ok(true);
    }
    if let Some(rest) = line.strip_prefix("option ") {
        if let Some(BlockFrame::Critical {
            branches,
            current_label,
            current_items,
        }) = stack.last_mut()
        {
            let label = std::mem::take(current_label);
            let items = std::mem::take(current_items);
            branches.push(AltBranch { label, items });
            *current_label = rest.trim().to_string();
        }
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("break ") {
        stack.push(BlockFrame::Break {
            label: rest.trim().to_string(),
            items: Vec::new(),
        });
        return Ok(true);
    }
    if line == "break" {
        stack.push(BlockFrame::Break {
            label: String::new(),
            items: Vec::new(),
        });
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("rect ") {
        // `rect <color>` — the whole argument is the fill (a bare label with no
        // color makes no sense for a background band).
        let arg = rest.trim();
        let color = (!arg.is_empty()).then(|| arg.to_string());
        stack.push(BlockFrame::Rect {
            color,
            items: Vec::new(),
        });
        return Ok(true);
    }
    if line == "rect" {
        stack.push(BlockFrame::Rect {
            color: None,
            items: Vec::new(),
        });
        return Ok(true);
    }

    if let Some(rest) = line.strip_prefix("box ") {
        let (color, label) = split_box_color(rest.trim());
        stack.push(BlockFrame::Box {
            color,
            label,
            participant_ids: Vec::new(),
        });
        return Ok(true);
    }
    if line == "box" {
        stack.push(BlockFrame::Box {
            color: None,
            label: String::new(),
            participant_ids: Vec::new(),
        });
        return Ok(true);
    }

    Ok(false)
}

fn close_frame(frame: BlockFrame) -> SequenceItem {
    match frame {
        BlockFrame::Alt {
            mut branches,
            current_label,
            current_items,
        } => {
            branches.push(AltBranch {
                label: current_label,
                items: current_items,
            });
            SequenceItem::Alt(branches)
        }
        BlockFrame::Par {
            mut branches,
            current_label,
            current_items,
        } => {
            branches.push(AltBranch {
                label: current_label,
                items: current_items,
            });
            SequenceItem::Par(branches)
        }
        BlockFrame::Critical {
            mut branches,
            current_label,
            current_items,
        } => {
            branches.push(AltBranch {
                label: current_label,
                items: current_items,
            });
            SequenceItem::Critical(branches)
        }
        BlockFrame::Loop { label, items } => SequenceItem::Loop(SequenceBlock { label, items }),
        BlockFrame::Opt { label, items } => SequenceItem::Opt(SequenceBlock { label, items }),
        BlockFrame::Break { label, items } => SequenceItem::Break(SequenceBlock { label, items }),
        BlockFrame::Rect { color, items } => SequenceItem::Rect(SequenceRect { color, items }),
        BlockFrame::Box {
            color,
            label,
            participant_ids,
        } => SequenceItem::Box(SequenceBox {
            color,
            label,
            participant_ids,
        }),
    }
}

pub(super) fn attach_pending(diag: &mut SequenceDiagram, frame: BlockFrame) {
    let item = close_frame(frame);
    diag.items.push(item);
}

pub(super) fn push_item(diag: &mut SequenceDiagram, stack: &mut [BlockFrame], item: SequenceItem) {
    if let Some(frame) = stack.last_mut() {
        match frame {
            BlockFrame::Alt { current_items, .. }
            | BlockFrame::Par { current_items, .. }
            | BlockFrame::Critical { current_items, .. } => current_items.push(item),
            BlockFrame::Loop { items, .. }
            | BlockFrame::Opt { items, .. }
            | BlockFrame::Break { items, .. }
            | BlockFrame::Rect { items, .. } => items.push(item),
            // A box only groups participants; any messages/notes inside it are
            // ordinary events that belong at the diagram level.
            BlockFrame::Box { .. } => diag.items.push(item),
        }
    } else {
        diag.items.push(item);
    }
}

/// Split a `box <color> <label>` header into an optional leading color and the
/// remaining label. Mermaid treats the first token as a color when it is a hex
/// value, an `rgb(...)`/`rgba(...)` function, or a named CSS color; otherwise
/// the whole string is the label.
fn split_box_color(s: &str) -> (Option<String>, String) {
    if let Some(rest) = s.strip_prefix("rgb(").or_else(|| s.strip_prefix("rgba(")) {
        if let Some(close) = rest.find(')') {
            let end = s.len() - rest.len() + close + 1;
            let color = s[..end].to_string();
            let label = s[end..].trim().to_string();
            return (Some(color), label);
        }
    }
    let (first, rest) = match s.split_once(char::is_whitespace) {
        Some((a, b)) => (a, b.trim()),
        None => (s, ""),
    };
    if is_color_token(first) {
        (Some(first.to_string()), rest.to_string())
    } else {
        (None, s.to_string())
    }
}

fn is_color_token(tok: &str) -> bool {
    if tok.starts_with('#') {
        return true;
    }
    const NAMED: &[&str] = &[
        "transparent",
        "aqua",
        "black",
        "blue",
        "cyan",
        "fuchsia",
        "gray",
        "grey",
        "green",
        "lightblue",
        "lightgray",
        "lightgreen",
        "lightgrey",
        "lightyellow",
        "lime",
        "magenta",
        "maroon",
        "navy",
        "olive",
        "orange",
        "pink",
        "purple",
        "red",
        "silver",
        "teal",
        "white",
        "yellow",
    ];
    NAMED.contains(&tok.to_ascii_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::super::parse;
    use crate::parse::ast::{SequenceBox, SequenceDiagram, SequenceItem};

    fn first_box(d: &SequenceDiagram) -> &SequenceBox {
        d.items
            .iter()
            .find_map(|i| match i {
                SequenceItem::Box(b) => Some(b),
                _ => None,
            })
            .expect("no box")
    }

    #[test]
    fn box_captures_members_and_color() {
        let d = parse(
            "sequenceDiagram\nbox Aqua Group\nparticipant A\nactor B\nend\nparticipant C\nA->>C: hi\n",
        )
        .unwrap();
        let b = first_box(&d);
        assert_eq!(b.color.as_deref(), Some("Aqua"));
        assert_eq!(b.label, "Group");
        assert_eq!(b.participant_ids, vec!["A".to_string(), "B".to_string()]);
        // C declared outside the box is not a member.
        assert!(!b.participant_ids.contains(&"C".to_string()));
    }

    #[test]
    fn box_without_color_keeps_full_label() {
        let d = parse("sequenceDiagram\nbox My Team\nparticipant A\nend\n").unwrap();
        let b = first_box(&d);
        assert_eq!(b.color, None);
        assert_eq!(b.label, "My Team");
    }

    #[test]
    fn box_rgb_color() {
        let d =
            parse("sequenceDiagram\nbox rgb(200, 200, 255) Team\nparticipant A\nend\n").unwrap();
        let b = first_box(&d);
        assert_eq!(b.color.as_deref(), Some("rgb(200, 200, 255)"));
        assert_eq!(b.label, "Team");
    }
}
