# Shortcuts

Shortcuts bind key combinations to actions. They are the primary way to
interact with Jay.

## Basic syntax

Shortcuts are defined in the `[shortcuts]` table. The left side is a key
combination; the right side is an action:

```toml
[shortcuts]
alt-q = "quit"
alt-Return = { type = "exec", exec = "alacritty" }
alt-shift-c = "close"
```

### Key format

Key combinations follow the pattern `MODIFIER-MODIFIER-KEYSYM`:

```
(MOD-)*KEYSYM
```

**Keysym names** are unmodified XKB keysym names from [xkbcommon-keysyms.h](https://github.com/xkbcommon/libxkbcommon/blob/master/include/xkbcommon/xkbcommon-keysyms.h)
with the `XKB_KEY_` prefix removed. Use the unmodified keysym -- write
`shift-q`, not `shift-Q`.

### Modifiers

The available modifiers are:

`shift`
: Shift key

`ctrl`
: Control key

`alt`
: Alt key

`logo`
: Super/Meta/Windows key

`lock`
: Lock modifier

`caps`
: Caps Lock

`num`
: Num Lock

`mod1`
: Mod1 (typically Alt)

`mod2`
: Mod2 (typically Num Lock)

`mod3`
: Mod3

`mod4`
: Mod4 (typically Super)

`mod5`
: Mod5

`release`
: Fire on key **release** instead of press

The `release` modifier is special: it causes the action to trigger when the key
is released rather than when it is pressed.

## Simple actions

Simple actions are written as plain strings. Here are the most commonly used
ones:

**Focus and movement:**

```toml
[shortcuts]
alt-h = "focus-left"
alt-j = "focus-down"
alt-k = "focus-up"
alt-l = "focus-right"

alt-shift-h = "move-left"
alt-shift-j = "move-down"
alt-shift-k = "move-up"
alt-shift-l = "move-right"
```

**Layout:**

```toml
[shortcuts]
alt-d = "split-horizontal"
alt-v = "split-vertical"
alt-t = "toggle-split"
alt-m = "toggle-mono"
alt-f = "focus-parent"
```

**Window management:**

```toml
[shortcuts]
alt-u = "toggle-fullscreen"
alt-shift-f = "toggle-floating"
alt-shift-c = "close"
```

**Compositor control:**

```toml
[shortcuts]
alt-q = "quit"
alt-shift-r = "reload-config-toml"
```

**Other useful simple actions:**

- `consume` -- consume the key event (prevent it from reaching applications).
  Key-press events that trigger shortcuts are consumed by default; key-release
  events are forwarded by default. Consuming key-release events can cause keys
  to get stuck in the focused application.
- `forward` -- forward the key event to the focused application (the inverse
  of `consume`)
- `none` -- unbind this key combination (useful for overriding defaults or
  inherited mode bindings)
- `disable-pointer-constraint` -- release a pointer lock/confinement
- `focus-parent` -- move focus to the parent container
- `toggle-bar`, `show-bar`, `hide-bar` -- control the status bar
- `open-control-center` -- open the Jay control center GUI
- `warp-mouse-to-focus` -- warp the cursor to the center of the focused window
- `kill-client` -- forcefully disconnect a client (in a window rule, kills the
  window's client; in a client rule, kills the matched client; has no effect
  in plain shortcuts)
- `focus-below`, `focus-above` -- move focus to the layer below or above the
  current layer
- `focus-tiles` -- focus the tile layer
- `create-mark`, `jump-to-mark` -- interactively create or jump to a mark
  (the next pressed key identifies the mark). See [Marks](#marks) below.
- `enable-window-management`, `disable-window-management` -- programmatically
  enable or disable [window management mode](../floating.md#window-management-mode)
- `reload-config-so` -- reload the shared-library configuration (`config.so`)

See the [specification](https://github.com/mahkoh/jay/blob/master/toml-spec/spec/spec.generated.md) for the full list of simple
actions.

## Parameterized actions

Actions that need arguments are written as tables with a `type` field:

### Launching programs

```toml
[shortcuts]
alt-Return = { type = "exec", exec = "alacritty" }
alt-p = { type = "exec", exec = "bemenu-run" }
```

### Switching virtual terminals

```toml
[shortcuts]
ctrl-alt-F1 = { type = "switch-to-vt", num = 1 }
ctrl-alt-F2 = { type = "switch-to-vt", num = 2 }
```

### Workspaces

```toml
[shortcuts]
alt-F1 = { type = "show-workspace", name = "1" }
alt-F2 = { type = "show-workspace", name = "2" }

alt-shift-F1 = { type = "move-to-workspace", name = "1" }
alt-shift-F2 = { type = "move-to-workspace", name = "2" }
```

### Moving workspaces to outputs

```toml
[shortcuts]
logo-ctrl-shift-Right = {
    type = "move-to-output",
    direction = "right",
}
logo-ctrl-shift-Left = {
    type = "move-to-output",
    direction = "left",
}
```

### Resizing windows

The `resize` action resizes the focused window by adjusting its edges.
Each of the four optional fields (`dx1`, `dy1`, `dx2`, `dy2`) specifies a
pixel offset for one edge of the window. Fields default to `0` when omitted.

`dx1`
: Change at the left edge (negative = grow left, positive = shrink left)

`dy1`
: Change at the top edge (negative = grow up, positive = shrink down)

`dx2`
: Change at the right edge (positive = grow right, negative = shrink right)

`dy2`
: Change at the bottom edge (positive = grow down, negative = shrink down)

Growing the window by 10 pixels on the right:

```toml
[shortcuts]
alt-Right = { type = "resize", dx2 = 10 }
```

Shrinking the window by 10 pixels on the right:

```toml
[shortcuts]
alt-Left = { type = "resize", dx2 = -10 }
```

Moving the window 10 pixels to the right without changing its size (both
left and right edges shift by the same amount):

```toml
[shortcuts]
alt-shift-Right = { type = "resize", dx1 = 10, dx2 = 10 }
```

### Other parameterized actions

- `set-keymap` -- change the active keymap
- `set-repeat-rate` -- change the keyboard repeat rate
- `set-env` -- set environment variables for future spawned programs
- `unset-env` -- remove environment variables
- `configure-connector` -- enable/disable a monitor
- `configure-input` -- change input device settings
- `configure-output` -- change output settings
- `configure-idle` -- change the idle timeout
- `configure-direct-scanout` -- enable or disable direct scanout
- `configure-drm-device` -- apply settings to a DRM device
- `set-theme` -- change theme settings
- `set-log-level` -- change the compositor log level
- `set-gfx-api` -- set the graphics API for new DRM devices (usually only
  effective at startup)
- `set-render-device` -- set the render device for compositing
- `define-action` -- define or redefine a named action at runtime
- `undefine-action` -- remove a named action
- `create-mark` -- create a mark with an explicit ID (see [Marks](#marks))
- `jump-to-mark` -- jump to a mark with an explicit ID
- `copy-mark` -- copy a mark from one ID to another
- `create-virtual-output` -- create a virtual output
- `remove-virtual-output` -- remove a virtual output

See the [specification](https://github.com/mahkoh/jay/blob/master/toml-spec/spec/spec.generated.md) for the complete list.

## Running multiple actions

Use an array to run several actions from a single shortcut:

```toml
[shortcuts]
alt-q = [
    { type = "exec", exec = ["notify-send", "Goodbye!"] },
    "quit",
]
```

## The exec action in detail

The `exec` field accepts three forms:

**A simple string** -- the program name with no arguments:

```toml
alt-Return = { type = "exec", exec = "alacritty" }
```

**An array of strings** -- the program name followed by arguments:

```toml
alt-n = { type = "exec", exec = ["notify-send", "Hello", "World"] }
```

**A table** -- full control over execution. Exactly one of `prog` or `shell`
must be specified:

```toml
# Using prog + args
alt-n = {
    type = "exec",
    exec = {
        prog = "notify-send",
        args = ["Hello"],
        env = { LANG = "en_US.UTF-8" },
    },
}

# Using shell (runs as: $SHELL -c "command")
alt-s = {
    type = "exec",
    exec = {
        shell = "grim - | wl-copy",
        privileged = true,
    },
}
```

Table fields:

`prog`
: Program to execute (mutually exclusive with `shell`)

`shell`
: Shell command to run via `$SHELL -c` (mutually exclusive with `prog`)

`args`
: Arguments array (only with `prog`)

`env`
: Per-process environment variables

`privileged`
: If `true`, grants access to privileged Wayland protocols (default: `false`)

`tag`
: Tag to apply to all Wayland connections spawned by this process

### Practical examples

Volume control with `pactl`:

```toml
[shortcuts]
XF86AudioRaiseVolume = {
    type = "exec",
    exec = ["pactl", "set-sink-volume", "0", "+5%"],
}
XF86AudioLowerVolume = {
    type = "exec",
    exec = ["pactl", "set-sink-volume", "0", "-5%"],
}
XF86AudioMute = {
    type = "exec",
    exec = ["pactl", "set-sink-mute", "0", "toggle"],
}
```

Taking a screenshot and copying to clipboard:

```toml
[shortcuts]
Print = {
    type = "exec",
    exec = {
        shell = "grim - | wl-copy",
        privileged = true,
    },
}
```

## Complex shortcuts

Complex shortcuts provide additional control via the `[complex-shortcuts]`
table. They support:

- **`mod-mask`** -- controls which modifiers are considered when matching.
  Set to `""` to ignore all modifiers.
- **`action`** -- the action to run on key press (defaults to `"none"`).
- **`latch`** -- an action to run when the key is **released**.

### Volume keys regardless of modifiers

The volume keys should work whether or not Alt, Shift, etc. are held:

```toml
[complex-shortcuts.XF86AudioRaiseVolume]
mod-mask = ""
action = {
    type = "exec",
    exec = ["pactl", "set-sink-volume", "0", "+5%"],
}

[complex-shortcuts.XF86AudioLowerVolume]
mod-mask = ""
action = {
    type = "exec",
    exec = ["pactl", "set-sink-volume", "0", "-5%"],
}
```

### Push-to-talk

Unmute audio while a key is held, mute on release:

```toml
[complex-shortcuts.alt-x]
action = {
    type = "exec",
    exec = ["pactl", "set-sink-mute", "0", "0"],
}
latch = {
    type = "exec",
    exec = ["pactl", "set-sink-mute", "0", "1"],
}
```

The `latch` action fires when the triggering key (`x` in this case) is
released, regardless of any other keys pressed at that time.

## Marks

Marks let you tag a window and quickly jump back to it later, similar to marks
in Vim.

### Interactive marks

The simplest way to use marks is interactively. Bind `create-mark` and
`jump-to-mark` as simple string actions:

```toml
[shortcuts]
alt-m = "create-mark"
alt-apostrophe = "jump-to-mark"
```

When you press `alt-m`, Jay waits for the next key press (e.g. `a`) and
assigns the mark to the currently focused window. When you press
`alt-apostrophe` followed by `a`, Jay focuses the marked window.

### Hard-coded marks

You can skip the interactive step by specifying a mark ID directly:

```toml
[shortcuts]
alt-shift-1 = { type = "create-mark", id.key = "1" }
alt-1       = { type = "jump-to-mark", id.key = "1" }
```

Mark IDs can be identified by a key name (`id.key`) or by an arbitrary string
(`id.name`):

```toml
[shortcuts]
alt-shift-b = { type = "create-mark", id.name = "browser" }
alt-b       = { type = "jump-to-mark", id.name = "browser" }
```

Key names use Linux input event code names with the `KEY_` prefix removed, all
lowercase (see the [Linux input event codes](https://github.com/torvalds/linux/blob/master/include/uapi/linux/input-event-codes.h)).

### Copying marks

The `copy-mark` action copies a mark from one ID to another:

```toml
[shortcuts]
alt-c = { type = "copy-mark", src.key = "a", dst.name = "backup" }
```

## Named actions

Named actions provide another layer of reuse. Define them in the `[actions]`
table and reference them with `$name`:

```toml
[actions]
my-layout = [
    "split-horizontal",
    { type = "exec", exec = "alacritty" },
]

[shortcuts]
alt-l = "$my-layout"
```

You can redefine named actions at runtime using the `define-action` and
`undefine-action` parameterized actions:

```toml
[shortcuts]
alt-shift-q = {
    type = "define-action",
    name = "my-layout",
    action = "quit",
}
```

## Virtual outputs

Virtual outputs can be created and removed via actions. A virtual output has
the connector name `VO-{name}` and the serial number `{name}`. A newly created
virtual output is initially disabled.

```toml
[shortcuts]
alt-shift-v = {
    type = "create-virtual-output",
    name = "screen-share",
}
alt-shift-x = {
    type = "remove-virtual-output",
    name = "screen-share",
}
```

You can pre-configure the virtual output using connector and output match
rules:

```toml
[[connectors]]
match.name = "VO-screen-share"
enabled = true

[[outputs]]
match.connector = "VO-screen-share"
mode = {
    width = 1920,
    height = 1080,
    refresh-rate = 120.0,
}
```

## Actions in window rules

When certain simple actions are used inside a [window rule](../window-rules.md),
they apply to the **matched window** instead of the focused window. The
affected actions are: `move-left`, `move-down`, `move-up`, `move-right`,
`split-horizontal`, `split-vertical`, `toggle-split`, `tile-horizontal`,
`tile-vertical`, `show-single`, `show-all`, `toggle-fullscreen`,
`enter-fullscreen`, `exit-fullscreen`, `close`, `toggle-floating`, `float`,
`tile`, `toggle-float-pinned`, `pin-float`, `unpin-float`, and `resize`.

Similarly, `kill-client` applies to the matched window's client in a window
rule, or to the matched client in a client rule.
