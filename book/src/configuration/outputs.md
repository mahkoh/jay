# Outputs (Monitors)

Jay configures monitors through the `[[outputs]]` array. Each entry matches
one or more connected displays and applies settings such as position, scale,
mode, and color management.

> [!NOTE]
> Output configuration defined in `config.toml` is only applied the first time
> a matching output is connected after the compositor starts. To change
> settings at runtime, use `jay randr` or the `configure-output` action.

## Matching outputs

Every `[[outputs]]` entry requires a `match` field that selects which monitors
the settings apply to. You can match by serial number, connector name,
manufacturer, or model.

When `match` is a **table**, all specified fields must match (AND logic). When
`match` is an **array**, any entry matching is sufficient (OR logic).

### By serial number (recommended)

The serial number is a unique identifier that stays the same regardless of
which port the monitor is plugged into:

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
scale = 1.5
```

Run `jay randr` to find the serial number of your connected displays.

### By connector name

```toml
[[outputs]]
match.connector = "DP-1"
scale = 1.25
```

Connector names (like `DP-1`, `HDMI-A-1`, `eDP-1`) can change if you move
cables between ports.

### By manufacturer and model

```toml
[[outputs]]
match = { manufacturer = "BNQ", model = "BenQ GW2480" }
scale = 1.25
```

When multiple fields appear in a single table, all must match.

### Combining criteria (OR)

Use an array to match any of several outputs with the same settings:

```toml
[[outputs]]
match = [
    { serial-number = "33K03894SL0" },
    { serial-number = "ETW1M02062SL0" },
]
scale = 2.0
```

## Naming outputs

Assign a `name` to reference an output from other parts of the config -- for
example, when mapping a tablet to a specific monitor or using shortcuts to
reconfigure outputs at runtime:

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

Other rules can then reference these names:

```toml
# Map a drawing tablet to the left monitor
[[inputs]]
match.name = "Wacom Intuos Pro M Pen"
output.name = "left"
```

```toml
# Rotate a named output with a shortcut
[shortcuts]
alt-r = {
    type = "configure-output",
    output = {
        match.name = "right",
        transform = "rotate-90",
    },
}
```

## Position

The `x` and `y` fields place outputs in compositor space. Coordinates are
integers >= 0 and represent the top-left corner of the output:

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
x = 0
y = 0

[[outputs]]
match.serial-number = "ETW1M02062SL0"
x = 2560
y = 0
```

> [!TIP]
> The control center (`alt-c` by default) includes a visual output arrangement
> editor where you can drag monitors into position.

## Scale

Set fractional scaling with a number greater than 0:

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
scale = 1.5
```

Common values: `1.0` (no scaling), `1.25`, `1.5`, `2.0`.

## Transform

Rotate or flip the output. The available values are:

`none`
: No transformation

`rotate-90`
: Rotate 90 degrees counter-clockwise

`rotate-180`
: Rotate 180 degrees

`rotate-270`
: Rotate 270 degrees counter-clockwise

`flip`
: Flip around the vertical axis

`flip-rotate-90`
: Flip vertically, then rotate 90 degrees counter-clockwise

`flip-rotate-180`
: Flip vertically, then rotate 180 degrees

`flip-rotate-270`
: Flip vertically, then rotate 270 degrees counter-clockwise

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
transform = "rotate-90"
```

## Mode

Set the resolution and refresh rate with the `mode` field. If `refresh-rate` is
omitted, the first available mode with the specified resolution is used:

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
mode = {
    width = 2560,
    height = 1440,
    refresh-rate = 144.0,
}
```

Use `jay randr` to see all available modes for each output:

```shell
~$ jay randr show --modes
```

## Variable Refresh Rate (VRR)

VRR (also known as FreeSync or Adaptive Sync) allows the display to vary its
refresh rate to match the content being rendered, reducing stuttering and
tearing.

Configure VRR with the `vrr` field:

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
vrr = { mode = "variant1", cursor-hz = 90 }
```

### VRR modes

`never`
: VRR is always off (default)

`always`
: VRR is always on

`variant1`
: VRR is on when one or more applications are displayed fullscreen

`variant2`
: VRR is on when exactly one application is displayed fullscreen

`variant3`
: VRR is on when a single application is displayed fullscreen and describes its
  content type as video or game via the `wp_content_type_v1` protocol

### Cursor refresh rate

When VRR is active, cursor movement can cause the screen to spike to maximum
refresh rate. The `cursor-hz` field limits cursor-triggered updates:

```toml
vrr = { mode = "always", cursor-hz = 90 }
```

Set `cursor-hz = "none"` for unbounded cursor updates (the default). A numeric
value means the cursor is updated at that rate in Hz, or faster if the
application is already driving updates above that rate.

You can also set default VRR settings for all outputs at the top level:

```toml
vrr = { mode = "variant1", cursor-hz = 90 }
```

Per-output settings override the top-level default.

## Tearing

Tearing allows frames to be presented immediately instead of waiting for
vertical blank, reducing input latency at the cost of visible tearing
artifacts.

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
tearing.mode = "variant3"
```

### Tearing modes

`never`
: Tearing is never enabled

`always`
: Tearing is always enabled

`variant1`
: Tearing is enabled when one or more applications are displayed fullscreen

`variant2`
: Tearing is enabled when a single application is displayed fullscreen

`variant3`
: Tearing is enabled when a single application is displayed and has requested tearing (default)

The default tearing mode is `variant3`. Like VRR, per-output settings override
top-level defaults:

```toml
tearing.mode = "never"
```

## Framebuffer format

The default framebuffer format is `xrgb8888`. You can change it to any DRM
fourcc format:

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
format = "rgb565"
```

Common formats include `xrgb8888`, `argb8888`, `xbgr8888`, `abgr8888`,
`rgb565`, and many others. See the
[auto-generated specification](https://github.com/mahkoh/jay/blob/master/toml-spec/spec/spec.generated.md) for the full list.

## HDR and color management

Jay supports HDR output through color space, transfer function, and brightness
settings. These require the Vulkan renderer. For a conceptual overview and
step-by-step guide, see [HDR & Color Management](../hdr.md).

### Color space

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
color-space = "bt2020"
```

Values: `default` (usually sRGB) or `bt2020`.

### Transfer function (EOTF)

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
transfer-function = "pq"
```

Values: `default` (usually gamma 2.2) or `pq` (Perceptual Quantizer, used for
HDR10).

### Brightness

Set SDR content brightness in cd/m^2 or use `"default"`:

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
brightness = 80
```

The default depends on the transfer function:
- With `default` EOTF: the maximum brightness of the display, anchored at
  80 cd/m^2. Setting a value below 80 creates HDR headroom.
- With `pq`: 203 cd/m^2.

This setting has no effect unless the Vulkan renderer is in use.

## Blend space

Controls how colors are blended when compositing overlapping surfaces:

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
blend-space = "linear"
```

`srgb`
: Classic desktop blending in sRGB space (default)

`linear`
: Physically correct blending in linear light -- produces brighter results

## Native gamut

By default, Jay assumes displays use sRGB primaries (matching the behavior of
most other compositors). In reality, many displays have a wider gamut.

Setting `use-native-gamut = true` tells Jay to use the primaries advertised in
the display's EDID. This can produce more accurate colors and allows
color-managed applications to use the full gamut:

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
use-native-gamut = true
```

This has no effect when the display is explicitly operating in a wide color
space (e.g. BT.2020).

## Disabling outputs

Setting `enabled = false` disables an output by default:

```toml
[[outputs]]
match.serial-number = "123456789"
enabled = false
```

This is useful for disabling a specific display regardless of which port it is
connected to. Disabled outputs can be re-enabled at runtime using the CLI.

## Connector configuration

The `[[connectors]]` array lets you enable or disable physical display
connectors. This is useful for permanently disabling a port:

```toml
[[connectors]]
match.name = "eDP-1"
enabled = false
```

Connector configuration is applied when the connector is first discovered by
the compositor, which typically happens only at startup.

### Precedence

Both `[[outputs]]` and `[[connectors]]` can enable or disable a display. If
both match the same connector, the `[[outputs]]` setting takes precedence.

`[[connectors]]` matches by physical port name (e.g. `eDP-1`, `HDMI-A-1`) and
is applied when the port is first discovered. `[[outputs]]` matches by display
identity (serial number, manufacturer, model) and is applied when the display is
first connected, overriding any earlier connector setting.

## Lid switch (auto-disable laptop screen)

On laptops, you can automatically disable the built-in display when the lid is
closed using the `[[inputs]]` array. The lid switch is an input device:

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

Run `jay input` to find the name of your lid switch device. See the
[Input Devices](inputs.md) chapter for more details.

## Runtime changes

Output settings in `config.toml` are only applied when a display is first
connected after compositor startup. For runtime changes, use the `jay randr`
CLI or the `configure-output` action.

### Listing outputs

```shell
~$ jay randr
```

This shows all connected outputs with their connector names, serial numbers,
current modes, scales, transforms, and available modes.

### Changing settings at runtime

```shell
~$ jay randr output <name-or-connector> scale 1.5
~$ jay randr output <name-or-connector> mode 2560 1440 144.0
~$ jay randr output <name-or-connector> position 1920 0
~$ jay randr output <name-or-connector> transform rotate-90
~$ jay randr output <name-or-connector> enable
~$ jay randr output <name-or-connector> disable
```

### Using shortcuts

The `configure-output` action lets you change output settings from a
keybinding:

```toml
[shortcuts]
alt-F7 = {
    type = "configure-output",
    output = {
        match.name = "right",
        transform = "none",
    },
}
alt-F8 = {
    type = "configure-output",
    output = {
        match.name = "right",
        transform = "rotate-90",
    },
}
```

## Full reference

For the exhaustive list of all output fields, match criteria, and related
types, see the [auto-generated specification](https://github.com/mahkoh/jay/blob/master/toml-spec/spec/spec.generated.md).
