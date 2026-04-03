# Features

This chapter provides a high-level overview of what Jay can do. Each feature
links to the chapter where it is covered in detail.

## Configuration

Jay can be configured via:

- a **declarative TOML file**, or
- a **shared library** that gets injected into the compositor for programmatic
  control.

Most users will use the TOML file. The configuration supports composable
actions, [input modes](input-modes.md) for vim-style modal keybindings, and
powerful [match rules](window-rules.md). See
[Configuration Overview](configuration/index.md) for details on getting started
and how the config file works.

## i3 Look and Feel

Jay provides an i3-inspired tiling layout with manual tiling,
horizontal/vertical splits, fullscreen, and floating windows. Its appearance is
based on the default i3 look and feel.

Colors, sizes, and fonts can all be customized. See
[Theme & Appearance](configuration/theme.md) for details, and
[Tiling](tiling.md) and [Floating Windows](floating.md) for how window
management works.

## Stability

Jay has been stable for a long time. Crashes and incorrect behavior in released
versions are rare.

Jay also aims to be forward and backward compatible for existing setups,
allowing you to upgrade or downgrade the compositor without having to adjust
your configuration.

There is a small but growing integration test suite that is used to ensure this.

## Command-Line Interface

Jay has a comprehensive CLI that can be used to inspect and configure the
compositor at runtime. All query commands support `--json` for machine-readable
output:

```
~$ jay
A wayland compositor

Usage: jay [OPTIONS] <COMMAND>

Commands:
  run                  Run the compositor
  config               Create/modify the toml config
  generate-completion  Generate shell completion scripts for jay
  log                  Open the log file
  set-log-level        Sets the log level
  quit                 Stop the compositor
  unlock               Unlocks the compositor
  screenshot           Take a screenshot
  idle                 Inspect/modify the idle (screensaver) settings
  run-privileged       Run a privileged program
  run-tagged           Run a program with a connection tag
  seat-test            Tests the events produced by a seat
  portal               Run the desktop portal
  randr                Inspect/modify graphics card and connector settings
  input                Inspect/modify input settings
  xwayland             Inspect/modify xwayland settings
  color-management     Inspect/modify the color-management settings
  clients              Inspect/manipulate the connected clients
  tree                 Inspect the surface tree
  control-center       Opens the control center
  version              Prints the Jay version and exits
  pid                  Prints the Jay PID and exits
  help                 Print this message or the help of the given subcommand(s)

Options:
      --log-level <LOG_LEVEL>  The log level [default: info] [possible values: trace, debug, info, warn, error, off]
      --json                   Output data as JSONL
      --all-json-fields        Print all fields in JSON output
  -h, --help                   Print help (see more with '--help')
```

See the full [Command-Line Interface](cli.md) reference for details.

## Control Center

Jay includes a built-in GUI control center (opened with `alt-c`) for managing
outputs, input devices, GPUs, idle settings, color management, and more --
without editing the config file. See [Control Center](control-center.md).

## Multi-Monitor Support

Jay can be used with multiple monitors with hot-plug and hot-unplug support.
When a monitor is unplugged, all of its workspaces are automatically moved to
one of the remaining monitors. When the monitor is plugged in again, these
workspaces are restored.

See [Outputs (Monitors)](configuration/outputs.md) for configuration options.

## Multi-GPU Support

Jay can be used with multiple GPUs and monitors connected to different GPUs.
One GPU is always used for rendering the desktop. You can change this GPU at
runtime.

See [GPUs](configuration/gpu.md) for details.

## Screen Sharing

Jay supports screen sharing via xdg-desktop-portal. Three capture modes are
available:

- **Window capture** -- share a single window.
- **Output capture** -- share an entire monitor.
- **Workspace capture** -- like output capture, but only one workspace is shown.

See [Screen Sharing](screen-sharing.md) for setup instructions.

## Screen Locking

Jay can automatically lock your screen and disable outputs after inactivity.
See [Idle & Screen Locking](configuration/idle.md) for configuration options.

## Notifications

Jay supports the `zwlr_layer_shell_v1` protocol used by notification daemons
such as [mako](https://github.com/emersion/mako), which is launched
automatically by the default configuration.

## Fractional Scaling

Jay supports per-monitor fractional scaling. Scale factors can be set per
output in the config file or at runtime via the control center and CLI.

See [Outputs (Monitors)](configuration/outputs.md) for details.

## OpenGL and Vulkan

Jay can use either OpenGL or Vulkan for rendering. Vulkan offers better
performance and memory usage but OpenGL is still provided for older hardware.

You can change the rendering API at runtime without restarting the compositor.

See [GPUs](configuration/gpu.md) for details.

## Explicit Sync

Jay supports explicit sync for compatibility with Nvidia hardware. This
requires Linux 6.7 or later.

## Xwayland

Jay supports running X11 applications seamlessly through Xwayland. See
[Xwayland](configuration/xwayland.md) for configuration options.

## Clipboard Managers

Jay supports clipboard managers via the `zwlr_data_control_manager_v1` and
`ext_data_control_manager_v1` protocols.

## Privilege Separation

Jay splits protocols into unprivileged and privileged protocols. By default,
applications only have access to unprivileged protocols. This means that tools
like screen lockers, status bars, and clipboard managers need to be explicitly
granted access.

Jay provides several ways to grant privileges, from giving a program full
access to all privileged protocols down to granting individual capabilities to
specific tagged processes. See
[Granting Privileges](window-rules.md#granting-privileges) for a detailed
guide and the [Protocol Support](#protocol-support) section below for the full
list of protocols and their privilege requirements.

## Push to Talk

Jay's shortcut system allows you to execute an action when a key is pressed and
a different action when the key is released, enabling push-to-talk
functionality. See [Shortcuts](configuration/shortcuts.md) for details.

## VR

Jay supports leasing VR headsets to applications via the
`wp_drm_lease_device_v1` protocol.

## Adaptive Sync

Jay supports adaptive sync (VRR) with configurable cursor refresh rates.
See [Outputs (Monitors)](configuration/outputs.md) for per-output VRR settings.

## Tearing

Jay supports tearing presentation for games. See
[Outputs (Monitors)](configuration/outputs.md) for per-output tearing settings.

## Low Input Latency

Jay uses frame scheduling to achieve input latency as low as 1.5 ms.

## Color Management & HDR

Jay supports the Wayland color management protocol and HDR10 output with
per-monitor color space, transfer function, brightness, and blend space
controls. See [HDR & Color Management](hdr.md) for a full walkthrough.

## Night Light

Jay supports night-light applications via the
`zwlr_gamma_control_manager_v1` protocol.

## Window and Client Rules

Jay supports powerful, reactive window and client rules. Rules are
re-evaluated whenever matching criteria change (e.g. a window's title changes).

See [Window & Client Rules](window-rules.md) for details.

## Protocol Support

Jay supports a large number of Wayland protocols. Protocols marked as
**Privileged** are only accessible to applications that have been explicitly
granted access. See
[Granting Privileges](window-rules.md#granting-privileges) for how to do this.

| Protocol                                             | Version | Privileged |
|------------------------------------------------------|---------|------------|
| ext_data_control_manager_v1                          | 1       | Yes        |
| ext_foreign_toplevel_image_capture_source_manager_v1 | 1       |            |
| ext_foreign_toplevel_list_v1                         | 1       | Yes        |
| ext_idle_notifier_v1                                 | 2       | Yes        |
| ext_image_copy_capture_manager_v1                    | 1[^2]   | Yes        |
| ext_output_image_capture_source_manager_v1           | 1       |            |
| ext_session_lock_manager_v1                          | 1       | Yes        |
| ext_transient_seat_manager_v1                        | 1[^3]   | Yes        |
| ext_workspace_manager_v1                             | 1       | Yes        |
| jay_popup_ext_manager_v1                             | 1       |            |
| jay_tray_v1                                          | 1       |            |
| org_kde_kwin_server_decoration_manager               | 1       |            |
| wl_compositor                                        | 7       |            |
| wl_data_device_manager                               | 4       |            |
| wl_drm                                               | 2       |            |
| wl_fixes                                             | 2       |            |
| wl_output                                            | 4       |            |
| wl_seat                                              | 10      |            |
| wl_shm                                               | 2       |            |
| wl_subcompositor                                     | 1       |            |
| wp_alpha_modifier_v1                                 | 1       |            |
| wp_color_manager_v1                                  | 2       |            |
| wp_color_representation_manager_v1                   | 1       |            |
| wp_commit_timing_manager_v1                          | 1       |            |
| wp_content_type_manager_v1                           | 1       |            |
| wp_cursor_shape_manager_v1                           | 2       |            |
| wp_drm_lease_device_v1                               | 1       |            |
| wp_fifo_manager_v1                                   | 1       |            |
| wp_fractional_scale_manager_v1                       | 1       |            |
| wp_linux_drm_syncobj_manager_v1                      | 1       |            |
| wp_pointer_warp_v1                                   | 1       |            |
| wp_presentation                                      | 2       |            |
| wp_security_context_manager_v1                       | 1       |            |
| wp_single_pixel_buffer_manager_v1                    | 1       |            |
| wp_tearing_control_manager_v1                        | 1       |            |
| wp_viewporter                                        | 1       |            |
| xdg_activation_v1                                    | 1       |            |
| xdg_session_manager_v1                               | 1       |            |
| xdg_toplevel_drag_manager_v1                         | 1       |            |
| xdg_toplevel_tag_manager_v1                          | 1       |            |
| xdg_wm_base                                          | 7       |            |
| xdg_wm_dialog_v1                                     | 1       |            |
| zwlr_data_control_manager_v1                         | 2       | Yes        |
| zwlr_foreign_toplevel_manager_v1                     | 3       | Yes        |
| zwlr_gamma_control_manager_v1                        | 1       | Yes        |
| zwlr_layer_shell_v1                                  | 5       | No[^1]     |
| zwlr_output_manager_v1                               | 4       | Yes        |
| zwlr_screencopy_manager_v1                           | 3       | Yes        |
| zwlr_virtual_pointer_manager_v1                      | 2       | Yes        |
| zwp_idle_inhibit_manager_v1                          | 1       |            |
| zwp_input_method_manager_v2                          | 1       | Yes        |
| zwp_linux_dmabuf_v1                                  | 5       |            |
| zwp_pointer_constraints_v1                           | 1       |            |
| zwp_pointer_gestures_v1                              | 3       |            |
| zwp_primary_selection_device_manager_v1              | 1       |            |
| zwp_relative_pointer_manager_v1                      | 1       |            |
| zwp_tablet_manager_v2                                | 2       |            |
| zwp_text_input_manager_v3                            | 1       |            |
| zwp_virtual_keyboard_manager_v1                      | 1       | Yes        |
| zxdg_decoration_manager_v1                           | 2       |            |
| zxdg_output_manager_v1                               | 3       |            |

[^1]: Sandboxes can restrict access to this protocol.
[^2]: Cursors are always composited.
[^3]: Seat creation is always rejected.
