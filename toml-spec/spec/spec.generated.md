# Jay TOML Config

This document describes the format of the TOML configuration of the Jay compositor.

A JSON Schema for this format is available at [spec.generated.json](./spec.generated.json). You can include this file in your editor to get auto completion.

Start at the top-level type: [Config](#types-config).

## Types

<a name="types-AccelProfile"></a>
### `AccelProfile`

The acceleration profile to apply to an input device.

See the libinput documentation for more details.

Values of this type should be strings.

The string should have one of the following values:

- `Flat`:

  The flat profile.

- `Adaptive`:

  The adaptive profile.



<a name="types-Action"></a>
### `Action`

An `Action` is an action performed by the compositor.

- Example:

  ```toml
  [shortcuts]
  alt-q = "quit"
  ```

Values of this type should have one of the following forms:

#### A string

The value should be the name of a `simple` action. See the description of that
variant for more details.

- Example:

  ```toml
  [shortcuts]
  alt-q = "quit"
  ```

The value should be a [SimpleActionName](#types-SimpleActionName).

#### An array

A list of actions to execute in sequence.

- Example:

  ```toml
  [shortcuts]
  alt-q = [
    { type = "exec", exec = ["notify-send", "exiting"] },
    "quit",
  ]
  ```

Each element of this array should be a [Action](#types-Action).

#### A table



This table is a tagged union. The variant is determined by the `type` field. It takes one of the following values:

- `simple`:

  A simple action that takes no arguments. These are usually written as plain
  strings instead.
  
  - Example 1:
  
    ```toml
    [shortcuts]
    alt-q = { type = "simple", cmd = "quit" }
    ```
  
  - Example 2:
  
    ```toml
    [shortcuts]
    alt-q = "quit"
    ```

  The table has the following fields:

  - `cmd` (required):

    The simple action to execute.

    The value of this field should be a [SimpleActionName](#types-SimpleActionName).

- `multi`:

  A list of actions to execute in sequence. These are usually written as plain
  arrays instead.
  
  - Example 1:
  
    ```toml
    [shortcuts]
    alt-q = { type = "multi", actions = ["quit", "quit"] }
    ```
  
  - Example 2:
  
    ```toml
    [shortcuts]
    alt-q = ["quit", "quit"]
    ```

  The table has the following fields:

  - `actions` (required):

    The actions to execute.

    The value of this field should be an array of [Actions](#types-Action).

- `exec`:

  Executes a program.
  
  - Example:
  
    ```toml
    [shortcuts]
    ctrl-a = { type = "exec", exec = "alacritty" }
    ctrl-b = { type = "exec", exec = ["notify-send", "hello world"] }
    ```

  The table has the following fields:

  - `exec` (required):

    The command to execute.

    The value of this field should be a [Exec](#types-Exec).

- `switch-to-vt`:

  Switches to a virtual terminal.
  
  - Example:
  
    ```toml
    [shortcuts]
    ctrl-alt-F1 = { type = "switch-to-vt", num = 1 }
    ```

  The table has the following fields:

  - `num` (required):

    The VT number to switch to.

    The value of this field should be a number.

    The numbers should be integers.

    The numbers should be greater than or equal to 1.

- `show-workspace`:

  Switches to a workspace.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-F1 = { type = "show-workspace", name = "1" }
    ```

  The table has the following fields:

  - `name` (required):

    The name of the workspace.

    The value of this field should be a string.

- `move-to-workspace`:

  Moves the currently focused window to a workspace.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-F1 = { type = "move-to-workspace", name = "1" }
    ```

  The table has the following fields:

  - `name` (required):

    The name of the workspace.

    The value of this field should be a string.

- `move-to-output`:

  Moves a workspace to a different output.
  
  - Example 1:
  
    ```toml
    [shortcuts]
    alt-F1 = { type = "move-to-output", workspace = "1", output.name = "right" }
    ```
  
  - Example 2:
  
    ```toml
    [shortcuts]
    alt-F1 = { type = "move-to-output", output.name = "right" }
    ```

  The table has the following fields:

  - `workspace` (optional):

    The name of the workspace.
    
    If this is omitted, the currently active workspace is moved.

    The value of this field should be a string.

  - `output` (required):

    The output to move to.
    
    If multiple outputs match, the workspace is moved to the first matching
    output.

    The value of this field should be a [OutputMatch](#types-OutputMatch).

- `configure-connector`:

  Applies a configuration to connectors.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-j = { type = "configure-connector", connector = { match.name = "eDP-1", enabled = false } }
    alt-k = { type = "configure-connector", connector = { match.name = "eDP-1", enabled = true } }
    ```

  The table has the following fields:

  - `connector` (required):

    The connector configuration.

    The value of this field should be a [Connector](#types-Connector).

- `configure-input`:

  Applies a configuration to input devices.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-l = { type = "configure-input", input = { match.tag = "mouse", left-handed = true } }
    alt-r = { type = "configure-input", input = { match.tag = "mouse", left-handed = false } }
  
    [[inputs]]
    tag = "mouse"
    match.is-pointer = true
    ```

  The table has the following fields:

  - `input` (required):

    The input configuration.

    The value of this field should be a [Input](#types-Input).

- `configure-idle`:

  Configures the idle timeout.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-l = { type = "configure-idle", idle.minutes = 0 }
    alt-r = { type = "configure-idle", idle.minutes = 10 }
    ```

  The table has the following fields:

  - `idle` (required):

    The idle timeout.

    The value of this field should be a [Idle](#types-Idle).

- `configure-output`:

  Applies a configuration to input devices.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-l = { type = "configure-output", output = { match.name = "right", transform = "none" } }
    alt-r = { type = "configure-output", output = { match.name = "right", transform = "rotate-90" } }
  
    [[outputs]]
    name = "right"
    match.serial-number = "33K03894SL0"
    ```

  The table has the following fields:

  - `output` (required):

    The output configuration.

    The value of this field should be a [Output](#types-Output).

- `set-env`:

  Sets environment variables for all programs started afterwards.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-l = { type = "set-env", env.GTK_THEME = "Adwaita:dark" }
    ```

  The table has the following fields:

  - `env` (required):

    The environment variables.

    The value of this field should be a table whose values are strings.

- `unset-env`:

  Unsets environment variables for all programs started afterwards.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-l = { type = "unset-env", env = ["Adwaita:dark"] }
    ```

  The table has the following fields:

  - `env` (required):

    The environment variables.

    The value of this field should be an array of strings.

- `set-keymap`:

  Sets the keymap.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-j = { type = "set-keymap", keymap.name = "laptop" }
    alt-k = { type = "set-keymap", keymap.name = "external" }
  
    [[keymaps]]
    name = "laptop"
    path = "./laptop-keymap.xkb"
  
    [[keymaps]]
    name = "external"
    path = "./external-keymap.xkb"
    ```

  The table has the following fields:

  - `keymap` (required):

    The keymap.

    The value of this field should be a [Keymap](#types-Keymap).

- `set-repeat-rate`:

  Sets the keyboard repeat rate.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-x = { type = "set-repeat-rate", rate = { rate = 25, delay = 250 } }
    ```

  The table has the following fields:

  - `rate` (required):

    The rate.

    The value of this field should be a [RepeatRate](#types-RepeatRate).

- `set-status`:

  Sets the status command.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-j = { type = "set-status", status = { exec = "i3status" } }
    ```

  The table has the following fields:

  - `status` (optional):

    The status setting.
    
    Omitting this causes the status to be reset to empty.

    The value of this field should be a [Status](#types-Status).

- `set-theme`:

  Sets the theme.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-j = { type = "set-theme", theme.bg-color = "#ff0000" }
    ```

  The table has the following fields:

  - `theme` (required):

    The theme.

    The value of this field should be a [Theme](#types-Theme).

- `set-log-level`:

  Sets the log level of the compositor..
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-j = { type = "set-log-level", level = "debug" }
    ```

  The table has the following fields:

  - `theme` (required):

    The log level.

    The value of this field should be a [LogLevel](#types-LogLevel).

- `set-gfx-api`:

  Sets the graphics API used by new DRM devices.
  
  Setting this after the compositor has started usually has no effect.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-j = { type = "set-gfx-api", api = "Vulan" }
    ```

  The table has the following fields:

  - `api` (required):

    The API.

    The value of this field should be a [GfxApi](#types-GfxApi).

- `configure-direct-scanout`:

  Configure whether the compositor attempts direct scanout of client surfaces.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-j = { type = "configure-direct-scanout", enabled = false }
    ```

  The table has the following fields:

  - `enabled` (required):

    Whether direct scanout is enabled.

    The value of this field should be a boolean.

- `configure-drm-device`:

  Applies a configuration to DRM devices.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-j = { type = "configure-drm-device", dev = { match.name = "integrated", gfx-api = "Vulkan" } }
  
    [[drm-devices]]
    name = "integrated"
    match.syspath = "/sys/devices/pci0000:00/0000:00:08.1/0000:14:00.0"
    ```

  The table has the following fields:

  - `dev` (required):

    The DRM device configuration.

    The value of this field should be a [DrmDevice](#types-DrmDevice).

- `set-render-device`:

  Sets the render device used for compositing.
  
  Changing this after the compositor has started might cause client windows to
  become invisible until they are resized.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-j = { type = "set-render-device", dev.name = "integrated" }
  
    [[drm-devices]]
    name = "integrated"
    match.syspath = "/sys/devices/pci0000:00/0000:00:08.1/0000:14:00.0"
    ```

  The table has the following fields:

  - `dev` (required):

    The rule to find the device.
    
    The first matching device is used.

    The value of this field should be a [DrmDeviceMatch](#types-DrmDeviceMatch).


<a name="types-Color"></a>
### `Color`

A color.

The format should be one of the following:

- `#rgb`
- `#rrggbb`
- `#rgba`
- `#rrggbba`

Values of this type should be strings.


<a name="types-Config"></a>
### `Config`

This is the top-level table.

- Example:

  ```toml
  keymap = """
    xkb_keymap {
        xkb_keycodes { include "evdev+aliases(qwerty)" };
        xkb_types    { include "complete"              };
        xkb_compat   { include "complete"              };
        xkb_symbols  { include "pc+us+inet(evdev)"     };
    };
    """

  on-graphics-initialized = { type = "exec", exec = "mako" }

  [shortcuts]
  alt-h = "focus-left"
  alt-j = "focus-down"
  alt-k = "focus-up"
  alt-l = "focus-right"

  alt-shift-h = "move-left"
  alt-shift-j = "move-down"
  alt-shift-k = "move-up"
  alt-shift-l = "move-right"

  alt-d = "split-horizontal"
  alt-v = "split-vertical"

  alt-t = "toggle-split"
  alt-m = "toggle-mono"
  alt-u = "toggle-fullscreen"

  alt-f = "focus-parent"
  alt-shift-c = "close"
  alt-shift-f = "toggle-floating"
  Super_L = { type = "exec", exec = "alacritty" }
  alt-p = { type = "exec", exec = "bemenu-run" }
  alt-q = "quit"
  alt-shift-r = "reload-config-toml"

  ctrl-alt-F1 = { type = "switch-to-vt", num = 1 }
  ctrl-alt-F2 = { type = "switch-to-vt", num = 2 }
  # ...

  alt-F1 = { type = "show-workspace", name = "1" }
  alt-F2 = { type = "show-workspace", name = "2" }
  # ...

  alt-shift-F1 = { type = "move-to-workspace", name = "1" }
  alt-shift-F2 = { type = "move-to-workspace", name = "2" }
  # ...
  ```

Values of this type should be tables.

The table has the following fields:

- `keymap` (optional):

  The keymap to use.
  
  - Example:
  
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

  The value of this field should be a [Keymap](#types-Keymap).

- `repeat-rate` (optional):

  The keyboard repeat rate.
  
  - Example:
    
    ```toml
    repeat-rate = { rate = 25, delay = 250 }
    ```

  The value of this field should be a [RepeatRate](#types-RepeatRate).

- `shortcuts` (optional):

  The compositor shortcuts.
  
  The keys should be in the following format:
  
  ```
  (MOD-)*KEYSYM
  ```
  
  `MOD` should be one of `shift`, `lock`, `ctrl`, `mod1`, `mod2`, `mod3`, `mod4`,
  `mod5`, `caps`, `alt`, `num`, or `logo`.
  
  `KEYSYM` should be the name of a keysym. The authorative location for these names
  is [1] with the `XKB_KEY_` prefix removed.
  
  The keysym should be the unmodified keysym. E.g. `shift-q` not `shift-Q`.
  
  [1]: https://github.com/xkbcommon/libxkbcommon/blob/master/include/xkbcommon/xkbcommon-keysyms.h
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-q = "quit"
    ```

  The value of this field should be a table whose values are [Actions](#types-Action).

- `on-graphics-initialized` (optional):

  An action to execute when the graphics have been initialized for the first time.
  
  This is a good place to start graphical applications.
  
  - Example:
  
    ```toml
    on-graphics-initialized = { type = "exec", exec = "mako" }
    ```

  The value of this field should be a [Action](#types-Action).

- `status` (optional):

  The status program that will be used for the status text.
  
  - Example:
  
    ```toml
    [status]
    format = "i3bar"
    exec = "i3status"
    ```

  The value of this field should be a [Status](#types-Status).

- `outputs` (optional):

  An array of output configurations.
  
  This can be used to configure outputs and create named outputs that can be
  referred to in actions.
  
  The configurations defined here will only be applied the first time matching
  outputs are connected to the compositor after the compositor has started.
  If you want change the configuration afterwards, use `jay randr` or a
  `configure-output` action.
  
  - Example:
  
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

  The value of this field should be an array of [Outputs](#types-Output).

- `connectors` (optional):

  An array of connector configurations.
  
  This can be used to configure connectors.
  
  The configurations defined here will only be applied when the connector is first
  discovered by the compositor. This usually never happens after the compositor has
  started unless you attach an external graphics card.
  
  - Example:
  
    ```toml
    [[connectors]]
    name = "eDP-1"
    enabled = false
    ```

  The value of this field should be an array of [Connectors](#types-Connector).

- `workspace-capture` (optional):

  Configures whether newly created workspaces can be captured.
  
  The default is `true`.

  The value of this field should be a boolean.

- `env` (optional):

  Defines environment variables that will be set for all applications.
  
  - Example:
  
    ```toml
    [env]
    GTK_THEME = "Adwaita:dark"
    ```

  The value of this field should be a table whose values are strings.

- `on-startup` (optional):

  An action to execute as early as possible when the compositor starts.
  
  At this point, graphics have not yet been initialized. You should not use this
  to start graphical applications. See `on-graphics-initialized`.
  
  This setting has no effect during configuration reloads.

  The value of this field should be a [Action](#types-Action).

- `keymaps` (optional):

  Defines named keymaps.
  
  These keymaps can be used to easily switch between keymaps for different
  keyboards.
  
  - Example:
  
    ```toml
    keymap.name = "laptop"
  
    [shortcuts]
    alt-j = { type = "set-keymap", keymap.name = "laptop" }
    alt-k = { type = "set-keymap", keymap.name = "external" }
  
    [[keymaps]]
    name = "laptop"
    path = "./laptop-keymap.xkb"
  
    [[keymaps]]
    name = "external"
    path = "./external-keymap.xkb"
    ```

  The value of this field should be an array of [Keymaps](#types-Keymap).

- `log-level` (optional):

  Sets the log level of the compositor.
  
  This setting cannot be changed by re-loading the configuration. Use
  `jay set-log-level` instead.
  
  - Example:
  
    ```toml
    log-level = "debug"
    ```

  The value of this field should be a [LogLevel](#types-LogLevel).

- `theme` (optional):

  Sets the theme of the compositor.

  The value of this field should be a [Theme](#types-Theme).

- `gfx-api` (optional):

  Sets the graphics API used for newly discovered DRM devices.
  
  Changing this setting after the compositor has started usually has no effect
  unless you attach an external graphics card. Use `jay randr` to change the API
  used by individual devices at runtime.
  
  - Example:
  
    ```toml
    gfx-api = "Vulkan"
    ```

  The value of this field should be a [GfxApi](#types-GfxApi).

- `drm-devices` (optional):

  Names and configures DRM devices.
  
  These settings are only applied to devices discovered after the configuration
  has been loaded. Therefore changing these settings usually has no effect at
  runtime unless you attach an external graphics card. You can use `jay randr` or
  a `configure-drm-device` Action to change these settings at runtime.
  
  - Example:
  
    ```toml
    render-device.name = "dedicated"
  
    [[drm-devices]]
    name = "dedicated"
    match = { pci-vendor = 0x1002, pci-model = 0x73ff }
  
    [[drm-devices]]
    name = "integrated"
    match = { pci-vendor = 0x1002, pci-model = 0x164e }
    gfx-api = "OpenGl"
    ```

  The value of this field should be an array of [DrmDevices](#types-DrmDevice).

- `direct-scanout` (optional):

  Configured whether the compositor attempts direct scanout.

  The value of this field should be a boolean.

- `explicit-sync` (optional):

  Configures whether the compositor supports explicit sync.
  
  This cannot be changed after the compositor has started.
  
  The default is `true`.

  The value of this field should be a boolean.

- `render-device` (optional):

  Selects the device to use for rendering in a system with multiple GPUs.
  
  The first device that matches will be used.
  
  - Example:
  
    ```toml
    render-device.name = "dedicated"
  
    [[drm-devices]]
    name = "dedicated"
    match = { pci-vendor = 0x1002, pci-model = 0x73ff }
    ```

  The value of this field should be a [DrmDeviceMatch](#types-DrmDeviceMatch).

- `inputs` (optional):

  Names and configures input devices.
  
  These settings are only applied to devices connected after the configuration
  has been loaded. You can apply setting without re-connecting the device by using
  `jay input` or a `configure-input` Action.
  
  - Example:
  
    ```toml
    render-device.name = "dedicated"
  
    [[inputs]]
    match.is-pointer = true
    left-handed = true
    transform-matrix = [[0.35, 0], [0, 0.35]]
    tap-enabled = true
    ```

  The value of this field should be an array of [Inputs](#types-Input).

- `on-idle` (optional):

  An action to execute when the compositor becomes idle.
  
  - Example:
  
    ```toml
    on-idle = { type = "exec", exec = "lock" }
    ```

  The value of this field should be a [Action](#types-Action).

- `idle` (optional):

  The configuration of the idle timeout.
  
  Changing thise field after compositor startup has no effect. Use `jay idle`
  or a `configure-idle` action to change the idle timeout at runtime.
  
  - Example:
  
    ```toml
    idle.minutes = 10
    ```

  The value of this field should be a [Idle](#types-Idle).


<a name="types-Connector"></a>
### `Connector`

Describes configuration to apply to a connector.

- Example: To disable the built-in display of a laptop:

  ```toml
  [[connectors]]
  match.name = "eDP-1"
  enabled = false
  ```

Values of this type should be tables.

The table has the following fields:

- `match` (required):

  The rule by which the connectors to modify are selected.

  The value of this field should be a [ConnectorMatch](#types-ConnectorMatch).

- `enabled` (optional):

  If specified, enables or disables the connector.

  The value of this field should be a boolean.


<a name="types-ConnectorMatch"></a>
### `ConnectorMatch`

Rules to match one of the connectors used by the compositor.

Values of this type should have one of the following forms:

#### An array

This rule matches if any of the rules in the array match.

Each element of this array should be a [ConnectorMatch](#types-ConnectorMatch).

#### A table

Describes a rule that matches a subset of connectors.

This rule matches if all of the specified fields match.

- Example:

  ```toml
  [[connectors]]
  match.name = "DP-1"
  ```

The table has the following fields:

- `name` (optional):

  The name of the connector.
  
  These values are not necessarily stable. You can find out the value by running
  `jay randr`.

  The value of this field should be a string.


<a name="types-DrmDevice"></a>
### `DrmDevice`

Describes configuration to apply to a DRM device (graphics card).

- Example: To disable direct scanout on a device:

  ```toml
  [[drm-devices]]
  match = { pci-vendor = 0x1002, pci-model = 0x73ff }
  direct-scanout = false
  ```

Values of this type should be tables.

The table has the following fields:

- `name` (optional):

  Assigns a name to the rule in the `match` field.
  
  This only has an effect when used in the top-level `drm-devices` array.

  The value of this field should be a string.

- `match` (required):

  The rule by which the DRM devices to modify are selected.

  The value of this field should be a [DrmDeviceMatch](#types-DrmDeviceMatch).

- `direct-scanout` (optional):

  If specified, enables or disables direct scanout on this device.

  The value of this field should be a boolean.

- `gfx-api` (optional):

  If specified, sets the graphics API to use for this device.

  The value of this field should be a [GfxApi](#types-GfxApi).


<a name="types-DrmDeviceMatch"></a>
### `DrmDeviceMatch`

Rules to match one of the DRM devices (graphics cards) used by the compositor.

Values of this type should have one of the following forms:

#### An array

This rule matches if any of the rules in the array match.

Each element of this array should be a [DrmDeviceMatch](#types-DrmDeviceMatch).

#### A table

Describes a rule that matches a subset of DRM devices.

This rule matches if all of the specified fields match.

- Example:

  ```toml
  [[drm-devices]]
  name = "dedicated"
  match = { pci-vendor = 0x1002, pci-model = 0x73ff }
  ```

The table has the following fields:

- `name` (optional):

  The name of another DrmDeviceMatch rule.
  
  For this rule to match, the referenced rule must match. The name of the rule
  should have been defined in the top-level `drm-devices` array.
  
  This can be used to easily refer to DRM devices.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-v = { type = "configure-drm-device", dev = { match.name = "dedicated", gfx-api = "Vulkan" } }
    alt-o = { type = "configure-drm-device", dev = { match.name = "dedicated", gfx-api = "OpenGl" } }
  
    [[drm-devices]]
    name = "dedicated"
    match = { pci-vendor = 0x1002, pci-model = 0x73ff }
    ```

  The value of this field should be a string.

- `syspath` (optional):

  The syspath of the device.
  
  This is useful if you have multiple copies of the same device installed so that
  the PCI numbers are not unique.
  
  The values are usually stable unless you re-configure your hardware.
  
  - Example:
  
    ```toml
    [[drm-devices]]
    name = "integrated"
    match.syspath = "/sys/devices/pci0000:00/0000:00:08.1/0000:14:00.0"
    ```

  The value of this field should be a string.

- `devnode` (optional):

  The devnode of the device.
  
  The values are usually not-stable across PC restarts.
  
  - Example:
  
    ```toml
    [[drm-devices]]
    name = "integrated"
    match.devnode = "/dev/dri/card0"
    ```

  The value of this field should be a string.

- `vendor` (optional):

  The name of the vendor.
  
  - Example:
  
    ```toml
    [[drm-devices]]
    name = "integrated"
    match.vendor = "Advanced Micro Devices, Inc. [AMD/ATI]"
    ```

  The value of this field should be a string.

- `model` (optional):

  The name of the model.
  
  - Example:
  
    ```toml
    [[drm-devices]]
    name = "integrated"
    match.vendor = "Raphael"
    ```

  The value of this field should be a string.

- `pci-vendor` (optional):

  The PCI number of the vendor.
  
  - Example:
  
    ```toml
    [[drm-devices]]
    name = "integrated"
    match.pci-vendor = 0x1002
    ```

  The value of this field should be a number.

  The numbers should be integers.

- `pci-model` (optional):

  The PCI number of the model.
  
  - Example:
  
    ```toml
    [[drm-devices]]
    name = "integrated"
    match.pci-model = 0x164e
    ```

  The value of this field should be a number.

  The numbers should be integers.


<a name="types-Exec"></a>
### `Exec`

Describes how to execute a program.

- Example 1:

  ```toml
  [shortcuts]
  ctrl-a = { type = "exec", exec = "alacritty" }
  ```

- Example 2:

  ```toml
  [shortcuts]
  ctrl-a = { type = "exec", exec = ["notify-send", "hello world"] }
  ```

- Example 3:

  ```toml
  [shortcuts]
  ctrl-a = { type = "exec", exec = { prog = "notify-send", args = ["hello world"], env.WAYLAND_DISPLAY = "2" } }
  ```

Values of this type should have one of the following forms:

#### A string

The name of the executable to execute.

- Example:

  ```toml
  [shortcuts]
  ctrl-a = { type = "exec", exec = "alacritty" }
  ```

#### An array

The name and arguments of the executable to execute.

- Example:

  ```toml
  [shortcuts]
  ctrl-a = { type = "exec", exec = ["notify-send", "hello world"] }
  ```

Each element of this array should be a string.

#### A table

The name, arguments, and environment variables of the executable to execute.

- Example:

  ```toml
  [shortcuts]
  ctrl-a = { type = "exec", exec = { prog = "notify-send", args = ["hello world"], env.WAYLAND_DISPLAY = "2" } }
  ```

The table has the following fields:

- `prog` (required):

  The name of the executable.

  The value of this field should be a string.

- `args` (optional):

  The arguments to pass to the executable.

  The value of this field should be an array of strings.

- `env` (optional):

  The environment variables to pass to the executable.

  The value of this field should be a table whose values are strings.

- `privileged` (optional):

  If `true`, the executable gets access to privileged wayland protocols.
  
  The default is `false`.

  The value of this field should be a boolean.


<a name="types-GfxApi"></a>
### `GfxApi`

A graphics API used for rendering.

Values of this type should be strings.

The string should have one of the following values:

- `OpenGl`:

  The OpenGL API.

- `Vulkan`:

  The Vulkan API.
  
  Note that this API has the following restriction: If any of the DRM devices in
  the system use Vulkan, then all devices must support DRM format modifiers. This
  is usually the case but not for AMD devices older than RX 5xxx.



<a name="types-Idle"></a>
### `Idle`

The definition of an idle timeout.

Omitted values are set to 0. If all values are 0, the idle timeout is disabled.

- Example:

  ```toml
  idle.minutes = 10
  ```

Values of this type should be tables.

The table has the following fields:

- `minutes` (optional):

  The number of minutes before going idle.

  The value of this field should be a number.

  The numbers should be integers.

  The numbers should be greater than or equal to 0.

- `seconds` (optional):

  The number of seconds before going idle.

  The value of this field should be a number.

  The numbers should be integers.

  The numbers should be greater than or equal to 0.


<a name="types-Input"></a>
### `Input`

Describes configuration to apply to an input device.

- Example: To make mice left handed:

  ```toml
  [[inputs]]
  match.is-pointer = true
  left-handed = true
  ```

Values of this type should be tables.

The table has the following fields:

- `tag` (optional):

  Assigns a name to the rule in the `match` field.
  
  This only has an effect when used in the top-level `inputs` array.

  The value of this field should be a string.

- `match` (required):

  The rule by which the input devices to modify are selected.

  The value of this field should be a [InputMatch](#types-InputMatch).

- `accel-profile` (optional):

  The acceleration profile to use.
  
  See the libinput documentation for more details.

  The value of this field should be a [AccelProfile](#types-AccelProfile).

- `accel-speed` (optional):

  The acceleration speed to use.
  
  Values should be in the range -1 to 1.
  
  See the libinput documentation for more details.

  The value of this field should be a number.

- `tap-enabled` (optional):

  Whether tap is enabled for this device.
  
  See the libinput documentation for more details.

  The value of this field should be a boolean.

- `tap-drag-enabled` (optional):

  Whether tap drag is enabled for this device.
  
  See the libinput documentation for more details.

  The value of this field should be a boolean.

- `tap-drag-lock-enabled` (optional):

  Whether tap drag lock is enabled for this device.
  
  See the libinput documentation for more details.

  The value of this field should be a boolean.

- `left-handed` (optional):

  Whether the device is left handed.
  
  See the libinput documentation for more details.

  The value of this field should be a boolean.

- `natural-scrolling` (optional):

  Whether the device uses natural scrolling.
  
  See the libinput documentation for more details.

  The value of this field should be a boolean.

- `px-per-wheel-scroll` (optional):

  The number of pixels to scroll for each scroll wheel dedent.

  The value of this field should be a boolean.

- `transform-matrix` (optional):

  A transformation matrix to apply to each motion event of this device.
  The matrix should be 2x2.
  
  - Example: To slow down the mouse to 35% of normal speed:
  
    ```toml
    [[inputs]]
    match.is-pointer = true
    transform-matrix = [[0.35, 0], [0, 0.35]]
    ```

  The value of this field should be an array of arrays of numbers.


<a name="types-InputMatch"></a>
### `InputMatch`

Rules to match one of the input devices used by the compositor.

Values of this type should have one of the following forms:

#### An array

This rule matches if any of the rules in the array match.

Each element of this array should be a [InputMatch](#types-InputMatch).

#### A table

Describes a rule that matches a subset of input devices.

This rule matches if all of the specified fields match.

- Example:

  ```toml
  [[inputs]]
  match.is-pointer = true
  left-handed = true
  ```

The table has the following fields:

- `tag` (optional):

  The tag of another InputMatch rule.
  
  For this rule to match, the referenced rule must match. The name of the rule
  should have been defined in the top-level `inputs` array.
  
  This can be used to easily refer to input devices.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-l = { type = "configure-input", input = { match.tag = "mouse", left-handed = true } }
    alt-r = { type = "configure-input", input = { match.tag = "mouse", left-handed = false } }
  
    [[inputs]]
    tag = "mouse"
    match.is-pointer = true
    ```

  The value of this field should be a string.

- `name` (optional):

  The libinput name of the device.
  
  You can find out the name of the devices by running `jay input`.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.name = "Logitech G300s Optical Gaming Mouse"
    left-handed = true
    ```

  The value of this field should be a string.

- `syspath` (optional):

  The syspath of the device.
  
  This is useful if you have multiple copies of the same device installed so that
  the name is not unique.
  
  The values are usually stable unless you re-configure your hardware.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.syspath = "/sys/devices/pci0000:00/0000:00:08.1/0000:14:00.4/usb5/5-1/5-1.1/5-1.1.2/5-1.1.2:1.0"
    left-handed = true
    ```

  The value of this field should be a string.

- `devnode` (optional):

  The devnode of the device.
  
  The values are usually not-stable across PC restarts.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.devnode = "/dev/input/event4"
    left-handed = true
    ```

  The value of this field should be a string.

- `is-keyboard` (optional):

  Whether the devices has been identified as a keyboard.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.is-keyboard = false
    left-handed = true
    ```

  The value of this field should be a boolean.

- `is-pointer` (optional):

  Whether the devices has been identified as a pointer.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.is-pointer = false
    left-handed = true
    ```

  The value of this field should be a boolean.

- `is-touch` (optional):

  Whether the devices has been identified as a touch device.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.is-touch = true
    tap-enabled = true
    ```

  The value of this field should be a boolean.

- `is-tablet-tool` (optional):

  Whether the devices has been identified as a tablet tool.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.is-tablet-tool = true
    tap-enabled = true
    ```

  The value of this field should be a boolean.

- `is-tablet-pad` (optional):

  Whether the devices has been identified as a tablet pad.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.is-tablet-tool = true
    tap-enabled = true
    ```

  The value of this field should be a boolean.

- `is-gesture` (optional):

  Whether the devices has been identified as a switch.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.is-switch = true
    ```

  The value of this field should be a boolean.


<a name="types-Keymap"></a>
### `Keymap`

A keymap.

Values of this type should have one of the following forms:

#### A string

Defines a keymap by its XKB representation.

- Example:

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

#### A table

Defines or references a keymap.

- Example:

  ```toml
  keymap.name = "my-keymap"

  [[keymaps]]
  name = "my-keymap"
  path = "./my-keymap.xkb"
  ```

The table has the following fields:

- `name` (optional):

  Defines a keymap name or references a defined keymap.
  
  If the value is set in the top-level `keymaps` array, it defines a named
  keymap.
  
  Otherwise it references a named keymap that should have been defined in the
  `keymaps` array.

  The value of this field should be a string.

- `map` (optional):

  Defines a keymap by its XKB representation.
  
  For each keymap defined in the top-level `keymaps` array, exactly one of `map`
  and `path` has to be defined.

  The value of this field should be a string.

- `path` (optional):

  Loads a keymap's XKB representation from a file.
  
  If the path is relative, it will be interpreted relative to the Jay config
  directory.
  
  For each keymap defined in the top-level `keymaps` array, exactly one of `map`
  and `path` has to be defined.

  The value of this field should be a string.


<a name="types-LogLevel"></a>
### `LogLevel`

A log level.

Values of this type should be strings.

The string should have one of the following values:

- `trace`:

  Trace log level.

- `debug`:

  Debug log level.

- `info`:

  Info log level.

- `warn`:

  Warn log level.

- `error`:

  Error log level.



<a name="types-MessageFormat"></a>
### `MessageFormat`

A message format used by status programs.

Values of this type should be strings.

The string should have one of the following values:

- `plain`:

  The messages are in plain text.

- `pango`:

  The messages contain pango markup.

- `i3bar`:

  The messages are in i3bar format.



<a name="types-Mode"></a>
### `Mode`

The mode of a display.

- Example:

  ```toml
  [[outputs]]
  match.serial-number = "33K03894SL0"
  mode = { width = 1920, height = 1080, refresh-rate = 59.94 }
  ```

Values of this type should be tables.

The table has the following fields:

- `width` (required):

  The width of the mode.

  The value of this field should be a number.

  The numbers should be integers.

- `height` (required):

  The height of the mode.

  The value of this field should be a number.

  The numbers should be integers.

- `refresh-rate` (optional):

  The refresh rate of the mode in HZ.

  The value of this field should be a number.


<a name="types-Output"></a>
### `Output`

Describes configuration to apply to an output.

- Example: To set the scale of an output.

  ```toml
  [[outputs]]
  match.serial-number = "33K03894SL0"
  scale = 1.25
  ```

Values of this type should be tables.

The table has the following fields:

- `name` (optional):

  Assigns a name to the rule in the `match` field.
  
  This only has an effect when used in the top-level `outputs` array.

  The value of this field should be a string.

- `match` (required):

  The rule by which the outputs to modify are selected.

  The value of this field should be a [OutputMatch](#types-OutputMatch).

- `x` (optional):

  The x coordinate of the output in compositor space.

  The value of this field should be a number.

  The numbers should be integers.

  The numbers should be greater than or equal to 0.

- `y` (optional):

  The y coordinate of the output in compositor space.

  The value of this field should be a number.

  The numbers should be integers.

  The numbers should be greater than or equal to 0.

- `scale` (optional):

  The scale of the output.

  The value of this field should be a number.

  The numbers should be strictly greater than 0.

- `transform` (optional):

  The transform of the output.

  The value of this field should be a [Transform](#types-Transform).

- `mode` (optional):

  The mode of the output.
  
  If the refresh rate is not specified, the first mode with the specified width and
  height is used.

  The value of this field should be a [Mode](#types-Mode).


<a name="types-OutputMatch"></a>
### `OutputMatch`

Rules to match one of the outputs used by the compositor.

Values of this type should have one of the following forms:

#### An array

This rule matches if any of the rules in the array match.

Each element of this array should be a [OutputMatch](#types-OutputMatch).

#### A table

Describes a rule that matches a subset of outputs.

This rule matches if all of the specified fields match.

- Example:

  ```toml
  [[outputs]]
  name = "right"
  match.serial-number = "33K03894SL0"
  x = 1920
  y = 0
  ```

The table has the following fields:

- `name` (optional):

  The name of another OutputMatch rule.
  
  For this rule to match, the referenced rule must match. The name of the rule
  should have been defined in the top-level `outputs` array.
  
  This can be used to easily refer to outputs.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-l = { type = "configure-output", output = { match.name = "right", transform = "none" } }
    alt-r = { type = "configure-output", output = { match.name = "right", transform = "rotate-90" } }
  
    [[outputs]]
    name = "right"
    match.serial-number = "33K03894SL0"
    ```

  The value of this field should be a string.

- `connector` (optional):

  The name of the connector the output is connected to.
  
  You can find out the name of the connector by running `jay randr`.
  
  - Example:
  
    ```toml
    [[outputs]]
    match.connector = "DP-1"
    scale = 1.25
    ```

  The value of this field should be a string.

- `serial-number` (optional):

  The serial number of the output.
  
  You can find out the serial number by running `jay randr`.
  
  - Example:
  
    ```toml
    [[outputs]]
    match.serial-number = "33K03894SL0"
    scale = 1.25
    ```

  The value of this field should be a string.

- `manufacturer` (optional):

  The manufacturer of the output.
  
  You can find out the manufacturer by running `jay randr`.
  
  - Example:
  
    ```toml
    [[outputs]]
    match.manufacturer = "BNQ"
    scale = 1.25
    ```

  The value of this field should be a string.

- `model` (optional):

  The model of the output.
  
  You can find out the model by running `jay randr`.
  
  - Example:
  
    ```toml
    [[outputs]]
    match.model = "BenQ GW2480"
    scale = 1.25
    ```

  The value of this field should be a string.


<a name="types-RepeatRate"></a>
### `RepeatRate`

Describes a keyboard repeat rate.

- Example:

  ```toml
  repeat-rate = { rate = 25, delay = 250 }
  ```

Values of this type should be tables.

The table has the following fields:

- `rate` (required):

  The number of times to repeat per second.

  The value of this field should be a number.

  The numbers should be integers.

- `delay` (required):

  The number of milliseconds after a key is pressed before repeating begins.

  The value of this field should be a number.

  The numbers should be integers.


<a name="types-SimpleActionName"></a>
### `SimpleActionName`

The name of a `simple` Action.

- Example:

  ```toml
  [shortcuts]
  alt-q = "quit"
  ```

Values of this type should be strings.

The string should have one of the following values:

- `focus-left`:

  Move the keyboard focus to the left of the currently focused window.

- `focus-down`:

  Move the keyboard focus down from the currently focused window.

- `focus-up`:

  Move the keyboard focus up from the currently focused window.

- `focus-right`:

  Move the keyboard focus to the right of the currently focused window.

- `move-left`:

  Move the currently focused window one to the left.

- `move-down`:

  Move the currently focused window one down.

- `move-up`:

  Move the currently focused window one up.

- `move-right`:

  Move the currently focused window one to the right.

- `move-right`:

  Move the currently focused window one to the right.

- `split-horizontal`:

  Split the currently focused window horizontally.

- `split-vertical`:

  Split the currently focused window vertically.

- `toggle-split`:

  Toggle the split of the currently focused container between vertical and
  horizontal.

- `toggle-mono`:

  Toggle the currently focused container between showing a single and all children.

- `toggle-fullscreen`:

  Toggle the currently focused window between fullscreen and windowed.

- `focus-parent`:

  Focus the parent of the currently focused window.

- `close`:

  Close the currently focused window.

- `disable-pointer-constraint`:

  Disable the currently active pointer constraint, allowing you to move the pointer
  outside the window.
  
  The constraint will be re-enabled when the pointer re-enters the window.

- `toggle-floating`:

  Toggle the currently focused window between floating and tiled.

- `quit`:

  Terminate the compositor.

- `reload-config-toml`:

  Reload the `config.toml`.

- `reload-config-to`:

  Reload the `config.so`.

- `none`:

  Perform no action.
  
  As a special case, if this is the action of a shortcut, the shortcut will be
  unbound. This can be used in modes to unbind a key.



<a name="types-Status"></a>
### `Status`

The configuration of a status program whose output will be shown in the bar.

- Example:

  ```toml
  [status]
  format = "i3bar"
  exec = "i3status"
  ```

Values of this type should be tables.

The table has the following fields:

- `format` (optional):

  The format used by the program.

  The value of this field should be a [MessageFormat](#types-MessageFormat).

- `exec` (required):

  The program that will emit the status messages.

  The value of this field should be a [Exec](#types-Exec).

- `i3bar-separator` (optional):

  The separator to be used between i3bar components.
  
  The default is ` | `.

  The value of this field should be a string.


<a name="types-Theme"></a>
### `Theme`

The theme of the compositor.

Values of this type should be tables.

The table has the following fields:

- `attention-requested-bg-color` (optional):

  The background color of title that have requested attention.

  The value of this field should be a [Color](#types-Color).

- `bg-color` (optional):

  The background color of the desktop.

  The value of this field should be a [Color](#types-Color).

- `bar-bg-color` (optional):

  The background color of the bar.

  The value of this field should be a [Color](#types-Color).

- `bar-status-text-color` (optional):

  The color of the status text in the bar.

  The value of this field should be a [Color](#types-Color).

- `border-color` (optional):

  The color of the borders between windows.

  The value of this field should be a [Color](#types-Color).

- `captured-focused-title-bg-color` (optional):

  The background color of focused titles that are being recorded.

  The value of this field should be a [Color](#types-Color).

- `captured-unfocused-title-bg-color` (optional):

  The background color of unfocused titles that are being recorded.

  The value of this field should be a [Color](#types-Color).

- `focused-inactive-title-bg-color` (optional):

  The background color of focused titles that are inactive.

  The value of this field should be a [Color](#types-Color).

- `focused-inactive-title-text-color` (optional):

  The text color of focused titles that are inactive.

  The value of this field should be a [Color](#types-Color).

- `focused-title-bg-color` (optional):

  The background color of focused titles.

  The value of this field should be a [Color](#types-Color).

- `focused-title-text-color` (optional):

  The text color of focused titles.

  The value of this field should be a [Color](#types-Color).

- `separator-color` (optional):

  The color of the separator between titles and window content.

  The value of this field should be a [Color](#types-Color).

- `unfocused-title-bg-color` (optional):

  The background color of unfocused titles.

  The value of this field should be a [Color](#types-Color).

- `unfocused-title-text-color` (optional):

  The text color of unfocused titles.

  The value of this field should be a [Color](#types-Color).

- `border-width` (optional):

  The width of borders between windows.

  The value of this field should be a number.

  The numbers should be integers.

  The numbers should be greater than or equal to 0.

- `title-height` (optional):

  The height of tabs.

  The value of this field should be a number.

  The numbers should be integers.

  The numbers should be greater than or equal to 0.

- `font` (optional):

  The name of the font to use.

  The value of this field should be a string.


<a name="types-Transform"></a>
### `Transform`

An output transformation.

Values of this type should be strings.

The string should have one of the following values:

- `none`:

  No transformation.

- `rotate-90`:

  The content of the output is rotated 90 degrees counter clockwise.

- `rotate-180`:

  The content of the output is rotated 180 degrees counter clockwise.

- `rotate-270`:

  The content of the output is rotated 270 degrees counter clockwise.

- `flip`:

  The content of the output is flipped around the vertical axis.

- `flip-rotate-90`:

  The content of the output is flipped around the vertical axis and then rotated
  90 degrees counter clockwise.

- `flip-rotate-180`:

  The content of the output is flipped around the vertical axis and then rotated
  180 degrees counter clockwise.

- `flip-rotate-270`:

  The content of the output is flipped around the vertical axis and then rotated
  270 degrees counter clockwise.



