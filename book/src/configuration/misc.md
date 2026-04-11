# Miscellaneous

This chapter covers smaller configuration options that don't warrant their own
chapter.

## Color Management

The Wayland color management protocol lets applications communicate color space
information to the compositor. It is disabled by default.

```toml
[color-management]
enabled = true
```

> [!NOTE]
> Changing this setting has no effect on applications that are already running.

The CLI and control center (**Color Management** pane) can also toggle it:

```shell
~$ jay color-management enable
~$ jay color-management disable
~$ jay color-management status
```

See [HDR & Color Management](../hdr.md) for a complete guide to HDR output and
the color management protocol.

## Session Management

Jay implements the `xdg_session_manager_v1` protocol. Applications that
support it can ask the compositor to remember their windows across restarts.
Jay stores each toplevel's workspace, output, floating-window position, and
fullscreen state, and restores them when the application re-creates the
session after a compositor or application restart.

Session data is stored in a local SQLite database under Jay's data directory
(typically `~/.local/share/jay/db/`).

The protocol is enabled by default. To disable it:

```toml
[session-management]
enabled = false
```

> [!NOTE]
> Changing this setting has no effect on applications that are already
> running. Disabling session management only hides the global from newly
> connecting clients -- previously stored session data is not deleted.

The control center's **Compositor** pane has a **Session Management** toggle
that flips this setting at runtime.

## Libei

[libei](https://gitlab.freedesktop.org/libinput/libei) allows applications to
emulate input events. By default, applications can only access libei through
the portal (which prompts the user for permission). Setting `enable-socket`
exposes an unauthenticated socket that any application can use without a prompt.

```toml
libei.enable-socket = false  # default
```

## UI Drag

Controls whether workspaces and tiles can be dragged with the mouse, and how
far the pointer must move before a drag begins.

```toml
ui-drag = { enabled = true, threshold = 10 }  # defaults
```

Set `enabled = false` to disable drag-and-drop rearrangement entirely. Increase
`threshold` if you find yourself accidentally starting drags.

## Floating Window Pin Icon

Floating windows can show a small pin icon. This is hidden by default.

```toml
[float]
show-pin-icon = true
```

## Workspace Capture

Controls whether newly created workspaces can be captured (e.g. for screen
sharing). The default is `true`.

```toml
workspace-capture = false
```

## Simple Input Method

Jay includes a built-in XCompose-based input method. It is enabled by default
but only activates when no external input method is running.

```toml
[simple-im]
enabled = true  # default
```

Related actions for use in shortcuts:

`enable-simple-im`
: Enable the built-in input method

`disable-simple-im`
: Disable the built-in input method

`toggle-simple-im-enabled`
: Toggle the built-in input method

`reload-simple-im`
: Reload XCompose files without restarting

`enable-unicode-input`
: Start Unicode codepoint input (requires active IM)

## Log Level

Sets the compositor's log verbosity. Valid values: `trace`, `debug`, `info`,
`warn`, `error`.

```toml
log-level = "info"
```

This setting **cannot** be changed by reloading the configuration. Use the CLI
instead:

```shell
~$ jay set-log-level debug
```

## Log File Cleanup

Jay creates a new log file each time it starts. Over time, old log files can
accumulate. To automatically delete old log files on startup, use the
`clean-logs-older-than` option:

```toml
clean-logs-older-than.days = 7
```

The table accepts `weeks` and `days` fields. At least one must be specified.
They can be combined and accept fractional values:

```toml
[clean-logs-older-than]
weeks = 2
days = 3
```

Log files belonging to other running Jay instances (e.g. on another VT) are
never deleted, even if they are older than the specified age.

> [!NOTE]
> This setting only takes effect at compositor startup. It cannot be triggered
> by reloading the configuration.

## Focus Follows Mouse

When enabled, moving the pointer over a window automatically gives it keyboard
focus.

```toml
focus-follows-mouse = true  # default
```

## Window Management Key

Designates a key that, while held, enables window management mode. In this
mode, the left mouse button moves floating windows and the right mouse button
resizes any window.

```toml
window-management-key = "Alt_L"
```

The value should be a keysym name (see the
[xkbcommon keysym list](https://github.com/xkbcommon/libxkbcommon/blob/master/include/xkbcommon/xkbcommon-keysyms.h)
with the `XKB_KEY_` prefix removed).

## Middle-Click Paste

Controls whether middle-clicking pastes the primary selection. Changing this
has no effect on running applications.

```toml
middle-click-paste = true  # default
```

## Pointer Revert Key

Pressing this key cancels any active grabs, drags, or selections, returning the
pointer to its default state. The default is `Escape`.

```toml
pointer-revert-key = "Escape"  # default
```

Set it to `NoSymbol` to disable this functionality entirely:

```toml
pointer-revert-key = "NoSymbol"
```

## Fallback Output Mode

Determines which output is used when no particular output is specified -- for
example, when placing a newly opened window or choosing which workspace to move
with `move-to-output`.

`cursor`
: Use the output the cursor is on (default)

`focus`
: Use the output the focused window is on

```toml
fallback-output-mode = "cursor"  # default
```

## Focus History

Configures the behavior of the `focus-prev` and `focus-next` actions.

`only-visible`
: Only cycle to windows that are already visible. Default: `false`.

`same-workspace`
: Only cycle to windows on the current workspace. Default: `false`.

If `only-visible` is `false`, switching to a non-visible window will make it
visible first.

```toml
[focus-history]
only-visible = true
same-workspace = true
```

## Control Center Fonts

The `[egui]` table configures fonts used by the control center (an egui-based
GUI).

```toml
[egui]
proportional-fonts = ["sans-serif", "Noto Sans", "Noto Color Emoji"]  # default
monospace-fonts = ["monospace", "Noto Sans Mono", "Noto Color Emoji"]  # default
```

Override these lists to use your preferred fonts in the control center UI.
