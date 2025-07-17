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

#### A string

The value should be the name of a `named` action, prefixed with the `$` character.

This is the same as using the `named` action with the `$` removed.

- Example:

  ```toml
  [actions]
  q = "quit"

  [shortcuts]
  alt-q = "$q"
  ```

The string should match the following regular expression: `^\$.*$`

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

- `named`:

  A named action that was defined via the top-level `actions` table or a
  `define-action` action. These are usually written as plain strings with a `$`
  prefix.
  
  - Example 1:
  
    ```toml
    [actions]
    my-action = "quit"
  
    [shortcuts]
    alt-q = { type = "named", name = "my-action" }
    ```
  
  - Example 2:
  
    ```toml
    [shortcuts]
    alt-q = [
      { type = "define-action", name = "my-action", action = "quit" },
      { type = "named", name = "my-action" },
    ]
    ```

  The table has the following fields:

  - `name` (required):

    The named action to execute.

    The value of this field should be a string.

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

- `define-action`:

  Defines a name for an action. Usually you would define these by using the
  top-level `actions` table. This action can be used to re-define actions.
  
  - Example:
  
    ```toml
    [actions]
    a1 = "quit"
    a2 = "$a1"
  
    [shortcuts]
    alt-q = [
      { type = "define-action", name = "a2", action = [] },
      "$2", # does nothing
    ]
    ```

  The table has the following fields:

  - `name` (required):

    The name of the action.

    The value of this field should be a string.

  - `action` (required):

    The action to execute.

    The value of this field should be a [Action](#types-Action).

- `undefine-action`:

  Removes a named action.

  The table has the following fields:

  - `name` (required):

    The name of the action.

    The value of this field should be a string.


<a name="types-Brightness"></a>
### `Brightness`

The brightness setting of an output.

Values of this type should have one of the following forms:

#### A string

The default brightness setting.

The string should have one of the following values:

- `default`:

  The default brightness setting.       
  
  The behavior depends on the transfer function:
  
  - `default`: The maximum brightness of the output.
  - `PQ`: 203 cd/m^2


#### A number

The brightness in cd/m^2.


<a name="types-ClickMethod"></a>
### `ClickMethod`

The click method to apply to an input device.

See the libinput documentation for more details.

Values of this type should be strings.

The string should have one of the following values:

- `none`:

  No click method handling.

- `button-areas`:

  Bottom area of the touchpad is divided into a left, middle and right button area.

- `clickfinger`:

  Number of fingers on the touchpad decide the button type.
  Clicking with 1, 2, 3 fingers triggers a left, right, or middle click, respectively.



<a name="types-ClientMatch"></a>
### `ClientMatch`

Criteria for matching clients.

If no fields are set, all clients are matched. If multiple fields are set, all fields
must match the client.

Values of this type should be tables.

The table has the following fields:

- `name` (optional):

  Matches if the client rule with this name matches.
  
  - Example:
  
    ```toml
    [[clients]]
    name = "spotify"
    match.sandbox-app-id = "com.spotify.Client"
  
    # Matches the same clients as the previous rule.
    [[clients]]
    match.name = "spotify"
    ```

  The value of this field should be a string.

- `not` (optional):

  Matches if the contained criteria don't match.
  
  - Example:
  
    ```toml
    [[clients]]
    name = "not-spotify"
    match.not.sandbox-app-id = "com.spotify.Client"
    ```

  The value of this field should be a [ClientMatch](#types-ClientMatch).

- `all` (optional):

  Matches if all of the contained criteria match.
  
  - Example:
  
    ```toml
    [[clients]]
    match.all = [
      { sandbox-app-id = "com.spotify.Client" },
      { sandbox-engine = "org.flatpak" },
    ]
    ```

  The value of this field should be an array of [ClientMatchs](#types-ClientMatch).

- `any` (optional):

  Matches if any of the contained criteria match.
  
  - Example:
  
    ```toml
    [[clients]]
    match.any = [
      { sandbox-app-id = "com.spotify.Client" },
      { sandbox-app-id = "com.valvesoftware.Steam" },
    ]
    ```

  The value of this field should be an array of [ClientMatchs](#types-ClientMatch).

- `exactly` (optional):

  Matches if a specific number of contained criteria match.
  
  - Example:
  
    ```toml
    # Matches any client that is either steam or sandboxed by flatpak but not both.
    [[clients]]
    match.exactly.num = 1
    match.exactly.list = [
      { sandbox-engine = "org.flatpak" },
      { sandbox-app-id = "com.valvesoftware.Steam" },
    ]
    ```

  The value of this field should be a [ClientMatchExactly](#types-ClientMatchExactly).

- `sandboxed` (optional):

  Matches if the client is/isn't sandboxed.
  
  - Example:
  
    ```toml
    [[clients]]
    match.sandboxed = true
    ```

  The value of this field should be a boolean.

- `sandbox-engine` (optional):

  Matches the engine name of the client's sandbox verbatim.
  
  - Example:
  
    ```toml
    [[clients]]
    match.sandbox-engine = "org.flatpak"
    ```

  The value of this field should be a string.

- `sandbox-engine-regex` (optional):

  Matches the engine name of the client's sandbox with a regular expression.
  
  - Example:
  
    ```toml
    [[clients]]
    match.sandbox-engine = "flatpak"
    ```

  The value of this field should be a string.

- `sandbox-app-id` (optional):

  Matches the app id of the client's sandbox verbatim.
  
  - Example:
  
    ```toml
    [[clients]]
    match.sandbox-app-id = "com.spotify.Client"
    ```

  The value of this field should be a string.

- `sandbox-app-id-regex` (optional):

  Matches the app id of the client's sandbox with a regular expression.
  
  - Example:
  
    ```toml
    [[clients]]
    match.sandbox-app-id-regex = "(?i)spotify"
    ```

  The value of this field should be a string.

- `sandbox-instance-id` (optional):

  Matches the instance id of the client's sandbox verbatim.

  The value of this field should be a string.

- `sandbox-instance-id-regex` (optional):

  Matches the instance id of the client's sandbox with a regular expression.

  The value of this field should be a string.

- `uid` (optional):

  Matches the user ID of the client.

  The value of this field should be a number.

  The numbers should be integers.

- `pid` (optional):

  Matches the process ID of the client.

  The value of this field should be a number.

  The numbers should be integers.

- `is-xwayland` (optional):

  Matches if the client is/isn't Xwayland.

  The value of this field should be a boolean.

- `comm` (optional):

  Matches the `/proc/pid/comm` of the client verbatim.

  The value of this field should be a string.

- `comm-regex` (optional):

  Matches the `/proc/pid/comm` of the client with a regular expression.

  The value of this field should be a string.

- `exe` (optional):

  Matches the `/proc/pid/exe` of the client verbatim.

  The value of this field should be a string.

- `exe-regex` (optional):

  Matches the `/proc/pid/exe` of the client with a regular expression.

  The value of this field should be a string.


<a name="types-ClientMatchExactly"></a>
### `ClientMatchExactly`

Criterion for matching a specific number of client criteria.

Values of this type should be tables.

The table has the following fields:

- `num` (required):

  The number of criteria that must match.

  The value of this field should be a number.

- `list` (required):

  The list of criteria.

  The value of this field should be an array of [ClientMatchs](#types-ClientMatch).


<a name="types-ClientRule"></a>
### `ClientRule`

A client rule.

Values of this type should be tables.

The table has the following fields:

- `name` (optional):

  The name of this rule.
  
  This name can be referenced in other rules.
  
  - Example
  
    ```toml
    [[clients]]
    name = "spotify"
    match.sandbox-app-id = "com.spotify.Client"
  
    [[clients]]
    match.name = "spotify"
    action = "kill-client"
    ```

  The value of this field should be a string.

- `match` (optional):

  The criteria that select the client that this rule applies to.

  The value of this field should be a [ClientMatch](#types-ClientMatch).

- `action` (optional):

  An action to execute when a client matches the criteria.

  The value of this field should be a [Action](#types-Action).

- `latch` (optional):

  An action to execute when a client no longer matches the criteria.

  The value of this field should be a [Action](#types-Action).


<a name="types-Color"></a>
### `Color`

A color.

The format should be one of the following:

- `#rgb`
- `#rrggbb`
- `#rgba`
- `#rrggbba`

Values of this type should be strings.


<a name="types-ColorManagement"></a>
### `ColorManagement`

Describes color-management settings.

- Example:

  ```toml
  [color-management]
  enabled = true
  ```

Values of this type should be tables.

The table has the following fields:

- `enabled` (optional):

  Whether the color management protocol is enabled.
  
  This has no effect on running applications.
  
  The default is `false`.

  The value of this field should be a boolean.


<a name="types-ColorSpace"></a>
### `ColorSpace`

The color space of an output.

Values of this type should be strings.

The string should have one of the following values:

- `default`:

  The default color space (usually sRGB).

- `bt2020`:

  The BT.2020 color space.



<a name="types-ComplexShortcut"></a>
### `ComplexShortcut`

Describes a complex shortcut.

- Example:

  ```toml
  [complex-shortcuts.XF86AudioRaiseVolume]
  mod-mask = "alt"
  action = { type = "exec", exec = ["pactl", "set-sink-volume", "0", "+10%"] }
  ```

Values of this type should be tables.

The table has the following fields:

- `mod-mask` (optional):

  The mod mask to apply to this shortcut.
  
  Should be a string containing modifiers concatenated by `-`. See the description
  of `Config.shortcuts` for more details.
  
  If this field is omitted, all modifiers are included in the mask.
  
  - Example:
    
    To raise the volume whenever the XF86AudioRaiseVolume key is pressed regardless
    of any modifiers except `alt`:
  
    ```toml
    [complex-shortcuts.XF86AudioRaiseVolume]
    mod-mask = "alt"
    action = { type = "exec", exec = ["pactl", "set-sink-volume", "0", "+10%"] }
    ```
  
    Set `mod-mask = ""` to ignore all modifiers.

  The value of this field should be a string.

- `action` (optional):

  The action to execute.
  
  Omitting this is the same as setting it to `"none"`.

  The value of this field should be a [Action](#types-Action).

- `latch` (optional):

  An action to execute when the key is released.
  
  This registers an action to be executed when the key triggering the shortcut is
  released. The active modifiers are ignored for this purpose.
  
  - Example:
  
    To mute audio while the key is pressed:
  
    ```toml
    [complex-shortcuts.alt-x]
    action = { type = "exec", exec = ["pactl", "set-sink-mute", "0", "1"] }
    latch = { type = "exec", exec = ["pactl", "set-sink-mute", "0", "0"] }
    ```
  
    Audio will be un-muted once `x` key is released, regardless of any other keys
    that are pressed at the time.

  The value of this field should be a [Action](#types-Action).


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
  `mod5`, `caps`, `alt`, `num`, `logo`, or `release`.
  
  Using the `release` modifier causes the shortcut to trigger when the key is
  released.
  
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

- `complex-shortcuts` (optional):

  Complex compositor shortcuts.
  
  The keys should have the same format as in the `shortcuts` table.
  
  - Example:
  
    ```toml
    [complex-shortcuts.XF86AudioRaiseVolume]
    mod-mask = "alt"
    action = { type = "exec", exec = ["pactl", "set-sink-volume", "0", "+10%"] }
    ```

  The value of this field should be a table whose values are [ComplexShortcuts](#types-ComplexShortcut).

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

- `focus-follows-mouse` (optional):

  Configures whether moving the mouse over a window automatically moves the keyboard
  focus to that window.
  
  The default is `true`.

  The value of this field should be a boolean.

- `window-management-key` (optional):

  Configures a key that will enable window management mode while pressed.
  
  In window management mode, floating windows can be moved by pressing the left
  mouse button and all windows can be resize by pressing the right mouse button.
  
  - Example:
  
    ```toml
    window-management-key = "Alt_L"
    ```

  The value of this field should be a string.

- `vrr` (optional):

  Configures the default VRR settings.
  
  This can be overwritten for individual outputs.
  
  By default, the VRR mode is `never` and the cursor refresh rate is unbounded.
  
  - Example:
    
    ```toml
    vrr = { mode = "always", cursor-hz = 90 }
    ```

  The value of this field should be a [Vrr](#types-Vrr).

- `tearing` (optional):

  Configures the default tearing settings.
  
  This can be overwritten for individual outputs.
  
  By default, the tearing mode is `variant3`.
  
  - Example:
  
    ```toml
    tearing.mode = "never"
    ```

  The value of this field should be a [Tearing](#types-Tearing).

- `libei` (optional):

  Configures the libei settings.
  
  - Example:
  
    ```toml
    libei.enable-socket = true
    ```

  The value of this field should be a [Libei](#types-Libei).

- `ui-drag` (optional):

  Configures the ui-drag settings.
  
  - Example:
  
    ```toml
    ui-drag = { enabled = false, threshold = 20 }
    ```

  The value of this field should be a [UiDrag](#types-UiDrag).

- `xwayland` (optional):

  Configures the Xwayland settings.
  
  - Example:
  
    ```toml
    xwayland = { scaling-mode = "downscaled" }
    ```

  The value of this field should be a [Xwayland](#types-Xwayland).

- `color-management` (optional):

  Configures the color-management settings.
  
  - Example:
  
    ```toml
    [color-management]
    enabled = true
    ```

  The value of this field should be a [ColorManagement](#types-ColorManagement).

- `float` (optional):

  Configures the settings of floating windows.
  
  - Example:
  
    ```toml
    [float]
    show-pin-icon = true
    ```

  The value of this field should be a [Float](#types-Float).

- `actions` (optional):

  Named actions.
  
  Named actions can be used everywhere an action can be used. This can be used to
  avoid repeating the same action multiple times.
  
  - Example:
  
    ```toml
    actions.switch-to-1 = [
      { type = "show-workspace", name = "1" },
      { type = "define-action", name = "switch-to-next", action = "$switch-to-2" },
    ]
      actions.switch-to-2 = [
      { type = "show-workspace", name = "2" },
      { type = "define-action", name = "switch-to-next", action = "$switch-to-3" },
    ]
      actions.switch-to-3 = [
      { type = "show-workspace", name = "3" },
      { type = "define-action", name = "switch-to-next", action = "$switch-to-1" },
    ]
    actions.switch-to-next = "$switch-to-1"
  
    [shortcuts]
    alt-x = "$switch-to-next"
    ```

  The value of this field should be a table whose values are [Actions](#types-Action).

- `max-action-depth` (optional):

  The maximum call depth of named actions. This setting prevents infinite recursion
  when using named actions. Setting this value to 0 or less disables named actions
  completely. The default is `16`.

  The value of this field should be a number.

  The numbers should be integers.

  The numbers should be greater than or equal to 0.

- `clients` (optional):

  An array of client rules.
  
  These rules can be used to give names to clients and to manipulate them.
  
  - Example:
  
    ```toml
    [[clients]]
    name = "spotify"
    match.sandbox-app-id = "com.spotify.Client"
    ```

  The value of this field should be an array of [ClientRules](#types-ClientRule).

- `windows` (optional):

  An array of window rules.
  
  These rules can be used to give names to windows and to manipulate them.
  
  - Example:
  
    ```toml
    [[windows]]
    name = "spotify"
    match.title-regex = "Spotify"
    action = { type = "move-to-workspace", name = "music" }
    ```

  The value of this field should be an array of [WindowRules](#types-WindowRule).

- `pointer-revert-key` (optional):

  Sets the keysym that can be used to revert the pointer to the default state.
  
  Pressing this key cancels any grabs, drags, selections, etc.
  
  The default is `Escape`. Setting this to `NoSymbol` effectively disables
  this functionality.
  
  The value of the string should be the name of a keysym. The authoritative location
  for these names is [1] with the `XKB_KEY_` prefix removed.
  
  [1]: https://github.com/xkbcommon/libxkbcommon/blob/master/include/xkbcommon/xkbcommon-keysyms.h
  
  - Example:
  
    ```toml
    pointer-revert-key = "NoSymbol"
    ```

  The value of this field should be a string.

- `use-hardware-cursor` (optional):

  Configures whether the default seat uses hardware cursors.
  
  The default is `true`.

  The value of this field should be a boolean.


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


<a name="types-ContentTypeMask"></a>
### `ContentTypeMask`

A mask of content types.

Values of this type should have one of the following forms:

#### A string

A named mask.

The string should have one of the following values:

- `none`:

  The mask matching windows without a content type.

- `any`:

  The mask containing every possible type except `none`.

- `photo`:

  The mask matching photo content.

- `video`:

  The mask matching video content.

- `game`:

  The mask matching game content.


#### An array

An array of masks that are OR'd.

Each element of this array should be a [ContentTypeMask](#types-ContentTypeMask).


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

- `flip-margin-ms` (optional):

  If specified, sets the flip margin of this device.
  
  This is duration between the compositor initiating a page flip and the output's
  vblank event. This determines the minimum input latency. The default is 1.5 ms.
  
  Note that if the margin is too small, the compositor will dynamically increase it.

  The value of this field should be a number.


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


<a name="types-Float"></a>
### `Float`

Describes settings of floating windows.

- Example:

  ```toml
  [float]
  show-pin-icon = true
  ```

Values of this type should be tables.

The table has the following fields:

- `show-pin-icon` (optional):

  Sets whether floating windows always show a pin icon.
  
  The default is `false`.

  The value of this field should be a boolean.


<a name="types-Format"></a>
### `Format`

A graphics format.

These formats are documented in https://github.com/torvalds/linux/blob/master/include/uapi/drm/drm_fourcc.h

- Example:

  ```toml
  [[outputs]]
  match.serial-number = "33K03894SL0"
  format = "rgb565"
  ```

Values of this type should be strings.

The string should have one of the following values:

- `argb8888`:


- `xrgb8888`:


- `abgr8888`:


- `xbgr8888`:


- `r8`:


- `gr88`:


- `rgb888`:


- `bgr888`:


- `rgba4444`:


- `rgbx4444`:


- `bgra4444`:


- `bgrx4444`:


- `rgb565`:


- `bgr565`:


- `rgba5551`:


- `rgbx5551`:


- `bgra5551`:


- `bgrx5551`:


- `argb1555`:


- `xrgb1555`:


- `argb2101010`:


- `xrgb2101010`:


- `abgr2101010`:


- `xbgr2101010`:


- `abgr16161616`:


- `xbgr16161616`:


- `abgr16161616f`:


- `xbgr16161616f`:




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



<a name="types-GracePeriod"></a>
### `GracePeriod`

The definition of a grace period.

Omitted values are set to 0. If all values are 0, the grace period is disabled.

- Example:

  ```toml
  idle.grace-period.seconds = 3
  ```

Values of this type should be tables.

The table has the following fields:

- `minutes` (optional):

  The number of minutes the grace period lasts.

  The value of this field should be a number.

  The numbers should be integers.

  The numbers should be greater than or equal to 0.

- `seconds` (optional):

  The number of seconds the grace period lasts.

  The value of this field should be a number.

  The numbers should be integers.

  The numbers should be greater than or equal to 0.


<a name="types-Idle"></a>
### `Idle`

The definition of an idle timeout.

Omitted values are set to 0. If any value is explicitly set and all values are 0, the
idle timeout is disabled.

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

- `grace-period` (optional):

  The grace period after the timeout expires.
  
  During the grace period, the screen goes black but the outputs are not yet
  disabled and the `on-idle` action does not yet run. This is a visual indicator
  that the system will soon get idle.
  
  The default is 5 seconds.

  The value of this field should be a [GracePeriod](#types-GracePeriod).


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

- `middle-button-emulation` (optional):

  Converts a simultaneous left and right button click into a middle button click.
  
  See the libinput documentation for more details.

  The value of this field should be a boolean.

- `click-method` (optional):

  Defines how button events are triggered on a clickable touchpad.
  
  See the libinput documentation for more details.

  The value of this field should be a [ClickMethod](#types-ClickMethod).

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

- `keymap` (optional):

  The keymap to use for this device.
  
  This overrides the global keymap. The keymap becomes active when a key is pressed.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.name = "ZSA Technology Labs Inc ErgoDox EZ"
    keymap.name = "external"
    ```

  The value of this field should be a [Keymap](#types-Keymap).

- `on-lid-closed` (optional):

  An action to execute when the laptop lid is closed.
  
  This should only be used in the top-level inputs array.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.name = "<switch name>"
    on-lid-closed = { type = "configure-connector", connector = { match.name = "eDP-1", enabled = false } }
    on-lid-opened = { type = "configure-connector", connector = { match.name = "eDP-1", enabled = true } }
    ```

  The value of this field should be a [Action](#types-Action).

- `on-lid-opened` (optional):

  An action to execute when the laptop lid is opened.
  
  This should only be used in the top-level inputs array.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.name = "<switch name>"
    on-lid-closed = { type = "configure-connector", connector = { match.name = "eDP-1", enabled = false } }
    on-lid-opened = { type = "configure-connector", connector = { match.name = "eDP-1", enabled = true } }
    ```

  The value of this field should be a [Action](#types-Action).

- `on-converted-to-laptop` (optional):

  An action to execute when the convertible device is converted to a laptop.
  
  This should only be used in the top-level inputs array.

  The value of this field should be a [Action](#types-Action).

- `on-converted-to-tablet` (optional):

  An action to execute when the convertible device is converted to a tablet.
  
  This should only be used in the top-level inputs array.

  The value of this field should be a [Action](#types-Action).

- `output` (optional):

  Maps this input device to an output.
  
  This is used to map touch screen and graphics tablets to outputs.
  
  - Example:
  
    ```toml
    [[inputs]]
    match.name = "Wacom Bamboo Comic 2FG Pen"
    output.connector = "DP-1"
    ```

  The value of this field should be a [OutputMatch](#types-OutputMatch).

- `remove-mapping` (optional):

  Removes the mapping of from this device to an output.
  
  This should only be used within `configure-input` actions.
  
  - Example:
  
    ```toml
    [shortcuts]
    alt-x = { type = "configure-input", input = { match.tag = "wacom", remove-mapping = true } }
  
    [[inputs]]
    tag = "wacom"
    match.name = "Wacom Bamboo Comic 2FG Pen"
    output.connector = "DP-1"
    ```

  The value of this field should be a boolean.

- `calibration-matrix` (optional):

  The calibration matrix of the device. This matrix should be 2x3.
  
  See the libinput documentation for more details.
  
  - Example: To flip the device 90 degrees:
  
    ```toml
    [[inputs]]
    calibration-matrix = [[0, 1, 0], [-1, 0, 1]]
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


<a name="types-Libei"></a>
### `Libei`

Describes libei settings.

- Example:

  ```toml
  libei.enable-socket = "true"
  ```

Values of this type should be tables.

The table has the following fields:

- `enable-socket` (optional):

  Enables or disables the unauthenticated libei socket.
  
  Even if the socket is disabled, application can still request access via the portal.
  
  The default is `false`.

  The value of this field should be a boolean.


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

- `vrr` (optional):

  Configures the VRR settings of this output.
  
  By default, the VRR mode is `never` and the cursor refresh rate is unbounded.
  
  - Example:
  
    ```toml
    [[outputs]]
    match.serial-number = "33K03894SL0"
    vrr = { mode = "always", cursor-hz = 90 }
    ```

  The value of this field should be a [Vrr](#types-Vrr).

- `tearing` (optional):

  Configures the tearing settings of this output.
  
  By default, the tearing mode is `variant3`.
  
  - Example:
  
    ```toml
    [[outputs]]
    match.serial-number = "33K03894SL0"
    tearing.mode = "never"
    ```

  The value of this field should be a [Tearing](#types-Tearing).

- `format` (optional):

  Configures the framebuffer format of this output.
  
  By default, the format is `xrgb8888`.
  
  - Example:
  
    ```toml
    [[outputs]]
    match.serial-number = "33K03894SL0"
    format = "rgb565"
    ```

  The value of this field should be a [Format](#types-Format).

- `color-space` (optional):

  The color space of the output.

  The value of this field should be a [ColorSpace](#types-ColorSpace).

- `transfer-function` (optional):

  The transfer function of the output.

  The value of this field should be a [TransferFunction](#types-TransferFunction).

- `brightness` (optional):

  The brightness of the output.
  
  This setting has no effect unless the vulkan renderer is used.

  The value of this field should be a [Brightness](#types-Brightness).


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

When used inside a window rule, the following actions apply to the matched window
instead fo the focused window:

- `move-left`
- `move-down`
- `move-up`
- `move-right`
- `split-horizontal`
- `split-vertical`
- `toggle-split`
- `tile-horizontal`
- `tile-vertical`
- `toggle-split`
- `show-single`
- `show-all`
- `toggle-fullscreen`
- `enter-fullscreen`
- `exit-fullscreen`
- `close`
- `toggle-floating`
- `float`
- `tile`
- `toggle-float-pinned`
- `pin-float`
- `unpin-float`


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

- `tile-horizontal`:

  Sets the split of the currently focused container to horizontal.

- `tile-vertical`:

  Sets the split of the currently focused container to vertical.

- `toggle-mono`:

  Toggle the currently focused container between showing a single and all children.

- `show-single`:

  Makes the currently focused container show a single child.

- `show-all`:

  Makes the currently focused container show all children.

- `toggle-fullscreen`:

  Toggle the currently focused window between fullscreen and windowed.

- `enter-fullscreen`:

  Makes the currently focused window fullscreen.

- `exit-fullscreen`:

  Makes the currently focused window windowed.

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

- `float`:

  Makes the currently focused window floating.

- `tile`:

  Makes the currently focused window tiled.

- `quit`:

  Terminate the compositor.

- `reload-config-toml`:

  Reload the `config.toml`.

- `reload-config-so`:

  Reload the `config.so`.

- `consume`:

  Consume the current key event. Don't forward it to the focused application.
  
  This action only has an effect in shortcuts.
  
  Key-press events events that trigger shortcuts are consumed by default.
  Key-release events events that trigger shortcuts are forwarded by default.
  
  Note that consuming key-release events can cause keys to get stuck in the focused
  application.
  
  See the `forward` action to achieve the opposite effect.

- `forward`:

  Forward the current key event to the focused application.
  
  See the `consume` action for more details.

- `none`:

  Perform no action.
  
  As a special case, if this is the action of a shortcut, the shortcut will be
  unbound. This can be used in modes to unbind a key.

- `enable-window-management`:

  Enables window management mode.
  
  In window management mode, floating windows can be moved by pressing the left
  mouse button and all windows can be resize by pressing the right mouse button.

- `disable-window-management`:

  Disables window management mode.

- `enable-float-above-fullscreen`:

  Enables floating windows showing above fullscreen windows.
  
  By default, floating windows are hidden below fullscreen windows.

- `disable-float-above-fullscreen`:

  Disables floating windows showing above fullscreen windows.

- `toggle-float-above-fullscreen`:

  Toggles floating windows showing above fullscreen windows.

- `pin-float`:

  Pins the currently focused floating window.
  
  If a floating window is pinned, it will stay visible even when switching to a
  different workspace.

- `unpin-float`:

  Unpins the currently focused floating window.

- `toggle-float-pinned`:

  Toggles whether the currently focused floating window is pinned.

- `kill-client`:

  Kills a client.
  
  Within a window rule, it applies to the client of the window. Within a client rule
  it applies to the matched client. Has no effect otherwise.



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


<a name="types-Tearing"></a>
### `Tearing`

Describes tearing settings.

- Example:

  ```toml
  tearing.mode = "never"
  ```

Values of this type should be tables.

The table has the following fields:

- `mode` (optional):

  The tearing mode.

  The value of this field should be a [TearingMode](#types-TearingMode).


<a name="types-TearingMode"></a>
### `TearingMode`

The tearing mode of an output.

- Example:

  ```toml
  tearing.mode = "never"
  ```

Values of this type should be strings.

The string should have one of the following values:

- `always`:

  Tearing is never enabled.

- `never`:

  Tearing is always enabled.

- `variant1`:

  Tearing is enabled when one or more applications are displayed fullscreen.

- `variant2`:

  Tearing is enabled when a single application is displayed fullscreen.

- `variant3`:

  Tearing is enabled when a single application is displayed and the application has
  requested tearing.



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

- `highlight-color` (optional):

  Color used to highlight parts of the UI.

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


<a name="types-TileState"></a>
### `TileState`

Whether a window is tiled or floating.

Values of this type should be strings.

The string should have one of the following values:

- `tiled`:

  The window is tiled.

- `floating`:

  The window is floating.



<a name="types-TransferFunction"></a>
### `TransferFunction`

The transfer function of an output.

Values of this type should be strings.

The string should have one of the following values:

- `default`:

  The default transfer function (usually sRGB).

- `pq`:

  The PQ transfer function.



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



<a name="types-UiDrag"></a>
### `UiDrag`

Describes ui-drag settings.

- Example:

  ```toml
  ui-drag = { enabled = false, threshold = 20 }
  ```

Values of this type should be tables.

The table has the following fields:

- `enabled` (optional):

  Enables or disables dragging of tiles and workspaces.
  
  The default is `true`.

  The value of this field should be a boolean.

- `threshold` (optional):

  Sets the distance at which ui dragging starts.
  
  The default is `10`.

  The value of this field should be a number.

  The numbers should be integers.


<a name="types-Vrr"></a>
### `Vrr`

Describes VRR settings.

- Example:

  ```toml
  vrr = { mode = "always", cursor-hz = 90 }
  ```

Values of this type should be tables.

The table has the following fields:

- `mode` (optional):

  The VRR mode.

  The value of this field should be a [VrrMode](#types-VrrMode).

- `cursor-hz` (optional):

  The VRR cursor refresh rate.
  
  Limits the rate at which cursors are updated on screen when VRR is active.

  The value of this field should be a [VrrHz](#types-VrrHz).


<a name="types-VrrHz"></a>
### `VrrHz`

A VRR refresh rate limiter.

- Example 1:

  ```toml
  vrr = { cursor-hz = 90 }
  ```

- Example 2:

  ```toml
  vrr = { cursor-hz = "none" }
  ```

Values of this type should have one of the following forms:

#### A string

The string `none` can be used to disable the limiter.

#### A number

The refresh rate in HZ.


<a name="types-VrrMode"></a>
### `VrrMode`

The VRR mode of an output.

- Example:

  ```toml
  vrr = { mode = "always", cursor-hz = 90 }
  ```

Values of this type should be strings.

The string should have one of the following values:

- `always`:

  VRR is never enabled.

- `never`:

  VRR is always enabled.

- `variant1`:

  VRR is enabled when one or more applications are displayed fullscreen.

- `variant2`:

  VRR is enabled when a single application is displayed fullscreen.

- `variant3`:

  VRR is enabled when a single game or video is displayed fullscreen.



<a name="types-WindowMatch"></a>
### `WindowMatch`

Criteria for matching windows.

If no fields are set, all windows are matched. If multiple fields are set, all fields
must match the window.

Values of this type should be tables.

The table has the following fields:

- `name` (optional):

  Matches if the window rule with this name matches.
  
  - Example:
  
    ```toml
    [[windows]]
    name = "spotify"
    match.title-regex = "Spotify"
  
    # Matches the same windows as the previous rule.
    [[windows]]
    match.name = "spotify"
    ```

  The value of this field should be a string.

- `not` (optional):

  Matches if the contained criteria don't match.
  
  - Example:
  
    ```toml
    [[windows]]
    name = "not-spotify"
    match.not.title-regex = "Spotify"
    ```

  The value of this field should be a [WindowMatch](#types-WindowMatch).

- `all` (optional):

  Matches if all of the contained criteria match.
  
  - Example:
  
    ```toml
    [[windows]]
    match.all = [
      { title-regex = "Spotify" },
      { title-regex = "Premium" },
    ]
    ```

  The value of this field should be an array of [WindowMatchs](#types-WindowMatch).

- `any` (optional):

  Matches if any of the contained criteria match.
  
  - Example:
  
    ```toml
    [[windows]]
    match.any = [
      { title-regex = "Spotify" },
      { title-regex = "Alacritty" },
    ]
    ```

  The value of this field should be an array of [WindowMatchs](#types-WindowMatch).

- `exactly` (optional):

  Matches if a specific number of contained criteria match.
  
  - Example:
  
    ```toml
    # Matches any window that is either Alacritty or on workspace 3 but not both.
    [[windows]]
    match.exactly.num = 1
    match.exactly.list = [
      { workspace = "3" },
      { title-regex = "Alacritty" },
    ]
    ```

  The value of this field should be a [WindowMatchExactly](#types-WindowMatchExactly).

- `types` (optional):

  Matches windows whose type is contained in the mask.

  The value of this field should be a [WindowTypeMask](#types-WindowTypeMask).

- `client` (optional):

  Matches if the window's client matches the client criterion.

  The value of this field should be a [ClientMatch](#types-ClientMatch).

- `title` (optional):

  Matches the title of the window verbatim.

  The value of this field should be a string.

- `title-regex` (optional):

  Matches the title of the window with a regular expression.

  The value of this field should be a string.

- `app-id` (optional):

  Matches the app-id of the window verbatim.

  The value of this field should be a string.

- `app-id-regex` (optional):

  Matches the app-id of the window with a regular expression.

  The value of this field should be a string.

- `floating` (optional):

  Matches if the window is/isn't floating.

  The value of this field should be a boolean.

- `visible` (optional):

  Matches if the window is/isn't visible.

  The value of this field should be a boolean.

- `urgent` (optional):

  Matches if the window has/hasn't the urgency flag set.

  The value of this field should be a boolean.

- `focused` (optional):

  Matches if the window has/hasn't the keyboard focus.

  The value of this field should be a boolean.

- `fullscreen` (optional):

  Matches if the window is/isn't fullscreen.

  The value of this field should be a boolean.

- `just-mapped` (optional):

  Matches if the window has/hasn't just been mapped.
  
  This is true for one iteration of the compositor's main loop immediately after the
  window has been mapped.

  The value of this field should be a boolean.

- `tag` (optional):

  Matches the toplevel-tag of the window verbatim.

  The value of this field should be a string.

- `tag-regex` (optional):

  Matches the toplevel-tag of the window with a regular expression.

  The value of this field should be a string.

- `x-class` (optional):

  Matches the X class of the window verbatim.

  The value of this field should be a string.

- `x-class-regex` (optional):

  Matches the X class of the window with a regular expression.

  The value of this field should be a string.

- `x-instance` (optional):

  Matches the X instance of the window verbatim.

  The value of this field should be a string.

- `x-instance-regex` (optional):

  Matches the X instance of the window with a regular expression.

  The value of this field should be a string.

- `x-role` (optional):

  Matches the X role of the window verbatim.

  The value of this field should be a string.

- `x-role-regex` (optional):

  Matches the X role of the window with a regular expression.

  The value of this field should be a string.

- `workspace` (optional):

  Matches the workspace of the window verbatim.

  The value of this field should be a string.

- `workspace-regex` (optional):

  Matches the workspace of the window with a regular expression.

  The value of this field should be a string.

- `content-types` (optional):

  Matches windows whose content type is contained in the mask.

  The value of this field should be a [ContentTypeMask](#types-ContentTypeMask).


<a name="types-WindowMatchExactly"></a>
### `WindowMatchExactly`

Criterion for matching a specific number of window criteria.

Values of this type should be tables.

The table has the following fields:

- `num` (required):

  The number of criteria that must match.

  The value of this field should be a number.

- `list` (required):

  The list of criteria.

  The value of this field should be an array of [WindowMatchs](#types-WindowMatch).


<a name="types-WindowRule"></a>
### `WindowRule`

A window rule.

Values of this type should be tables.

The table has the following fields:

- `name` (optional):

  The name of this rule.
  
  This name can be referenced in other rules.
  
  - Example
  
    ```toml
    [[windows]]
    name = "spotify"
    match.title-regex = "Spotify"
  
    [[windows]]
    match.name = "spotify"
    action = "enter-fullscreen"
    ```

  The value of this field should be a string.

- `match` (optional):

  The criteria that select the window that this rule applies to.

  The value of this field should be a [WindowMatch](#types-WindowMatch).

- `action` (optional):

  An action to execute when a window matches the criteria.

  The value of this field should be a [Action](#types-Action).

- `latch` (optional):

  An action to execute when a window no longer matches the criteria.

  The value of this field should be a [Action](#types-Action).

- `auto-focus` (optional):

  Whether newly mapped windows that match this rule get the keyboard focus.
  
  If a window matches any rule for which this is false, the window will not be
  automatically focused.

  The value of this field should be a boolean.

- `initial-tile-state` (optional):

  Specifies if the window is initially mapped tiled or floating.

  The value of this field should be a [TileState](#types-TileState).


<a name="types-WindowTypeMask"></a>
### `WindowTypeMask`

A mask of window types.

Values of this type should have one of the following forms:

#### A string

A named mask.

The string should have one of the following values:

- `none`:

  The empty mask.

- `any`:

  The mask containing every possible type.

- `container`:

  The mask matching a container.

- `xdg-toplevel`:

  The mask matching an XDG toplevel.

- `x-window`:

  The mask matching an X window.

- `client-window`:

  The mask matching any type of client window.


#### An array

An array of masks that are OR'd.

Each element of this array should be a [WindowTypeMask](#types-WindowTypeMask).


<a name="types-XScalingMode"></a>
### `XScalingMode`

The scaling mode of X windows.

- Example:

  ```toml
  xwayland = { scaling-mode = "downscaled" }
  ```

Values of this type should be strings.

The string should have one of the following values:

- `default`:

  The default mode.
  
  Currently this means that windows are rendered at the lowest scale and then upscaled
  if necessary.

- `downscaled`:

  Windows are rendered at the highest integer scale and then downscaled.
  
  This has significant performance implications unless the window is running on the
  output with the highest scale and that scale is an integer scale.
  
  For example, on a 3840x2160 output with a 1.5 scale, a fullscreen window will be
  rendered at 3840x2160 * 2 / 1.5 = 5120x2880 pixels and then downscaled to
  3840x2160. This overhead gets worse the lower the scale of the output is.
  
  Additionally, this mode requires the X window to scale its contents itself. In the
  example above, you might achieve this by setting the environment variable
  `GDK_SCALE=2`.



<a name="types-Xwayland"></a>
### `Xwayland`

Describes Xwayland settings.

- Example:

  ```toml
  xwayland = { scaling-mode = "downscaled" }
  ```

Values of this type should be tables.

The table has the following fields:

- `scaling-mode` (optional):

  The scaling mode of X windows.

  The value of this field should be a [XScalingMode](#types-XScalingMode).


