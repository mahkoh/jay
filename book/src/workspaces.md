# Workspaces

Workspaces are virtual desktops that group windows together. Each workspace
lives on an output (monitor) and contains its own tiling layout. Jay creates
workspaces on demand and automatically manages them when monitors are connected
or disconnected.

## Switching Workspaces

Use the `show-workspace` action to switch to a workspace. In the default
configuration, `alt-F1` through `alt-F12` switch to workspaces named "1"
through "12":

```toml
[shortcuts]
alt-F1  = { type = "show-workspace", name = "1" }
alt-F2  = { type = "show-workspace", name = "2" }
alt-F3  = { type = "show-workspace", name = "3" }
# ... and so on through alt-F12
```

If the workspace does not yet exist, it is created on the output that currently
contains the cursor. You can override this by specifying an output:

```toml
[shortcuts]
alt-F1 = {
    type = "show-workspace",
    name = "1",
    output.name = "left",
}
```

You can also scroll over the bar to cycle through workspaces on that output.

## Moving Windows to Workspaces

Use the `move-to-workspace` action to send the focused window to a different
workspace. The default bindings are `alt-shift-F1` through `alt-shift-F12`:

```toml
[shortcuts]
alt-shift-F1  = { type = "move-to-workspace", name = "1" }
alt-shift-F2  = { type = "move-to-workspace", name = "2" }
alt-shift-F3  = { type = "move-to-workspace", name = "3" }
# ... and so on through alt-shift-F12
```

You can also drag a tile's title onto a workspace tab in the bar to move it to
that workspace. Dragging a tile onto the bar outside any workspace tab creates a
new workspace for it.

## Moving Workspaces Between Outputs

The `move-to-output` action moves a workspace to a different output. You can
target the output by name or by direction:

```toml
[shortcuts]
# Move the current workspace to a named output
alt-o = { type = "move-to-output", output.name = "right" }

# Move the current workspace in a direction
logo-ctrl-shift-Right = {
    type = "move-to-output",
    direction = "right",
}
logo-ctrl-shift-Left = {
    type = "move-to-output",
    direction = "left",
}
logo-ctrl-shift-Up = {
    type = "move-to-output",
    direction = "up",
}
logo-ctrl-shift-Down = {
    type = "move-to-output",
    direction = "down",
}
```

You can also move a specific workspace by name:

```toml
[shortcuts]
alt-o = {
    type = "move-to-output",
    workspace = "1",
    output.name = "right",
}
```

If `workspace` is omitted, the currently active workspace is moved.

## Workspace Display Order

Workspaces appear as tabs in the bar. Their order can be configured in two
modes:

- **manual** (default) -- workspaces appear in the order they were created and
  can be reordered by dragging their titles in the bar.
- **sorted** -- workspaces are sorted alphabetically. Dragging to reorder is
  disabled.

Set the order in your configuration:

```toml
workspace-display-order = "sorted"
```

You can also change this at runtime in the control center.

## Hot-Plug and Hot-Unplug

Jay handles monitor connections gracefully:

- When a monitor is **unplugged**, its workspaces are automatically migrated to
  one of the remaining monitors.
- When the monitor is **plugged back in**, those workspaces are restored to it.

This means you never lose your workspace layout when docking or undocking a
laptop.

## Workspace Capture

By default, newly created workspaces can be captured for screen sharing. You
can disable this globally:

```toml
workspace-capture = false
```

When workspace capture is enabled, screen-sharing applications can share
individual workspaces (in addition to full outputs and individual windows). See
[Screen Sharing](screen-sharing.md) for more details.

## Matching Windows by Workspace

In [window rules](window-rules.md), you can match windows based on the
workspace they are on:

```toml
[[windows]]
match.workspace = "music"
action = "enter-fullscreen"
```

The `workspace-regex` field is also available for pattern matching:

```toml
[[windows]]
match.workspace-regex = "^(music|video)$"
action = "enter-fullscreen"
```

Since window rules are reactive, these rules are re-evaluated whenever a window
moves to a different workspace.
