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

/// Configures what happens to empty workspaces when they are left or become inactive.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum WorkspaceEmptyBehavior {
    /// Never destroy or hide empty workspaces automatically.
    Preserve,
    /// Destroy an empty workspace when switching away from it.
    DestroyOnLeave,
    /// Hide an empty workspace when switching away from it.
    HideOnLeave,
    /// Destroy an empty workspace whenever it is empty and inactive.
    Destroy,
    /// Hide an empty workspace whenever it is empty and inactive.
    Hide,
}

/// Sets what should happen to empty workspaces.
///
/// The default is `WorkspaceEmptyBehavior::DestroyOnLeave`.
pub fn set_workspace_empty_behavior(behavior: WorkspaceEmptyBehavior) {
    get!().set_workspace_empty_behavior(behavior);
}
