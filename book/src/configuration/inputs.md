# Input Devices

Jay configures input devices through the `[[inputs]]` array. Each entry matches
one or more devices and applies settings such as acceleration, tap behavior,
and device-to-output mapping.

> [!NOTE]
> Input configuration defined in `config.toml` is only applied to devices
> connected after the configuration is loaded. To change settings for
> already-connected devices, use `jay input` or the `configure-input` action.

## Matching input devices

Every `[[inputs]]` entry requires a `match` field. When `match` is a
**table**, all specified fields must match (AND logic). When `match` is an
**array**, any entry matching is sufficient (OR logic).

### By device name

```toml
[[inputs]]
match.name = "Logitech G300s Optical Gaming Mouse"
left-handed = true
```

Run `jay input` to see the names of all connected input devices.

### By device type

Match all devices of a given type using boolean flags:

```toml
[[inputs]]
match.is-pointer = true
natural-scrolling = true
```

Available type flags: `is-keyboard`, `is-pointer`, `is-touch`,
`is-tablet-tool`, `is-tablet-pad`, `is-gesture`, `is-switch`.

### By syspath or devnode

The `syspath` is usually stable across reboots and useful when you have
multiple identical devices:

```toml
[[inputs]]
match.syspath = "/sys/devices/pci0000:00/0000:00:08.1/0000:14:00.4/usb5/5-1/5-1.1/5-1.1.2/5-1.1.2:1.0"
left-handed = true
```

The `devnode` (e.g. `/dev/input/event4`) is typically not stable across
reboots.

### Combining criteria

AND -- all fields in a single table must match:

```toml
[[inputs]]
match = { name = "SynPS/2 Synaptics TouchPad", is-pointer = true }
tap-enabled = true
```

OR -- any entry in an array may match:

```toml
[[inputs]]
match = [
    { name = "Logitech G300s Optical Gaming Mouse" },
    { name = "Razer DeathAdder V2" },
]
left-handed = true
```

## Tagging devices

Assign a `tag` to reference a device from shortcuts or actions:

```toml
[[inputs]]
tag = "mouse"
match.is-pointer = true

[shortcuts]
alt-l = {
    type = "configure-input",
    input = {
        match.tag = "mouse",
        left-handed = true,
    },
}
alt-r = {
    type = "configure-input",
    input = {
        match.tag = "mouse",
        left-handed = false,
    },
}
```

Tags work similarly to output names -- they let you refer to matched devices
elsewhere in the configuration.

## Libinput settings

These settings map directly to libinput device options. See the
[libinput documentation](https://wayland.freedesktop.org/libinput/doc/latest/)
for detailed explanations of each.

### Acceleration

```toml
[[inputs]]
match.is-pointer = true
accel-profile = "Flat"
accel-speed = 0.0
```

| Field          | Values                     | Description                            |
|----------------|----------------------------|----------------------------------------|
| `accel-profile`| `Flat` or `Adaptive`       | Pointer acceleration curve             |
| `accel-speed`  | `-1.0` to `1.0`           | Speed within the selected profile      |

### Tap and click

```toml
[[inputs]]
match.is-pointer = true
tap-enabled = true
tap-drag-enabled = true
tap-drag-lock-enabled = false
click-method = "clickfinger"
```

| Field                   | Values                                     | Description                               |
|-------------------------|--------------------------------------------|-------------------------------------------|
| `tap-enabled`           | `true` / `false`                           | Tap-to-click on touchpads                 |
| `tap-drag-enabled`      | `true` / `false`                           | Tap-and-drag on touchpads                 |
| `tap-drag-lock-enabled` | `true` / `false`                           | Keep drag active after lifting finger     |
| `click-method`          | `none`, `button-areas`, `clickfinger`      | How physical clicks are interpreted       |

### Other libinput options

```toml
[[inputs]]
match.is-pointer = true
left-handed = true
natural-scrolling = true
middle-button-emulation = true
```

`left-handed`
: Swap left and right buttons

`natural-scrolling`
: Reverse scroll direction ("macOS-style")

`middle-button-emulation`
: Simultaneous left+right click produces a middle click

## Scroll speed

Control how many pixels each scroll wheel detent produces:

```toml
[[inputs]]
match.is-pointer = true
px-per-wheel-scroll = 30
```

This setting maps to the legacy `wl_pointer.axis` event that is mostly unused
nowadays. It has no effect on applications that don't use this event.

## Transform matrix

Apply a 2x2 matrix to relative motion events. This is useful for adjusting
pointer speed independently of libinput acceleration:

```toml
[[inputs]]
match.is-pointer = true
transform-matrix = [[0.35, 0], [0, 0.35]]
```

The example above reduces pointer speed to 35% of normal. The identity matrix
is `[[1, 0], [0, 1]]`.

## Calibration matrix

A 2x3 matrix for absolute input devices (touchscreens). This is passed
directly to libinput:

```toml
[[inputs]]
match.is-touch = true
calibration-matrix = [[0, 1, 0], [-1, 0, 1]]
```

The example above rotates touch input by 90 degrees.

## Per-device keymap

Override the global keymap for a specific keyboard:

```toml
[[inputs]]
match.name = "ZSA Technology Labs Inc ErgoDox EZ"
keymap.rmlvo = {
    layout = "us",
    options = "compose:ralt",
}
```

The override becomes active when a key is pressed on that device. See the
[Keymaps & Repeat Rate](keymaps.md) chapter for the full range of keymap
options.

## Mapping to outputs

Map tablets and touchscreens to a specific output so that the input area
corresponds to the correct display:

```toml
[[outputs]]
name = "left"
match.serial-number = "33K03894SL0"

[[inputs]]
match.name = "Wacom Bamboo Comic 2FG Pen"
output.name = "left"
```

You can also map by connector:

```toml
[[inputs]]
match.name = "Wacom Bamboo Comic 2FG Pen"
output.connector = "DP-1"
```

To remove a mapping at runtime, use the `remove-mapping` field in a
`configure-input` action:

```toml
[shortcuts]
alt-x = {
    type = "configure-input",
    input = {
        match.tag = "wacom",
        remove-mapping = true,
    },
}
```

## Lid switch events

Lid switch devices report when a laptop lid is opened or closed. Use the
`on-lid-closed` and `on-lid-opened` fields to trigger actions. These fields
only work in the top-level `[[inputs]]` array:

```toml
[[inputs]]
match.name = "<lid switch name>"
on-lid-closed = {
    type = "configure-connector",
    connector = {
        match.name = "eDP-1",
        enabled = false,
    },
}
on-lid-opened = {
    type = "configure-connector",
    connector = {
        match.name = "eDP-1",
        enabled = true,
    },
}
```

Run `jay input` to find the name of your lid switch device.

## Convertible (2-in-1) events

For convertible laptops that switch between laptop and tablet form factors:

```toml
[[inputs]]
match.name = "<switch name>"
on-converted-to-laptop = "$enable-keyboard"
on-converted-to-tablet = "$disable-keyboard"
```

These fields only work in the top-level `[[inputs]]` array.

## Runtime changes

### Listing devices

```shell
~$ jay input
```

This shows all connected input devices with their names, syspaths, devnodes,
type flags, and current settings.

### Changing settings at runtime

```shell
~$ jay input device <id> set-accel-profile flat
~$ jay input device <id> set-accel-speed 0.5
~$ jay input device <id> set-tap-enabled true
~$ jay input device <id> set-left-handed true
~$ jay input device <id> set-natural-scrolling true
~$ jay input device <id> set-transform-matrix 0.35 0 0 0.35
```

### Using shortcuts

The `configure-input` action applies settings from a keybinding:

```toml
[shortcuts]
alt-n = {
    type = "configure-input",
    input = {
        match.tag = "touchpad",
        natural-scrolling = true,
    },
}
```

## Full reference

For the exhaustive list of all input fields, match criteria, and related types,
see the [auto-generated specification](https://github.com/mahkoh/jay/blob/master/toml-spec/spec/spec.generated.md).
