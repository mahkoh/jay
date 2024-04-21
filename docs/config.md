# Configuration

Jay can be configured via

- a declarative TOML file or
- a shared library that gets injected into the compositor.

## Shared Library Configuration

This is described in the [rustdoc](https://docs.rs/jay-config) of the configuration crate.

## TOML Configuration

The configuration file is stored under `$HOME/.config/jay/config.toml`.
If you don't have such a file, the default configuration will be used.

The full format of this file is described in the auto-generated file [spec.generated.md](../toml-spec/spec/spec.generated.md).
You can also get auto completion with the auto-generated JSON Schema linked from that document.

The following code block contains the annotated default configuration.
Below that we will describe individual usecases.

```toml
# The keymap that is used for shortcuts and also sent to clients.
keymap = """
    xkb_keymap {
        xkb_keycodes { include "evdev+aliases(qwerty)" };
        xkb_types    { include "complete"              };
        xkb_compat   { include "complete"              };
        xkb_symbols  { include "pc+us+inet(evdev)"     };
    };
    """

# An action that will be executed when the GPU has been initialized.
on-graphics-initialized = { type = "exec", exec = { prog = "mako", privileged = true } }

# Shortcuts that are processed by the compositor.
# The left hand side should be a key, possibly prefixed with modifiers.
# The right hand side should be an action.
[shortcuts]
# The focus-X actions move the keyboard focus to next window on the X.
alt-h = "focus-left"
alt-j = "focus-down"
alt-k = "focus-up"
alt-l = "focus-right"

# The move-X actions move window that has the keyboard focus to the X.
alt-shift-h = "move-left"
alt-shift-j = "move-down"
alt-shift-k = "move-up"
alt-shift-l = "move-right"

# The split-X action places the currently focused window in a container
# and sets the split direction of the container to X.
alt-d = "split-horizontal"
alt-v = "split-vertical"

# The toggle-split action changes the split direction of the current
# container.
alt-t = "toggle-split"
# The toggle-mono action changes whether the current container shows
# a single window or all windows next to each other.
alt-m = "toggle-mono"
# The toggle-fullscreen action toggles the current window between
# windowed and fullscreen.
alt-u = "toggle-fullscreen"

# The focus-parent action moves the keyboard focus to the parrent of
# the currently focused window.
alt-f = "focus-parent"
# The close action requests the currently focused window to close.
alt-shift-c = "close"
# The toggle-floating action changes the currently focused window between
# floating and tiled.
alt-shift-f = "toggle-floating"

# All actions above are so-called simple actions that are identified by
# a string. More complex actions take parameters and are written as a table.
# For example, the exec action spawns an application and has the exec field
# that describes how to spawn the application.
Super_L = { type = "exec", exec = "alacritty" }
alt-p = { type = "exec", exec = { prog: "bemenu-run", privileged = true } }

# The quit action terminates the compositor.
alt-q = "quit"
# The reload-config-toml action reloads the TOML configuration file.
alt-shift-r = "reload-config-toml"

# The switch-to-vt action switches to a different virtual terminal.
ctrl-alt-F1 = { type = "switch-to-vt", num = 1 }
ctrl-alt-F2 = { type = "switch-to-vt", num = 2 }
ctrl-alt-F3 = { type = "switch-to-vt", num = 3 }
ctrl-alt-F4 = { type = "switch-to-vt", num = 4 }
ctrl-alt-F5 = { type = "switch-to-vt", num = 5 }
ctrl-alt-F6 = { type = "switch-to-vt", num = 6 }
ctrl-alt-F7 = { type = "switch-to-vt", num = 7 }
ctrl-alt-F8 = { type = "switch-to-vt", num = 8 }
ctrl-alt-F9 = { type = "switch-to-vt", num = 9 }
ctrl-alt-F10 = { type = "switch-to-vt", num = 10 }
ctrl-alt-F11 = { type = "switch-to-vt", num = 11 }
ctrl-alt-F12 = { type = "switch-to-vt", num = 12 }

# The show-workspace action switches to a workspace. If the workspace is not
# currently being used, it is created on the output that contains the pointer.
alt-F1 = { type = "show-workspace", name = "1" }
alt-F2 = { type = "show-workspace", name = "2" }
alt-F3 = { type = "show-workspace", name = "3" }
alt-F4 = { type = "show-workspace", name = "4" }
alt-F5 = { type = "show-workspace", name = "5" }
alt-F6 = { type = "show-workspace", name = "6" }
alt-F7 = { type = "show-workspace", name = "7" }
alt-F8 = { type = "show-workspace", name = "8" }
alt-F9 = { type = "show-workspace", name = "9" }
alt-F10 = { type = "show-workspace", name = "10" }
alt-F11 = { type = "show-workspace", name = "11" }
alt-F12 = { type = "show-workspace", name = "12" }

# The move-to-workspace action moves the currently focused window to a workspace.
alt-shift-F1 = { type = "move-to-workspace", name = "1" }
alt-shift-F2 = { type = "move-to-workspace", name = "2" }
alt-shift-F3 = { type = "move-to-workspace", name = "3" }
alt-shift-F4 = { type = "move-to-workspace", name = "4" }
alt-shift-F5 = { type = "move-to-workspace", name = "5" }
alt-shift-F6 = { type = "move-to-workspace", name = "6" }
alt-shift-F7 = { type = "move-to-workspace", name = "7" }
alt-shift-F8 = { type = "move-to-workspace", name = "8" }
alt-shift-F9 = { type = "move-to-workspace", name = "9" }
alt-shift-F10 = { type = "move-to-workspace", name = "10" }
alt-shift-F11 = { type = "move-to-workspace", name = "11" }
alt-shift-F12 = { type = "move-to-workspace", name = "12" }
```

### Configuring Keymaps and Repeat Rates

The keymap can be configured via the top-level `keymap` field.

```toml
keymap = """
    xkb_keymap {
        xkb_keycodes { include "evdev+aliases(qwerty)" };
        xkb_types    { include "complete"              };
        xkb_compat   { include "complete"              };
        xkb_symbols  { include "pc+us+inet(evdev)"     };
    };
    """
```

The format is described in the ArchWiki: https://wiki.archlinux.org/title/X_keyboard_extension

If you want to use multiple keymaps, you can assign names to them:

```toml
keymap.name = "laptop"

[[keymaps]]
name = "laptop"
path = "./laptop-keymap.xkb"

[[keymaps]]
name = "external"
path = "./external-keymap.xkb"
```

Such paths are relative to the configuration file.
You can also write the map inline in this format:

```toml
[[keymaps]]
name = "external"
map = "..."
```

If you want to switch the keymap with a shortcut, use the `set-keymap` action:

```toml
[shortcuts]
alt-j = { type = "set-keymap", keymap.name = "laptop" }
alt-k = { type = "set-keymap", keymap.name = "external" }
```

The keyboard repeat rate is configured via the top-level `repeat-rate` field.

```toml
repeat-rate = { rate = 25, delay = 250 }
```

You can change this at runtime with the `set-repeat-rate` action:

```toml
[shortcuts]
alt-x = { type = "set-repeat-rate", rate = { rate = 25, delay = 250 } }
```

Note that you can change all of this from the command line with the `jay input` command.

### Configuring Shortcuts

Shortcuts are configured in the top-level `shortcuts` table.

```toml
[shortcuts]
alt-h = "focus-left"
```

The left-hand side should be a key that can optionally be prefixed with modifiers.

The right-hand side should be an action.

See [spec.generated.md](../toml-spec/spec/spec.generated.md) for a full list of actions.

### Complex Shortcuts

If you need more control over shortcut execution, you can use the `complex-shortcuts` table.

```toml
[complex-shortcuts.alt-x]
action = { type = "exec", exec = ["pactl", "set-sink-mute", "0", "1"] }
latch  = { type = "exec", exec = ["pactl", "set-sink-mute", "0", "0"] }
```

This mutes the audio output while the key is pressed and un-mutes once the `x` key is released.
The order in which `alt` and `x` are released does not matter for this.

This can also be used to implement push to talk.

See the specification for more details.

### Running Multiple Actions

In every place that accepts an action, you can also run multiple actions by wrapping them
in an array:

```toml
[shortcuts]
alt-h = ["focus-left", "focus-up"]
```

### Spawning Applications

You can spawn applications by using the `exec` action:

```toml
Super_L = { type = "exec", exec = "alacritty" }
```

The `exec` field can be either a string, an array of strings, or a table.

When a string is used, it should be the name of the application.

When an array is used, it should be the name of the application followed by arguments.

```toml
Super_L = { type = "exec", exec = ["alacritty", "-e", "date"] }
```

When a table is used, you can additionally specify

- environment variables to pass to the application,
- whether the application should have access to privileged protocols.

See the specification for more details.

### Running an Action at Startup

If you want to run an action at startup, you can use the top-level `on-graphics-initialized`
field:

```toml
on-graphics-initialized = { type = "exec", exec = { prog = "mako", privileged = true } }
```

### Setting Environment Variables

You can set environment variables with the the top level `env` table.

```toml
[env]
GTK_THEME = "Adwaita:dark"
```

These environment variables are passed to all applications started afterwards.

You can also use the `set-env` action to modify these variables:

```toml
[shortcuts]
alt-l = { type = "set-env", env.GTK_THEME = "Adwaita:dark" }
```

The `unset-env` action is similar.
See the specification for more details.

### Using a Status Program

You can configure a status program with the top-level `status` table.

```toml
[status]
format = "i3bar"
exec = "i3status"
```

The `format` field specifies the format used by the status program.
Possible values are `plain`, `pango`, and `i3bar`.

The `exec` field specifies how to start the status program.

Note that i3status will not automatically use i3bar format when started this way.
You have to explicitly opt into i3bar format in your i3status configuration.

See the specification for more details.

### Configuring Idle Timeout and Actions

You can configure the idle timeout with the top-level `idle` table.

```toml
idle.minutes = 10
```

If you want to lock the screen when this timeout happens, you can use the `on-idle` table.

```toml
on-idle = { type = "exec", exec = { prog = "swaylock", privileged = "true" } }
```

See the specification for more details.

### Configuring GPUs

You can configure GPUs with the top-level `drm-devices` array.

```toml
[[drm-devices]]
name = "dedicated"
match = { pci-vendor = 0x1002, pci-model = 0x73ff }

[[drm-devices]]
name = "integrated"
match = { pci-vendor = 0x1002, pci-model = 0x164e }
gfx-api = "OpenGl"
```

For each device, you can configure the following properties:

- Whether direct scanout is enabled on monitors connected to this device.
- Which API to use for this device (OpenGL or Vulkan).

You can assign names to these device to refer to them elsewhere.

The `match` field is used to identify the device.
Unless you have two identical graphics cards installed, using the pci-vendor and model
fields is usually the best choice.
You can get these values by running `jay randr`.

You can select the device used for rendering the desktop with the top-level `render-device` field.

```toml
render-device.name = "dedicated"
```

You can modify the render device and configure GPUs at runtime with the `set-render-device`
and `configure-drm-device` actions.

You can use the top-level `gfx-api` field to set the default API used (unless overwritten for specific device).

```toml
gfx-api = "Vulkan"
```

See the specification for more details.

### Configuring Monitors

You can configure monitors with the top-level `outputs` field.

```toml
[[outputs]]
name = "left"
match.serial-number = "33K03894SL0"
x = 0
y = 0

[[outputs]]
name = "right"
match.serial-number = "ETW1M02062SL0"
x = 1920
y = 0
```

For each output, you can configure the following properties:

- The x, y coordinates in global compositor space.
- The scale to use for the monitor.
- The transformation to apply to the content (rotation, mirroring).
- The mode to use for the monitor.

You can query the available modes and modify these properties from the command line with
the `jay randr` command.

The `match` field selects the monitors the configuration applies to.
The serial number is usually a good unique identifier.

You can assign a name to monitors to refer to them in other places.

You can use the `configure-output` action to change this configuration at runtime.

See the specification for more details.

### Configuring Connectors

Connectors are the physical ports at the back of your GPU.
You can configure them with the top-level `connectors` array.

```toml
[[connectors]]
name = "eDP-1"
enabled = false
```

Currently you can only use this to disable or enable connectors.
This is useful to disable the internal monitor of a laptop when the laptop is closed.

You can use the `configure-connector` action to change this configuration at runtime.

See the specification for more details.

### Configuring Input Devices

You can configure input devices with the top-level `inputs` array.

```toml
[[inputs]]
tag = "mice"
match.is-pointer = true
left-handed = true
transform-matrix = [[0.35, 0], [0, 0.35]]
tap-enabled = true
```

For each input device you can configure the following properties:

- The libinput acceleration profile.
- The libinput acceleration speed.
- The libinput tap setting.
- The libinput tap-drag setting.
- The libinput tap-drag-lock setting.
- The libinput left-handed setting.
- The libinput natural-scrolling setting.
- The number of pixels to scroll per scroll-wheel dedent.
- A transformation matrix to apply to relative movements.

You can inspect and modify these settings from the command line with the `jay input` command.

The `match` field selects the input devices to operate on.

You can assign a `tag` to input devices to refer to them elsewhere.

You can use the `configure-input` action to change these settings at runtime.

See the specification for more details.

# Theming

You can configure the colors, sizes, and fonts used by the compositor with the top-level `theme` table.

```toml
[theme]
bg-color = "#ff000"
```

See the specification for more details.
