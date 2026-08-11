# Tiling

Jay uses an i3-like tiling layout. Windows are arranged automatically in
containers that can be split horizontally or vertically. Containers can be nested
to create complex layouts.

## Splitting Containers

When you split a window, Jay wraps it in a new container with the specified
direction. Subsequent windows opened in that container are placed side by side
(horizontal split) or stacked top to bottom (vertical split).

`alt-d` -- `split-horizontal`
: Split the focused window horizontally

`alt-v` -- `split-vertical`
: Split the focused window vertically

`alt-t` -- `toggle-split`
: Toggle the container's split direction

You can also set the direction explicitly without toggling:

```toml
[shortcuts]
alt-shift-d = "tile-horizontal"
alt-shift-v = "tile-vertical"
```

The `tile-horizontal` action sets the container to horizontal, and
`tile-vertical` sets it to vertical -- unlike `split-horizontal`/`split-vertical`
which wrap the window in a new container first.

`split-major` and `split-minor` derive the direction from the shape of the
window instead: `split-major` splits along its longer side, `split-minor`
along its shorter side. `tile-major` and `tile-minor` do the same for the
container's shape. A square window counts as wide.

```toml
[shortcuts]
alt-Return = ["split-major", { type = "exec", exec = "alacritty" }]
```

New terminals then appear next to a wide window and below a tall one, keeping
tiles roughly square.

If the window being split is the only window in its container, the
[`split-reuses-container`](configuration/misc.md#split-reuses-container)
option makes the split change that container's direction instead of wrapping the
window in a nested container.

## Moving Focus

Move keyboard focus between windows with the directional focus actions:

`alt-h` -- `focus-left`
: Move focus left

`alt-j` -- `focus-down`
: Move focus down

`alt-k` -- `focus-up`
: Move focus up

`alt-l` -- `focus-right`
: Move focus right

Focus crosses container boundaries, so you can navigate across your entire
layout with these four keys.

## Moving Windows

Move the focused window within or between containers:

`alt-shift-h` -- `move-left`
: Move window left

`alt-shift-j` -- `move-down`
: Move window down

`alt-shift-k` -- `move-up`
: Move window up

`alt-shift-l` -- `move-right`
: Move window right

When a window reaches the edge of its container, the move action pushes it into
the adjacent container.

## Focus Parent

Press `alt-f` (`focus-parent`) to move focus from a window to its parent
container. This is useful when you want to operate on an entire group of
windows at once. For example, focusing a parent container and then using
`move-left` moves the whole group rather than a single window.

## Mono Mode

By default, a container shows all its children side by side. Mono mode changes
this so only one child is visible at a time, similar to a tabbed view.

`alt-m` -- `toggle-mono`
: Toggle between mono and side-by-side

You can also right-click any title in a container to toggle mono mode.

In mono mode, scroll over the title bar to cycle between windows in the
container.

For explicit control without toggling:

```toml
[shortcuts]
alt-s = "show-single"   # Enter mono mode
alt-a = "show-all"      # Exit mono mode
```

## Target Container

The [container actions](configuration/shortcuts.md#container-actions) change
the parent container of the window they apply to. Their table form takes a
`target` field to change that -- `"self"` for the window itself when it is a
container, `"auto"` for whichever of the two is a container.

## Fullscreen

Press `alt-u` (`toggle-fullscreen`) to make the focused window fill the entire
output, hiding the bar and other windows. Press it again to return to the tiled
layout.

For explicit control:

```toml
[shortcuts]
alt-shift-u = "enter-fullscreen"
alt-ctrl-u  = "exit-fullscreen"
```

## Resizing Tiles

Drag the separators between tiles with the mouse to resize them. The separator
changes the cursor to a resize indicator when hovered.

In [window management mode](mouse.md#window-management-mode), you can also
right-drag anywhere on a tile to resize it without needing to target the
separator.

## Closing Windows

Press `alt-shift-c` (`close`) to request the focused window to close. This
sends a polite close request to the application -- it is not a forceful kill.

```toml
[shortcuts]
alt-shift-c = "close"
```

## Toggling Floating

Double-click a tile's title bar to toggle it between tiled and floating. See
[Floating Windows](floating.md) for more details.

## Summary of Tiling Actions

`split-horizontal`
: Wrap focused window in a horizontal container (or, with
  [`split-reuses-container`](configuration/misc.md#split-reuses-container),
  make its container horizontal if it is the only window in it)

`split-vertical`
: Wrap focused window in a vertical container (or, with
  [`split-reuses-container`](configuration/misc.md#split-reuses-container),
  make its container vertical if it is the only window in it)

`split-major` / `split-minor`
: Wrap focused window in a container split along its longer / shorter side

`toggle-split`
: Toggle container split direction

`tile-horizontal`
: Set container direction to horizontal

`tile-vertical`
: Set container direction to vertical

`tile-major` / `tile-minor`
: Set container direction along its longer / shorter side

`focus-left/right/up/down`
: Move keyboard focus

`move-left/right/up/down`
: Move focused window

`focus-parent`
: Focus the parent container

`toggle-mono`
: Toggle mono mode

`show-single`
: Enter mono mode

`show-all`
: Exit mono mode

`toggle-fullscreen`
: Toggle fullscreen

`enter-fullscreen`
: Enter fullscreen

`exit-fullscreen`
: Exit fullscreen

`close`
: Request focused window to close

`toggle-floating`
: Toggle between tiled and floating
