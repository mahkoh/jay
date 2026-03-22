# Input Modes

Jay supports an **input mode stack** that enables vim-style modal keybindings.
When a mode is active, its shortcuts take effect instead of (or in addition to)
the top-level shortcuts. Multiple modes can be stacked, and the topmost mode
always wins.

## Defining Modes

Modes are defined in the `[modes]` table. Each mode is a named sub-table that
can contain its own `shortcuts` and `complex-shortcuts`:

```toml
[modes."move".shortcuts]
h = "move-left"
j = "move-down"
k = "move-up"
l = "move-right"
Escape = "pop-mode"
```

## Inheritance

By default, a mode inherits all shortcuts from the top-level `[shortcuts]`
table. Any key not explicitly defined in the mode falls through to the
inherited shortcuts.

To inherit from a different mode instead, set the `parent` field:

```toml
[modes."navigation"]
parent = "base"

[modes."navigation".shortcuts]
w = "focus-up"
a = "focus-left"
s = "focus-down"
d = "focus-right"
```

Here, `navigation` inherits from the mode called `base` rather than from the
top-level shortcuts.

### Unbinding Inherited Shortcuts

Use `"none"` as the action value to unbind a shortcut that would otherwise be
inherited:

```toml
[modes."locked".shortcuts]
# Disable alt-q (quit) while in this mode
alt-q = "none"
```

## Activating and Deactivating Modes

Jay provides four actions for managing the mode stack:

`push-mode`
: Push a named mode onto the stack. It stays active until explicitly removed.

`latch-mode`
: Temporarily push a mode. It auto-pops after the next shortcut fires.

`pop-mode`
: Remove the topmost mode from the stack.

`clear-modes`
: Clear the entire mode stack, returning to top-level shortcuts.

### push-mode

```toml
[shortcuts]
alt-r = { type = "push-mode", name = "resize" }
```

After pressing `alt-r`, the `resize` mode's shortcuts take effect. It remains
active until something pops it.

### latch-mode

```toml
[shortcuts]
alt-x = { type = "latch-mode", name = "navigation" }
```

After pressing `alt-x`, the `navigation` mode is pushed. The very next shortcut
that fires will use the mode's bindings, and then the mode is automatically
popped. This is ideal for **leader key** patterns.

### pop-mode

```toml
[modes."resize".shortcuts]
Escape = "pop-mode"
```

Pressing `Escape` while in `resize` mode removes it from the stack.

### clear-modes

```toml
[modes."resize".shortcuts]
ctrl-c = "clear-modes"
```

This removes all modes from the stack at once, returning to the top-level
shortcuts regardless of how many modes have been stacked.

## Practical Examples

### Move Mode

A dedicated mode for moving windows with `h`/`j`/`k`/`l`, activated by
`alt-r` and exited with `Escape`:

```toml
[shortcuts]
alt-r = { type = "push-mode", name = "move" }

[modes."move".shortcuts]
h = "move-left"
j = "move-down"
k = "move-up"
l = "move-right"
Escape = "pop-mode"
```

While in move mode, pressing `h` moves the focused window left, `l` moves it
right, and so on -- without needing a modifier key. All other inherited
shortcuts (like `alt-q` to quit) remain available. Press `Escape` to return
to normal.

### Leader Key

A leader key pattern where pressing `alt-x` followed by one key triggers a
mode shortcut, then immediately returns to normal:

```toml
[shortcuts]
alt-x = { type = "latch-mode", name = "leader" }

[modes."leader".shortcuts]
t = { type = "exec", exec = "alacritty" }
b = { type = "exec", exec = "firefox" }
f = { type = "exec", exec = "thunar" }
q = "quit"
```

Press `alt-x` then `t` to launch a terminal. Because `latch-mode` auto-pops
after one shortcut, you are immediately back to normal shortcuts.

### Nested Modes

Modes can push other modes, creating layers of context:

```toml
[shortcuts]
alt-n = { type = "push-mode", name = "navigate" }

[modes."navigate".shortcuts]
m = { type = "push-mode", name = "move" }
Escape = "pop-mode"

[modes."move".shortcuts]
h = "move-left"
l = "move-right"
Escape = "pop-mode"
```

`alt-n` enters navigation mode. From there, pressing `m` enters move mode
on top of it. `Escape` pops the current mode one level at a time. Use
`clear-modes` if you want a shortcut that returns directly to the top level.

See [spec.generated.md](https://github.com/mahkoh/jay/blob/master/toml-spec/spec/spec.generated.md) for the full
specification of `InputMode`, `push-mode`, `latch-mode`, and related actions.
