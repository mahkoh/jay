use serde::Deserialize;
use serde::Serialize;

/// A target description for neighbor searching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct Target {
    /// The kind of neighbor to find.
    pub kind: Kind,
    /// Whether to find the succeeding or preceding neighbor.
    pub backward: bool,
    /// Whether to search horizontally or vertically.
    pub vertical: bool,
    /// Moving-past-edge behavior.
    pub edge_mode: EdgeMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Kind {
    Split,
    Mono,
    Float,
    Workspace,
    Output,
}

/// Describes what to do when attempting to move past the last or first child of a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum EdgeMode {
    /// Do nothing, don't change focus.
    Stop,
    /// Wrap around and focus the first or last child.
    Wrap,
    /// Spill over, focus the closest descendant in new parent.
    Traverse,
    /// Spill over, focus the inactive-focus child of the new parent.
    Inactive,
}
