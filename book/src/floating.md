# Floating Windows

Floating windows are not part of the tiling layout. They hover above tiled
windows and can be freely moved and resized. Any tiled window can be made
floating, and any floating window can be returned to the tiling layout.

## Toggling Between Tiled and Floating

`alt-shift-f` -- `toggle-floating`
: Toggle the focused window between tiled and floating

You can also double-click a window's title bar to toggle between the two states.

For explicit control without toggling:

```toml
[shortcuts]
alt-shift-t = "tile"    # Make the focused window tiled
alt-shift-g = "float"   # Make the focused window floating
```

## Moving Floating Windows

Drag a floating window's title bar to move it. The window follows the cursor
until you release the mouse button.

In [window management mode](#window-management-mode), you can left-drag
anywhere on a floating window to move it -- you do not need to target the
title bar.

## Resizing Floating Windows

Drag a floating window's border to resize it. The cursor changes to a resize
indicator when hovering over the border.

In [window management mode](#window-management-mode), you can right-drag
anywhere on a floating window to resize it.

## Pinning

A pinned floating window stays visible across workspace switches. This is
useful for things like a sticky terminal, a video player, or a chat window that
you want on screen at all times.

`pin-float`
: Pin the focused floating window

`unpin-float`
: Unpin the focused floating window

`toggle-float-pinned`
: Toggle pinning on the focused floating window

Example shortcut configuration:

```toml
[shortcuts]
alt-shift-p = "toggle-float-pinned"
```

You can also right-click a floating window's title bar to pin or unpin it.

### Pin Icon

By default, the pin icon is hidden. Enable it to get a clickable pin indicator
on floating windows:

```toml
[float]
show-pin-icon = true
```

When the pin icon is visible, click it to pin or unpin the window.

## Float Above Fullscreen

By default, floating windows are hidden below fullscreen windows. You can
change this so floating windows render above fullscreen windows:

`enable-float-above-fullscreen`
: Floating windows render above fullscreen windows

`disable-float-above-fullscreen`
: Floating windows are hidden below fullscreen windows

`toggle-float-above-fullscreen`
: Toggle the behavior

Example:

```toml
[shortcuts]
alt-shift-a = "toggle-float-above-fullscreen"
```

This is a global setting -- it affects all floating windows, not just the
focused one.

## Window Rules: Initial Tile State

You can force specific windows to start as floating (or tiled) using the
`initial-tile-state` field in a [window rule](window-rules.md):

```toml
[[windows]]
match.app-id = "pavucontrol"
initial-tile-state = "floating"

[[windows]]
match.app-id = "mpv"
initial-tile-state = "floating"
```

Valid values are `"floating"` and `"tiled"`.

## Window Management Mode

Window management mode makes it easier to move and resize windows without
needing to target specific UI elements like title bars or borders. Set the
`window-management-key` to a keysym -- holding that key activates the mode:

```toml
window-management-key = "Alt_L"
```

While the key is held:

- **Left-drag** anywhere on a floating window to move it.
- **Right-drag** anywhere on a floating window to resize it.

This mode also works with tiled windows and popups. See
[Mouse Interactions](mouse.md#window-management-mode) for the full list of
interactions available in this mode.

> [!NOTE]
> Entering window management mode disables all pointer constraints. This can be
> used to break out of games or other applications that grab the pointer, but
> it also means pointer-dependent applications will behave differently while
> the key is held.
