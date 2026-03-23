# Command-Line Interface

Jay provides a comprehensive CLI for controlling the compositor, managing
displays, input devices, clients, and more. All subcommands communicate with
the running compositor over the Wayland protocol unless otherwise noted.

> [!TIP]
> Generate shell completions for tab-completion support:
>
> ```shell
> ~$ jay generate-completion bash > ~/.local/share/bash-completion/completions/jay
> ~$ jay generate-completion zsh > ~/.zfunc/_jay
> ~$ jay generate-completion fish > ~/.config/fish/completions/jay.fish
> ~$ jay generate-completion elvish  # pipe to appropriate location
> ~$ jay generate-completion powershell  # pipe to appropriate location
> ```

Every subcommand accepts a global `--log-level` option (`trace`, `debug`,
`info`, `warn`, `error`, `off`) that controls the verbosity of the CLI tool itself.

---

## JSON Output

Most query and status commands can output machine-readable JSON instead of
human-readable text. Pass the global `--json` flag before the subcommand:

```shell
~$ jay --json randr
~$ jay --json clients
~$ jay --json idle
```

Each command prints one or more JSON objects, one per line (JSONL format). This
makes it easy to process with tools like `jq`:

```shell
~$ jay --json randr | jq '.drm_devices[].connectors[].name'
~$ jay --json clients | jq 'select(.pid != null) | .pid'
```

By default, fields that are empty arrays, `null`, or `false` are omitted from
the output to reduce noise. To include every field, pass `--all-json-fields`:

```shell
~$ jay --all-json-fields --json randr
```

### Supported Commands

The following commands support `--json`:

`jay clients`
: One JSON object per client.

`jay color-management status`
: Color management enabled/available status.

`jay config path`
: The config file path as a JSON string.

`jay idle status`
: Idle interval, grace period, and inhibitors.

`jay input show`, `jay input seat <seat> show`, `jay input device <id> show`
: Seats and input devices with all properties.

`jay log --path`
: The log file path as a JSON string.

`jay pid`
: The compositor PID as a JSON number.

`jay randr show`
: DRM devices, connectors, outputs, modes, and display properties.

`jay seat-test`
: Streaming JSONL -- one JSON object per input event (key, pointer, touch,
  gesture, tablet, switch).

`jay tree query`
: One JSON object per root node, with children nested recursively.

`jay version`
: The version string as a JSON value.

`jay xwayland status`
: Xwayland scaling mode and implied scale.

> [!TIP]
> Mutating commands (e.g., `jay idle set`, `jay randr output ... enable`)
> produce no output, so `--json` has no effect on them.

---

## Running

### `jay run`

Start the compositor.

```shell
~$ jay run
```

Optionally specify which backends to try (comma-separated, tried in order):

```shell
~$ jay run --backends x11,metal,headless
```

The default order is `x11,metal`. The first backend that can be started is
used. `metal` is the native DRM/KMS backend for bare-metal sessions.

### `jay quit`

Stop the running compositor.

```shell
~$ jay quit
```

### `jay pid`

Print the PID of the running compositor.

```shell
~$ jay pid
```

### `jay version`

Print the Jay version.

```shell
~$ jay version
```

---

## Configuration

### `jay config init`

Generate a configuration file pre-populated with all defaults.

```shell
~$ jay config init
```

If a config already exists, pass `--overwrite` to replace it (the old file is
backed up):

```shell
~$ jay config init --overwrite
```

> [!IMPORTANT]
> Once a config file exists, the entire built-in default configuration is
> replaced -- not merged. An empty config file means no shortcuts, no startup
> actions, nothing. Always use `jay config init` to start with a working
> configuration.

### `jay config path`

Print the path to the config file.

```shell
~$ jay config path
```

### `jay config open-dir`

Open the configuration directory with `xdg-open`.

```shell
~$ jay config open-dir
```

---

## Display & Graphics

All display and GPU commands live under `jay randr`.

### Showing Current Settings

```shell
~$ jay randr
~$ jay randr show
~$ jay randr show --modes      # include all available modes
~$ jay randr show --formats    # include all available framebuffer formats
```

### GPU / Card Commands

Set the primary (render) GPU:

```shell
~$ jay randr card card0 primary
```

Set the graphics API:

```shell
~$ jay randr card card0 api vulkan
~$ jay randr card card0 api opengl
```

Toggle direct scanout:

```shell
~$ jay randr card card0 direct-scanout enable
~$ jay randr card card0 direct-scanout disable
```

Adjust the page-flip margin (in milliseconds, default 1.5):

```shell
~$ jay randr card card0 timing set-flip-margin 1.5
```

### Output Commands

Enable or disable an output:

```shell
~$ jay randr output DP-1 enable
~$ jay randr output DP-1 disable
```

Set the position:

```shell
~$ jay randr output DP-1 position 1920 0
```

Set the scale (fractional scaling supported):

```shell
~$ jay randr output DP-1 scale 1.5
~$ jay randr output DP-1 scale --round-to-float 1.333
```

Set the mode (width, height, refresh rate in Hz):

```shell
~$ jay randr output DP-1 mode 2560 1440 144.0
```

Set the transform:

```shell
~$ jay randr output DP-1 transform rotate-90
~$ jay randr output DP-1 transform none
```

Available transforms: `none`, `rotate-90`, `rotate-180`, `rotate-270`, `flip`,
`flip-rotate-90`, `flip-rotate-180`, `flip-rotate-270`.

Override the non-desktop setting:

```shell
~$ jay randr output DP-1 non-desktop true
~$ jay randr output DP-1 non-desktop default
```

Configure VRR (variable refresh rate):

```shell
~$ jay randr output DP-1 vrr set-mode always
~$ jay randr output DP-1 vrr set-mode variant3
~$ jay randr output DP-1 vrr set-cursor-hz 240
~$ jay randr output DP-1 vrr set-cursor-hz none
```

VRR modes: `never`, `always`, `variant1` (one or more fullscreen apps),
`variant2` (single fullscreen app), `variant3` (single fullscreen app with
game/video content type). Pass `none` to `set-cursor-hz` to remove the cursor
Hz cap.

Configure tearing:

```shell
~$ jay randr output DP-1 tearing set-mode variant3
```

Tearing modes: `never`, `always`, `variant1` (one or more applications
displayed fullscreen), `variant2` (single application displayed fullscreen),
`variant3` (default -- single displayed application that requested tearing).

Set the framebuffer format:

```shell
~$ jay randr output DP-1 format set xrgb8888
```

Set the output color space and EOTF (for HDR):

```shell
~$ jay randr output DP-1 colors set bt2020 pq
~$ jay randr output DP-1 colors set default default
```

Set the brightness (in cd/m^2, or `default`):

```shell
~$ jay randr output DP-1 brightness 203
~$ jay randr output DP-1 brightness default
```

Set the blend space:

```shell
~$ jay randr output DP-1 blend-space linear
~$ jay randr output DP-1 blend-space srgb
```

Enable or disable native gamut usage:

```shell
~$ jay randr output DP-1 use-native-gamut true
~$ jay randr output DP-1 use-native-gamut false
```

### Virtual Outputs

```shell
~$ jay randr virtual-output create my-virtual-display
~$ jay randr virtual-output remove my-virtual-display
```

---

## Input

All input commands live under `jay input`.

### Showing Input Devices

```shell
~$ jay input
~$ jay input show
~$ jay input show -v    # verbose output with device details
```

### Seat Commands

Show seat information:

```shell
~$ jay input seat default show
~$ jay input seat default show -v
```

Set keyboard repeat rate (repeats per second, initial delay in ms):

```shell
~$ jay input seat default set-repeat-rate 25 600
```

Set the keymap from RMLVO names:

```shell
~$ jay input seat default set-keymap-from-names -l de
~$ jay input seat default set-keymap-from-names -l us -v intl -o compose:ralt
```

Set the keymap from a file (or stdin):

```shell
~$ jay input seat default set-keymap /path/to/keymap.xkb
```

Retrieve the current keymap:

```shell
~$ jay input seat default keymap > current.xkb
```

Toggle hardware cursor:

```shell
~$ jay input seat default use-hardware-cursor true
~$ jay input seat default use-hardware-cursor false
```

Set cursor size:

```shell
~$ jay input seat default set-cursor-size 24
```

Configure the simple (XCompose-based) input method:

```shell
~$ jay input seat default simple-im enable
~$ jay input seat default simple-im disable
~$ jay input seat default simple-im reload
```

### Device Commands

Show device information:

```shell
~$ jay input device 42 show
```

Set acceleration profile and speed:

```shell
~$ jay input device 42 set-accel-profile flat
~$ jay input device 42 set-accel-profile adaptive
~$ jay input device 42 set-accel-speed -0.5
```

Configure tap behavior:

```shell
~$ jay input device 42 set-tap-enabled true
~$ jay input device 42 set-tap-drag-enabled true
~$ jay input device 42 set-tap-drag-lock-enabled false
```

Set left-handed mode:

```shell
~$ jay input device 42 set-left-handed true
```

Set natural scrolling:

```shell
~$ jay input device 42 set-natural-scrolling true
```

Set pixels per scroll-wheel step:

```shell
~$ jay input device 42 set-px-per-wheel-scroll 30.0
```

Set the transform matrix (2x2):

```shell
~$ jay input device 42 set-transform-matrix 1.0 0.0 0.0 1.0
```

Set the calibration matrix (2x3, for touchscreens):

```shell
~$ jay input device 42 set-calibration-matrix 1.0 0.0 0.0 0.0 1.0 0.0
```

Set the click method:

```shell
~$ jay input device 42 set-click-method clickfinger
~$ jay input device 42 set-click-method button-areas
~$ jay input device 42 set-click-method none
```

Toggle middle button emulation:

```shell
~$ jay input device 42 set-middle-button-emulation true
```

Set a per-device keymap:

```shell
~$ jay input device 42 set-keymap /path/to/keymap.xkb
~$ jay input device 42 set-keymap-from-names -l de
~$ jay input device 42 keymap > device.xkb
```

Attach / detach a device from a seat:

```shell
~$ jay input device 42 attach default
~$ jay input device 42 detach
```

Map a device to a specific output:

```shell
~$ jay input device 42 map-to-output DP-1
~$ jay input device 42 remove-mapping
```

---

## Idle & Locking

### `jay idle`

Show idle status, including the current interval, grace period, and active
inhibitors:

```shell
~$ jay idle
~$ jay idle status
```

### `jay idle set`

Set the idle interval. Durations can be specified in flexible formats:

```shell
~$ jay idle set 10m
~$ jay idle set 1m 30s
~$ jay idle set disabled
```

### `jay idle set-grace-period`

Set the grace period (screens go black but are not locked/disabled):

```shell
~$ jay idle set-grace-period 30s
~$ jay idle set-grace-period disabled
```

### `jay unlock`

Unlock the compositor. This is useful when the screen locker crashes and the
session remains locked. Run it from another TTY or via SSH. You must set
`WAYLAND_DISPLAY` to the socket of the Jay compositor:

```shell
~$ WAYLAND_DISPLAY=wayland-1 jay unlock
```

---

## Logging

### `jay log`

Open the log file in `less`:

```shell
~$ jay log
~$ jay log -f        # follow mode (like tail -f)
~$ jay log -e        # jump to end of log
~$ jay log --path    # print the log file path instead
```

### `jay set-log-level`

Change the log level at runtime:

```shell
~$ jay set-log-level debug
~$ jay set-log-level info
```

Available levels: `trace`, `debug`, `info`, `warn`, `error`, `off`.

---

## Screenshots

### `jay screenshot`

Take a screenshot of the entire display:

```shell
~$ jay screenshot
~$ jay screenshot --format png
~$ jay screenshot --format qoi
~$ jay screenshot my-screenshot.png
```

If no filename is given, the screenshot is saved as
`%Y-%m-%d-%H%M%S_jay.<ext>` in the current directory. The filename supports
strftime format specifiers.

---

## Clients & Windows

### `jay clients`

List all connected clients:

```shell
~$ jay clients
~$ jay clients show all
```

Show a specific client by ID:

```shell
~$ jay clients show id 42
```

Interactively select a window and show its client:

```shell
~$ jay clients show select-window
```

Kill a client by ID:

```shell
~$ jay clients kill id 42
```

Interactively select a window and kill its client:

```shell
~$ jay clients kill select-window
```

### `jay tree query`

Inspect the compositor's surface tree:

```shell
~$ jay tree query root
~$ jay tree query -r root                    # recursive
~$ jay tree query -r --all-clients root      # show client details for every node
~$ jay tree query workspace-name main
~$ jay tree query select-workspace
~$ jay tree query select-window
```

---

## Xwayland

### `jay xwayland`

Show Xwayland status (scaling mode, implied scale):

```shell
~$ jay xwayland
~$ jay xwayland status
```

### `jay xwayland set-scaling-mode`

```shell
~$ jay xwayland set-scaling-mode default
~$ jay xwayland set-scaling-mode downscaled
```

In `downscaled` mode, X11 windows are rendered at the highest integer scale and
then downscaled, which can improve sharpness on HiDPI displays.

---

## Color Management

### `jay color-management`

Show color management status:

```shell
~$ jay color-management
~$ jay color-management status
```

### `jay color-management enable / disable`

```shell
~$ jay color-management enable
~$ jay color-management disable
```

---

## Other Commands

### `jay control-center`

Open the [Control Center](control-center.md) GUI:

```shell
~$ jay control-center
```

### `jay portal`

Run the Jay desktop portal (provides screen sharing and other XDG desktop
portal interfaces):

```shell
~$ jay portal
```

Normally the portal is started automatically. This command is for running it
manually or debugging.

### `jay seat-test`

Test input events from a seat. Prints all keyboard, pointer, touch, gesture,
tablet, and switch events to stdout:

```shell
~$ jay seat-test
~$ jay seat-test default
~$ jay seat-test -a              # test all seats simultaneously
```

### `jay run-privileged`

Run a program with access to a privileged Wayland socket:

```shell
~$ jay run-privileged my-program --arg1
```

### `jay run-tagged`

Run a program with a tagged Wayland connection. All Wayland connections from the
spawned process tree will carry the specified tag, which can be matched in
[client rules](window-rules.md):

```shell
~$ jay run-tagged my-tag firefox
```

### `jay generate-completion`

Generate shell completion scripts:

```shell
~$ jay generate-completion bash
~$ jay generate-completion zsh
~$ jay generate-completion fish
~$ jay generate-completion elvish
~$ jay generate-completion powershell
```
