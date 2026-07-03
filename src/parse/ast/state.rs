//! State-diagram AST types.

use super::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StateDiagram {
    pub direction: FlowDirection,
    pub states: Vec<State>,
    pub transitions: Vec<StateTransition>,
    pub composites: Vec<CompositeState>,
    pub notes: Vec<StateNote>,
    /// `classDef <name> …` definitions, keyed by class name.
    pub class_defs: HashMap<String, Style>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub id: String,
    pub label: String,
    pub kind: StateKind,
    /// Class names applied via `class`/`:::`.
    pub classes: Vec<String>,
    /// Inline `style <id> …` properties.
    pub style: Style,
    /// Interaction bound via a `click` directive, if any (reuses the flowchart
    /// [`ClickAction`] model).
    pub click: Option<ClickAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StateKind {
    Normal,
    Start,
    End,
    Choice,
    Fork,
    Join,
    /// History pseudo-state: `[H]`/`<<history>>` (shallow) or `[H*]` (deep).
    History {
        deep: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositeState {
    pub id: String,
    /// Each region is an ordered list of child state ids (multi-region
    /// composites use the `--` separator to split into parallel regions).
    pub regions: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateNote {
    pub target: String,
    pub position: NotePosition,
    pub text: String,
}
