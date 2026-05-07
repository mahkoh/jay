# Overlays

Overlays are workspaces that render on top of the normal workspace and
layer-shell surfaces. They are useful for scratch pads, floating terminals,
quick-access tools, or any set of windows you want to summon and dismiss
without disturbing your main workspace layout. Other compositors call this
concept special workspaces or scratchpads.

## Defining an Overlay

The `show-overlay` and `toggle-overlay` actions automatically set the
workspace type to `overlay`. A shortcut binding is all you need:

```toml
[shortcuts]
alt-s = { type = "toggle-overlay", name = "scratchpad" }
```

If you want to configure additional workspace properties, you can also
declare the overlay explicitly in the `[workspaces]` table:

```toml
[workspaces."scratchpad"]
type = "overlay"
```

Overlays and normal workspaces share the same namespace. A workspace can only
be one type at a time.

## Showing an Overlay

Use the `show-overlay` action to display an overlay on the current output:

```toml
[shortcuts]
alt-s = { type = "show-overlay", name = "scratchpad" }
```

This creates the overlay if it does not exist and moves it to the output that
currently has focus. If the overlay is already visible on another output, it
moves to the current one.

`show-overlay` is a convenience alias for `show-workspace` with
`move-to-output` set to `true` and `toggle` set to `false`. It accepts the
same optional fields as `show-workspace` (`output`, `focus`,
`fallback-output-mode`).

## Toggling an Overlay

Use the `toggle-overlay` action to show or hide an overlay:

```toml
[shortcuts]
alt-s = { type = "toggle-overlay", name = "scratchpad" }
```

If the overlay is not visible, it is shown on the current output. If it is
already visible, it is hidden.

`toggle-overlay` is a convenience alias for `show-workspace` with
`move-to-output` set to `false` and `toggle` set to `true`.

## Hiding Overlays

There are several ways to hide overlays:

`hide-overlay`
: Hides a specific overlay by name.

`hide-overlays`
: Hides all visible overlays on all outputs.

```toml
[shortcuts]
alt-shift-s = { type = "hide-overlay", name = "scratchpad" }
alt-Escape  = "hide-overlays"
```

You can also middle-click an overlay's tab in the bar to hide it.

When an overlay is hidden, its windows are preserved. Showing the overlay
again restores them. If an overlay is hidden while empty, it is automatically
destroyed. Hidden overlays with windows are not shown in the bar. You can use
the [control center](control-center.md) Workspaces pane to see all existing
workspaces, including hidden overlays.

## Behavior

Overlay workspaces differ from normal workspaces in several ways:

- **Rendering order.** Overlays render above layer-shell surfaces. This means an
  overlay is always on top of panels, status bars, and other layer-shell
  clients.

- **Floating by default.** Windows opened while an overlay is active default
  to floating rather than tiled. You can still tile windows within an overlay
  using the normal tiling actions.

- **Overlay icon.** Windows in an overlay display a diamond-shaped icon in
  their title bar to distinguish them from windows in normal workspaces.

- **One per output.** Each output can display at most one overlay at a time.
  Showing a different overlay on an output replaces the current one.

- **Bar tab.** A visible overlay appears as a tab in the bar, after the
  normal workspace tabs. It is always highlighted with the focused title
  background color.

## Moving Windows from and to Overlays

The `move-to-workspace` action works with overlays just like with normal
workspaces. You can also use `move-left`, `move-right`, etc. to move a window
to an adjacent output's overlay.

You can move floating windows between normal workspaces and overlays by
dragging their title bar. When you drop a floating window on an output, it
lands on the overlay if one is visible, or on the normal workspace otherwise.
To move a floating window out of an overlay, either drag it to an output that
has no overlay visible, or hide the overlay while the mouse button is still
held down.

Tiled windows can also be moved by dragging their title bar. Drop a tile onto
the overlay's bar tab to move it into the overlay, or onto a normal workspace
tab to move it out.

## Hold-to-Show

Using [complex shortcuts](configuration/shortcuts.md#complex-shortcuts), you
can show an overlay while a key is held and hide it on release:

```toml
[complex-shortcuts.alt-s]
action = { type = "show-overlay", name = "scratchpad" }
latch = { type = "hide-overlay", name = "scratchpad" }
```

## Using `show-workspace` Directly

The `show-overlay` and `toggle-overlay` actions are convenience aliases. You
can use `show-workspace` directly with overlay workspaces for full control
over the `toggle` and `move-to-output` fields. Unlike the overlay actions,
`show-workspace` does not automatically set the workspace type, so you must
define the overlay in the `[workspaces]` table:

```toml
[shortcuts]
# Toggle the overlay and always move it to the current output
alt-s = {
    type = "show-workspace",
    name = "scratchpad",
    toggle = true,
    move-to-output = true,
}
```

The `toggle` field has no effect on normal workspaces.
