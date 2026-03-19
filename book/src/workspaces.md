# Workspaces

Workspaces are virtual desktops that group windows together. Each workspace
lives on an output (monitor) and contains its own tiling layout. Jay creates
workspaces on demand and automatically manages them when monitors are connected
or disconnected.

Jay also supports [overlay](overlays.md) workspaces that render above normal
workspaces and can be toggled on and off.

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

The `show-workspace` action supports several optional fields:

`output`
: The output to create a new workspace on. Has no effect on workspaces that
  already exist unless `move-to-output` is also set to `true`. If multiple
  outputs match, the first match is used.

`move-to-output`
: Whether to move the workspace to the target output if it already exists on a
  different output. Default: `false`.

`focus`
: Whether the workspace grabs the keyboard focus. Default: `true`.

`fallback-output-mode`
: Which output to use when no explicit `output` is specified. Either `"cursor"`
  or `"focus"`. Default: the global
  [fallback-output-mode](configuration/misc.md#fallback-output-mode) setting.

`toggle`
: Whether to hide the workspace if it is already visible. This only has an
  effect for [overlay](overlays.md) workspaces. Default: `false`.

For example, to always show a workspace on a specific output, moving it there
if necessary:

```toml
[shortcuts]
alt-F1 = {
    type = "show-workspace",
    name = "1",
    output.name = "left",
    move-to-output = true,
}
```

To switch to a workspace without changing focus:

```toml
[shortcuts]
alt-F1 = {
    type = "show-workspace",
    name = "1",
    focus = false,
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
- **sorted** -- workspaces are sorted using natural ordering. Dragging to
  reorder is disabled.

Set the order in your configuration:

```toml
workspace-display-order = "sorted"
```

You can also change this at runtime in the control center.

## Empty Workspace Behavior

Jay creates workspaces on demand. When a workspace becomes empty, Jay can
optionally hide or destroy it automatically so your workspace list does not
accumulate unused entries.

Configure this with the `workspace-empty-behavior` top-level setting, with an
`empty-behavior` override in a workspace definition, or at runtime in the
control center, in the Compositor pane:

```toml
workspace-empty-behavior = "hide-on-leave"

[workspaces."scratchpad"]
empty-behavior = "preserve"
```

> [!NOTE]
> This behavior is evaluated per output.
>
> - "leave" means the workspace stops being the active workspace on its output
>   because you showed another workspace on that same output.
> - "inactive" means the workspace is currently not the active workspace on its
>   output.

Supported values:

`preserve`
: Never destroy or hide empty workspaces automatically.

`destroy-on-leave`
: Destroy an empty workspace when you leave it (default).

`hide-on-leave`
: Hide an empty workspace when you leave it.

`destroy`
: Destroy an empty workspace whenever it is empty and inactive.

`hide`
: Hide an empty workspace whenever it is empty and inactive.

> [!TIP]
> Hidden workspaces are omitted from Jay's built-in workspace lists and the bar.
> Some external workspace tools may still show them as hidden. You can restore
> them by showing the workspace by name (for example via the `show-workspace`
> action). When restoring a hidden workspace, Jay prefers the output it was last
> shown on if that output is still connected.

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
