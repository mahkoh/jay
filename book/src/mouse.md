# Mouse Interactions

Jay supports a range of mouse-driven interactions for managing windows,
workspaces, and layout. This page is a comprehensive reference for all of them.

## Tiling

**Resizing tiles.** Drag the separator between two tiles to resize them. The
cursor changes to a resize indicator when hovering over a separator.

**Moving tiles.** Drag a tile's title bar to move it within or between
containers. While dragging:

- Drop it onto another position in a container to rearrange tiles.
- Drop it onto a workspace tab in the bar to move it to that workspace.
- Drop it onto the bar outside any workspace tab to create a new workspace
  for it.

**Double-click a tile's title** to toggle it between tiled and floating. See
[Floating Windows](floating.md) for details.

**Right-click any title** in a container to toggle mono mode (showing one window
at a time vs. all side by side). See [Tiling -- Mono Mode](tiling.md#mono-mode).

**Scroll over a title** in mono mode to cycle between the tiles in that
container.

## Floating Windows

**Drag a floating window's title** to move it.

**Drag a floating window's border** to resize it.

**Double-click a floating window's title** to toggle it back to tiled.

**Right-click a floating window's title** to pin or unpin it. Pinned floating
windows stay visible across workspace switches.

**Click the pin icon** (if visible) to pin or unpin the window. The pin icon
can be enabled with:

```toml
[float]
show-pin-icon = true
```

## Workspaces

**Scroll over the bar** to switch between workspaces on that output.

**Drag workspace titles** in the bar to reorder them. This only works in manual
display order mode (the default). See
[Workspaces -- Display Order](workspaces.md#workspace-display-order).

## Window Management Mode

Window management mode enables additional mouse interactions that do not require
targeting specific UI elements. Configure it by setting `window-management-key`
to a keysym:

```toml
window-management-key = "Alt_L"
```

**Hold the configured key** to enter window management mode. While held:

- **Left-drag** anywhere on a floating window, tile, popup, or fullscreen window
  to **move** it.
- **Right-drag** anywhere on a floating window, tile, or popup to **resize** it.

This is especially useful for:

- Moving or resizing floating windows without needing to precisely target the
  title bar or border.
- Moving tiled windows without targeting the title bar.
- Moving fullscreen windows (not possible without this mode).
- Resizing tiles without targeting the separator.

> [!NOTE]
> Entering window management mode disables all pointer constraints. This means
> you can use it to move the pointer out of applications that have grabbed it
> (such as games in fullscreen), but pointer-dependent applications will behave
> differently while the key is held.

## Other

**Toplevel selection.** Some actions (like screen sharing) ask you to select a
window, indicated by a purple overlay. During this selection, right-click a
tile's title to select the entire container instead of an individual tile.

**Canceling interactions.** Press `Escape` to cancel any in-progress mouse
interaction (dragging, resizing, selection, etc.). The cancel key can be changed
with the `pointer-revert-key` configuration:

```toml
# Use a different key to cancel mouse interactions
pointer-revert-key = "grave"

# Disable the cancel key entirely
pointer-revert-key = "NoSymbol"
```

The default is `Escape`.
