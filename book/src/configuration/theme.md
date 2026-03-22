# Theme & Appearance

Jay's visual appearance -- colors, fonts, sizes, and layout -- is controlled by
the `[theme]` table and a handful of top-level toggles. Every setting described
here can also be adjusted at runtime from the control center's **Look and Feel**
pane, which provides color pickers and live preview.

## Colors

Colors are specified as hex strings in one of four formats:

`#rgb` (e.g. `#f00`)
: Short RGB

`#rrggbb` (e.g. `#ff0000`)
: Full RGB

`#rgba` (e.g. `#f008`)
: Short RGB + alpha

`#rrggbbaa` (e.g. `#ff000080`)
: Full RGB + alpha

The available color keys in the `[theme]` table are:

`bg-color`
: Desktop background

`bar-bg-color`
: Bar background

`bar-status-text-color`
: Status text in the bar

`border-color`
: Borders between tiled windows

`focused-title-bg-color`
: Background of the focused window's title

`focused-title-text-color`
: Text color of the focused window's title

`unfocused-title-bg-color`
: Background of unfocused window titles

`unfocused-title-text-color`
: Text color of unfocused window titles

`focused-inactive-title-bg-color`
: Background of focused-but-inactive titles

`focused-inactive-title-text-color`
: Text color of focused-but-inactive titles

`attention-requested-bg-color`
: Background of titles that have requested attention

`captured-focused-title-bg-color`
: Background of focused titles that are being recorded

`captured-unfocused-title-bg-color`
: Background of unfocused titles that are being recorded

`separator-color`
: Separator between title bars and window content

`highlight-color`
: Accent color used to highlight parts of the UI

"Focused-inactive" refers to a window that was most recently focused in its
container but whose container is not the active one. The "captured" colors apply
when a window is being recorded (e.g. via screen sharing).

### Example

```toml
[theme]
bg-color = "#1e1e2e"
bar-bg-color = "#181825"
bar-status-text-color = "#cdd6f4"
border-color = "#313244"
focused-title-bg-color = "#89b4fa"
focused-title-text-color = "#1e1e2e"
unfocused-title-bg-color = "#313244"
unfocused-title-text-color = "#cdd6f4"
attention-requested-bg-color = "#f38ba8"
highlight-color = "#f5c2e7"
```

## Sizes

`border-width`
: Width of borders between windows (px)

`title-height`
: Height of window title tabs (px)

`bar-height`
: Height of the bar (px). Defaults to the same as `title-height`.

`bar-separator-width`
: Width of the bar's bottom separator (px). Default: `1`.

```toml
[theme]
border-width = 2
title-height = 24
bar-height = 28
```

## Fonts

`font`
: General font for the compositor

`title-font`
: Font used in window title bars. Defaults to the same as `font`.

`bar-font`
: Font used in the status bar. Defaults to the same as `font`.

```toml
[theme]
font = "JetBrains Mono 10"
title-font = "Inter 10"
bar-font = "Inter 10"
```

## Bar Position

The `bar-position` field controls whether the bar appears at the top or bottom
of each output. The default is `top`.

```toml
[theme]
bar-position = "bottom"
```

## Changing the Theme at Runtime

Use the `set-theme` action in a shortcut to change theme properties on the fly:

```toml
[shortcuts]
alt-F9 = { type = "set-theme", theme.bg-color = "#000000" }
```

Only the fields you include are changed; everything else stays the same.

## Showing and Hiding UI Elements

These top-level settings control whether the bar and title bars are visible:

`show-bar`
: Show the built-in status bar. Default: `true`.

`show-titles`
: Show window title bars. Default: `true`.

```toml
show-bar = false
show-titles = false
```

Corresponding actions let you toggle these at runtime:

`show-bar`
: Shows the bar

`hide-bar`
: Hides the bar

`toggle-bar`
: Toggles bar visibility

`show-titles`
: Shows title bars

`hide-titles`
: Hides title bars

`toggle-titles`
: Toggles title bars

```toml
[shortcuts]
alt-b = "toggle-bar"
alt-t = "toggle-titles"
```

## Workspace Display Order

The `workspace-display-order` top-level setting controls how workspace tabs
appear in the bar:

`manual`
: Workspaces can be reordered by dragging (default)

`sorted`
: Workspaces are displayed in alphabetical order

```toml
workspace-display-order = "sorted"
```
