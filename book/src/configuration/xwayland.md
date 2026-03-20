# Xwayland

Xwayland provides compatibility with legacy X11 applications inside a Wayland
session. Jay starts Xwayland automatically by default.

## Configuration

The `[xwayland]` table controls Xwayland behavior:

`enabled`
: Whether Xwayland is started. Default: `true`.

`scaling-mode`
: How X11 windows are scaled on HiDPI outputs. Default: `default`.

```toml
[xwayland]
enabled = true
scaling-mode = "default"
```

### Scaling Modes

`default`
: Render at the lowest scale, then upscale to other outputs

`downscaled`
: Render at the highest integer scale, then downscale to match each output

The `downscaled` mode produces sharper text and UI on HiDPI monitors but has a
significant performance cost. For example, on a 3840x2160 output at 1.5x scale,
a fullscreen X11 window would be rendered at 5120x2880 (integer scale 2) and
then downscaled -- roughly doubling the pixel count. This mode also requires
X11 applications to handle scaling themselves (e.g. `GDK_SCALE=2`).

## CLI

You can inspect and change Xwayland settings from the command line:

```shell
~$ jay xwayland status
~$ jay xwayland set-scaling-mode default
~$ jay xwayland set-scaling-mode downscaled
```

Xwayland settings are also available in the control center's **Xwayland** pane.

## Disabling Xwayland

If you don't need X11 compatibility, disabling Xwayland avoids starting the
X server entirely:

```toml
[xwayland]
enabled = false
```

## Matching X11 Windows in Rules

Window rules can target X11 windows using properties that only exist on X
clients. These fields are available in the `match` table of `[[windows]]`
rules:

`x-class` / `x-class-regex`
: Match the X11 WM_CLASS class (verbatim or regex)

`x-instance` / `x-instance-regex`
: Match the X11 WM_CLASS instance (verbatim or regex)

`x-role` / `x-role-regex`
: Match the X11 WM_WINDOW_ROLE (verbatim or regex)

For example, to float all GIMP tool windows:

```toml
[[windows]]
match.x-class = "Gimp"
match.x-role-regex = "gimp-(toolbox|dock)"
initial-tile-state = "floating"
```

### Matching at the Client Level

Client rules support the `is-xwayland` field to match (or exclude) the
Xwayland client itself:

```toml
[[clients]]
match.is-xwayland = true
# ... grant capabilities, etc.
```
