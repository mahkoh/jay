# HDR & Color Management

Jay supports HDR (High Dynamic Range) output and the Wayland color management
protocol. This chapter explains how HDR works in Jay and walks through
enabling it.

## Prerequisites

HDR in Jay requires:

- The **Vulkan renderer**. The OpenGL renderer does not support color
  management or HDR. Vulkan is the default when available; you can verify
  with `jay randr show` (look for the "Api" field under your GPU) or switch
  to it in the [control center](control-center.md) GPUs pane.
- A **monitor that supports HDR**. The display must advertise PQ (Perceptual
  Quantizer) support and BT.2020 colorimetry through its EDID. Run
  `jay randr show` to see which color spaces and EOTFs your display supports.

## Quick start

Enable HDR10 on a specific output using the CLI:

```shell
~$ jay randr output DP-1 colors set bt2020 pq
```

Or in your configuration file:

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
color-space = "bt2020"
transfer-function = "pq"
```

Enable the color management protocol so that applications can communicate
their color spaces to the compositor:

```shell
~$ jay color-management enable
```

Or in your configuration file:

```toml
[color-management]
enabled = true
```

To return to SDR:

```shell
~$ jay randr output DP-1 colors set default default
```

> [!NOTE]
> Run `jay randr` to find connector names and serial numbers for your
> displays.

## Concepts

### Color space

The color space determines the range of colors (gamut) the output uses.

`default`
: sRGB gamut. This is the standard gamut for desktop displays.

`bt2020`
: BT.2020 gamut. A much wider gamut used by HDR10 and modern cinema standards.

### Transfer function (EOTF)

The transfer function (technically the Electro-Optical Transfer Function)
controls how encoded pixel values are mapped to display luminance.

`default`
: Gamma 2.2. The traditional SDR transfer function.

`pq`
: Perceptual Quantizer (SMPTE ST 2084). The HDR10 transfer function, designed
  to cover luminance levels from 0 to 10,000 cd/m^2.

Setting `color-space = "bt2020"` and `transfer-function = "pq"` together
activates HDR10 mode. Jay sends the appropriate HDR metadata infoframe to the
display, including the display's mastering luminance and primaries from its
EDID.

### Brightness

The brightness setting controls SDR content luminance in cd/m^2. This
determines how bright non-HDR content appears on an HDR display.

With the `default` (gamma 2.2) transfer function, the default brightness is
the maximum the display supports, anchored at 80 cd/m^2. Setting brightness
below 80 makes SDR content dimmer while creating HDR headroom above it.

With the `pq` transfer function, the default brightness is 203 cd/m^2 (the
ITU reference level for SDR content in an HDR signal).

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
color-space = "bt2020"
transfer-function = "pq"
brightness = 250
```

```shell
~$ jay randr output DP-1 brightness 250
~$ jay randr output DP-1 brightness default
```

> [!NOTE]
> Brightness has no effect unless the Vulkan renderer is in use.

### Blend space

When Jay composites overlapping translucent surfaces, the blend space
determines in which color space the alpha blending is performed.

`srgb`
: Classic desktop blending in sRGB space. This is the default and matches the
  behavior of most other compositors.

`linear`
: Blending in linear light. This is physically correct but can make
  semi-transparent elements appear brighter than expected.

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
blend-space = "linear"
```

```shell
~$ jay randr output DP-1 blend-space linear
```

### Native gamut

By default, Jay assumes all displays use sRGB primaries. This matches the
behavior of most other compositors, but most modern displays actually have a
wider gamut (e.g. 95% DCI-P3 coverage). Because the display interprets
sRGB-intended color values in its wider native gamut, colors appear more
saturated than they should.

Setting `use-native-gamut = true` tells Jay the display's actual primaries
from its EDID, allowing the compositor to map colors correctly. This produces
less saturated but more accurate colors, and allows color-managed applications
to address the full display gamut.

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
use-native-gamut = true
```

```shell
~$ jay randr output DP-1 use-native-gamut true
```

This setting has no effect when the display is already operating in a wide
color space like BT.2020.

### Framebuffer format

The default framebuffer format (`xrgb8888`) uses 8 bits per channel, which
can cause banding in HDR content. For HDR, consider a higher-precision format:

- `xrgb2101010` or `argb2101010` -- 10 bits per channel. Good balance between
  precision and performance.
- `xbgr16161616f` or `abgr16161616f` -- 16-bit floating point per channel.
  Maximum precision.

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
color-space = "bt2020"
transfer-function = "pq"
format = "xrgb2101010"
```

```shell
~$ jay randr output DP-1 format set xrgb2101010
```

> [!TIP]
> Run `jay randr show --formats` to see which formats your display supports.

## Color management protocol

The Wayland color management protocol (`wp_color_manager_v1`) lets
applications communicate their color space to the compositor. This enables
color-aware applications to render correctly regardless of the output's color
settings -- for example, an image viewer can tag its surfaces as sRGB and Jay
will map the colors to the output's actual gamut and transfer function.

The protocol is **disabled by default** and must be enabled explicitly:

```toml
[color-management]
enabled = true
```

Or at runtime:

```shell
~$ jay color-management enable
```

> [!NOTE]
> Changing this setting has no effect on applications that are already running.
> They must be restarted to discover the protocol.

Color management availability requires both the protocol to be enabled **and**
the Vulkan renderer to be active. Check with:

```shell
~$ jay color-management status
```

This prints one of:

`Enabled`
: The protocol is enabled and available to clients.

`Enabled (Unavailable)`
: The protocol is enabled but unavailable -- usually because the OpenGL
  renderer is in use.

`Disabled`
: The protocol is disabled.

The [control center](control-center.md) **Color Management** pane shows the
same information as a toggle and a read-only availability indicator.

### Supported capabilities

Jay's color management implementation supports:

- **Parametric image descriptions** with custom primaries, luminances, and
  transfer functions (including power curves).
- **Named primaries**: sRGB, BT.2020, DCI-P3, Display P3, Adobe RGB, and
  others.
- **Named transfer functions**: sRGB, PQ (ST 2084), gamma 2.2, gamma 2.4,
  BT.1886, linear, and others.
- **Mastering display metadata** for HDR content.
- **Windows scRGB** compatibility.

ICC profile creation is not supported.

## Checking display capabilities

Use `jay randr show` to inspect what your display supports:

```shell
~$ jay randr show
```

The output for each connector includes:

- **Color spaces** -- which color spaces the display supports (e.g. `default`,
  `bt2020`), with the current setting marked.
- **EOTFs** -- which transfer functions the display supports (e.g. `default`,
  `pq`), with the current setting marked.
- **Brightness range** -- minimum and maximum luminance from the EDID, in
  cd/m^2.
- **Current brightness** -- displayed if a custom value is set.
- **Blend space** -- current blend space setting.
- **Native gamut** -- the display's CIE xy primaries for red, green, blue, and
  white point.

## Complete example

A typical HDR configuration for a monitor that supports HDR10:

```toml
[[outputs]]
match.serial-number = "33K03894SL0"
color-space = "bt2020"
transfer-function = "pq"
brightness = 203
format = "xrgb2101010"
blend-space = "linear"

[color-management]
enabled = true
```

This sets the output to HDR10 with 10-bit color, physically correct blending,
and enables the color management protocol so applications can communicate
their color spaces.

## Control center

The Outputs pane in the [control center](control-center.md) (`alt-c` by
default) provides GUI controls for all HDR-related settings per output:
Colorimetry, EOTF, Custom Brightness, Format, Blend Space, Use Native Gamut,
and a read-only Native Gamut display. See
[Outputs (Monitors)](configuration/outputs.md) for details on each field.

The Color Management pane has a toggle to enable/disable the protocol and a
read-only indicator showing whether color management is currently available.

## See also

- [Outputs (Monitors)](configuration/outputs.md) -- per-output configuration
  reference including all color and HDR fields
- [Miscellaneous](configuration/misc.md) -- the `[color-management]` config
  table
- [Command-Line Interface](cli.md) -- `jay randr output` color commands and
  `jay color-management`
- [GPUs](configuration/gpu.md) -- renderer selection (Vulkan is required for
  HDR)
