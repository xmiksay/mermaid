//! v11 named-shape → `NodeShape` mapping table for flowchart nodes.

use crate::parse::ast::NodeShape;

/// Map a v11 named shape onto an existing `NodeShape`. Aliases follow upstream
/// Mermaid; visual-only shapes still without a variant (e.g. `sm-circ`, `fork`,
/// `text`) fall back to `Rect` so their content is still rendered. Unknown names
/// likewise fall back to `Rect`.
pub(super) fn shape_from_name(name: &str) -> NodeShape {
    match name.trim() {
        "rounded" | "event" => NodeShape::Round,
        "stadium" | "pill" | "term" | "terminal" => NodeShape::Stadium,
        "subproc" | "subprocess" | "subroutine" | "fr-rect" | "framed-rectangle" => {
            NodeShape::Subroutine
        }
        "cyl" | "cylinder" | "database" | "db" => NodeShape::Cylinder,
        "circle" | "circ" => NodeShape::Circle,
        "dbl-circ" | "double-circle" => NodeShape::DoubleCircle,
        "diam" | "diamond" | "decision" | "question" => NodeShape::Rhombus,
        "hex" | "hexagon" | "prepare" => NodeShape::Hexagon,
        "lean-r" | "lean-right" | "in-out" => NodeShape::Parallelogram,
        "lean-l" | "lean-left" | "out-in" => NodeShape::ParallelogramAlt,
        "trap-b" | "trapezoid-bottom" | "trapezoid" | "priority" => NodeShape::Trapezoid,
        "trap-t" | "trapezoid-top" | "inv-trapezoid" | "manual" => NodeShape::TrapezoidAlt,
        "odd" => NodeShape::Asymmetric,
        "notch-rect" | "card" | "notched-rectangle" => NodeShape::NotchedRect,
        "doc" | "document" => NodeShape::Document,
        "docs" | "documents" | "st-doc" | "stacked-document" => NodeShape::MultiDocument,
        "tag-doc" | "tagged-document" => NodeShape::TaggedDocument,
        "bolt" | "com-link" | "lightning-bolt" => NodeShape::LightningBolt,
        "hourglass" | "collate" => NodeShape::Hourglass,
        "brace" | "brace-l" | "brace-r" | "braces" | "comment" => NodeShape::Comment,
        "delay" | "half-rounded-rectangle" => NodeShape::Delay,
        "das" | "h-cyl" | "horizontal-cylinder" => NodeShape::DirectAccessStorage,
        "lin-cyl" | "disk" | "lined-cylinder" => NodeShape::LinedCylinder,
        "lin-rect" | "lin-proc" | "lined-process" | "lined-rectangle" | "shaded-process" => {
            NodeShape::LinedProcess
        }
        "div-rect" | "div-proc" | "divided-rectangle" | "divided-process" => {
            NodeShape::DividedProcess
        }
        "win-pane" | "window-pane" | "internal-storage" => NodeShape::WindowPane,
        "tri" | "triangle" | "extract" => NodeShape::Triangle,
        "flip-tri" | "flipped-triangle" | "manual-file" => NodeShape::FlippedTriangle,
        "f-circ" | "filled-circle" | "junction" => NodeShape::FilledCircle,
        "cross-circ" | "crossed-circle" | "summary" => NodeShape::CrossedCircle,
        "flag" | "paper-tape" => NodeShape::PaperTape,
        "bow-rect" | "bow-tie-rectangle" | "stored-data" => NodeShape::StoredData,
        _ => NodeShape::Rect,
    }
}
