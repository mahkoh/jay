//! Tools for configuring workspaces.

use serde::Deserialize;
use serde::Serialize;

/// How workspaces should be ordered in the UI.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum WorkspaceDisplayOrder {
    /// Workspaces are not sorted and can be manually dragged.
    Manual,
    /// Workspaces are sorted alphabetically and cannot be manually dragged.
    Sorted,
}

/// Sets how workspaces should be ordered in the UI.
///
/// The default is `WorkspaceDisplayOrder::Manual`.
pub fn set_workspace_display_order(order: WorkspaceDisplayOrder) {
    get!().set_workspace_display_order(order);
}

/// The layout that the root container of a workspace is initially created with.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum WorkspaceLayout {
    /// The container is created in mono mode.
    Mono,
    /// The container is created in tiled mode.
    Tile {
        /// The direction in which the container is split.
        direction: TileDirection,
    },
}

/// The direction in which a tiled container is split.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum TileDirection {
    /// The children are placed next to each other. This is the default.
    Horizontal,
    /// The children are placed on top of each other.
    Vertical,
    /// The children are placed along the larger dimension of the workspace.
    ///
    /// If the workspace is exactly as wide as it is high, this is the same as
    /// `Horizontal`.
    Major,
    /// The children are placed along the smaller dimension of the workspace.
    ///
    /// If the workspace is exactly as wide as it is high, this is the same as
    /// `Horizontal`.
    Minor,
}
