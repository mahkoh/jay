# GPUs

Jay configures graphics cards (DRM devices) through the `[[drm-devices]]`
array. This is primarily useful on multi-GPU systems for selecting the render
device, choosing a graphics API, and tuning per-device settings.

> [!NOTE]
> DRM device configuration in `config.toml` is only applied when a device is
> first discovered after the configuration is loaded. To change settings at
> runtime, use `jay randr` or the `configure-drm-device` action.

## Matching GPUs

Every `[[drm-devices]]` entry requires a `match` field. When `match` is a
**table**, all specified fields must match (AND). When `match` is an **array**,
any entry matching is sufficient (OR).

### By PCI vendor and model (recommended)

PCI IDs are stable, unique identifiers. Use `jay randr` to find them:

```shell
~$ jay randr
```

```toml
[[drm-devices]]
match = {
    pci-vendor = 0x1002,
    pci-model = 0x73ff,
}
gfx-api = "Vulkan"
```

### By vendor or model name

```toml
[[drm-devices]]
match.vendor = "Advanced Micro Devices, Inc. [AMD/ATI]"
```

```toml
[[drm-devices]]
match.model = "Raphael"
```

### By syspath or devnode

The `syspath` is usually stable across reboots:

```toml
[[drm-devices]]
match.syspath = "/sys/devices/pci0000:00/0000:00:08.1/0000:14:00.0"
```

The `devnode` (e.g. `/dev/dri/card0`) is typically not stable.

## Naming GPUs

Assign a `name` to reference a device from other parts of the config:

```toml
[[drm-devices]]
name = "dedicated"
match = {
    pci-vendor = 0x1002,
    pci-model = 0x73ff,
}

[[drm-devices]]
name = "integrated"
match = {
    pci-vendor = 0x1002,
    pci-model = 0x164e,
}
```

Names can then be used in `render-device`, shortcuts, and actions:

```toml
render-device.name = "dedicated"
```

```toml
[shortcuts]
alt-v = {
    type = "configure-drm-device",
    dev = {
        match.name = "dedicated",
        gfx-api = "Vulkan",
    },
}
alt-o = {
    type = "configure-drm-device",
    dev = {
        match.name = "dedicated",
        gfx-api = "OpenGl",
    },
}
```

## Graphics API

Jay supports two rendering backends per device:

`Vulkan`
: Uses libvulkan. The primary renderer -- use this unless you have a specific
  reason not to. Required for HDR. All devices in the system must support DRM
  format modifiers (most do, except AMD GPUs older than RX 5000).

`OpenGl`
: Uses libEGL + libGLESv2. Maintained for backwards compatibility only. No new
  features will be added to this renderer.

### Per-device API

```toml
[[drm-devices]]
match = {
    pci-vendor = 0x1002,
    pci-model = 0x73ff,
}
gfx-api = "Vulkan"
```

### Default API for all devices

Set the top-level `gfx-api` to apply to any device without a per-device
override:

```toml
gfx-api = "Vulkan"
```

This only takes effect for devices discovered after the config is loaded.

## Direct scanout

Direct scanout lets the compositor hand a client's buffer directly to the
display hardware, bypassing composition. This can reduce latency and power
usage, but may cause visual glitches with some hardware or applications.

### Per-device

```toml
[[drm-devices]]
match = {
    pci-vendor = 0x1002,
    pci-model = 0x73ff,
}
direct-scanout = false
```

### Global toggle

```toml
direct-scanout = false
```

## Flip margin

The flip margin is the time (in milliseconds) between the compositor
initiating a page flip and the output's vertical blank event. It determines
the minimum achievable input latency. The default is 1.5 ms.

```toml
[[drm-devices]]
match = {
    pci-vendor = 0x1002,
    pci-model = 0x73ff,
}
flip-margin-ms = 2.0
```

If the margin is set too small, the compositor will dynamically increase it to
avoid missed frames.

## Explicit sync

Explicit sync coordinates buffer access between the compositor and GPU drivers.
It is enabled by default and generally should not be disabled:

```toml
explicit-sync = true
```

> [!WARNING]
> This setting cannot be changed after the compositor has started. It can only
> be set in `config.toml` before launching Jay.

## Render device

On multi-GPU systems, select which GPU performs compositing with the
`render-device` field. The first matching device is used:

```toml
render-device.name = "dedicated"

[[drm-devices]]
name = "dedicated"
match = {
    pci-vendor = 0x1002,
    pci-model = 0x73ff,
}

[[drm-devices]]
name = "integrated"
match = {
    pci-vendor = 0x1002,
    pci-model = 0x164e,
}
gfx-api = "OpenGl"
```

You can also match directly without naming:

```toml
render-device = {
    pci-vendor = 0x1002,
    pci-model = 0x73ff,
}
```

> [!NOTE]
> Changing the render device at runtime (via the `set-render-device` action)
> may cause windows to become invisible until they are resized or otherwise
> redrawn.

## Runtime changes

### Listing GPUs

```shell
~$ jay randr
```

This shows all DRM devices with their PCI IDs, vendor/model names, current
API, and other settings.

### Changing settings at runtime

Use `jay randr` subcommands:

```shell
~$ jay randr card <card> api vulkan
~$ jay randr card <card> direct-scanout enable
~$ jay randr card <card> primary
```

### Using shortcuts

The `configure-drm-device` action applies settings from a keybinding:

```toml
[shortcuts]
alt-F5 = {
    type = "configure-drm-device",
    dev = {
        match.name = "dedicated",
        gfx-api = "Vulkan",
    },
}
alt-F6 = {
    type = "configure-drm-device",
    dev = {
        match.name = "dedicated",
        gfx-api = "OpenGl",
    },
}
```

The `set-render-device` action switches the compositing GPU:

```toml
[shortcuts]
alt-F7 = { type = "set-render-device", dev.name = "dedicated" }
alt-F8 = { type = "set-render-device", dev.name = "integrated" }
```

## Hardware cursor

The `use-hardware-cursor` top-level setting controls whether the hardware cursor
plane is used. Disabling this forces software cursor rendering, which can be
useful for debugging.

```toml
use-hardware-cursor = true  # default
```

## Full reference

For the exhaustive list of all DRM device fields, match criteria, and related
types, see the [auto-generated specification](https://github.com/mahkoh/jay/blob/master/toml-spec/spec/spec.generated.md).
