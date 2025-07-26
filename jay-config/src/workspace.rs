//! Tools for configuring workspaces.

use serde::{Deserialize, Serialize};

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
