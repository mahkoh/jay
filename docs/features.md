# Features

## Configuration

Jay can be configured via

- a declarative TOML file or
- a shared library that gets injected into the compositor.

See [config.md](config.md) for more details.

## i3 Look and Feel

Jay's appearance is based on the default i3 look and feel.

Colors, sizes, and fonts can be customized.

## Stability

Jay has been stable for a long time.
Crashes and incorrect behavior in released versions are very rare.

Jay also aims to be forward and backward compatible for existing setups, allowing you to
upgrade or downgrade the compositor without having to adjust your configuration.

There is a small but growing integration test suite that is used to ensure this.

## CLI

Jay has a CLI that can be used to configure the compositor at runtime.

```
~$ jay
A wayland compositor

Usage: jay [OPTIONS] <COMMAND>

Commands:
  run                  Run the compositor
  generate-completion  Generate shell completion scripts for jay
  log                  Open the log file
  set-log-level        Sets the log level
  quit                 Stop the compositor
  unlock               Unlocks the compositor
  screenshot           Take a screenshot
  idle                 Inspect/modify the idle (screensaver) settings
  run-privileged       Run a privileged program
  seat-test            Tests the events produced by a seat
  portal               Run the desktop portal
  randr                Inspect/modify graphics card and connector settings
  input                Inspect/modify input settings
  help                 Print this message or the help of the given subcommand(s)

Options:
      --log-level <LOG_LEVEL>  The log level [default: info] [possible values: trace, debug, info, warn, error]
  -h, --help                   Print help
```

## Multi-Monitor Support

Jay can be used with multiple monitors with hot-plug and hot-unplug support.
When a monitor is unplugged, all workspaces are automatically moved one of the remaining
monitors.
When the monitor is plugged in again, these workspaces are restored.

## Multi-GPU Support

Jay can be used with multiple GPUs and monitors connected to different GPUs.
One GPU is always used for rendering the desktop.
You can change this GPU at runtime.

## Screen Sharing

Jay supports screen sharing via xdg-desktop-portal.
There are three supported modes:

- Window capture
- Output capture
- Workspace capture which is like output capture except that only one workspace will be
  shown.

## Screen Locking

Jay can automatically lock your screen and disable outputs after inactivity.

## Notifications

Jay supports the zwlr_layer_shell_v1 protocol used by notification daemons.

## Fractional Scaling

Jay supports per-monitor fractional scaling.

## OpenGL and Vulkan

Jay can use either OpenGL or Vulkan for rendering.
Vulkan offers better performance and memory usage but OpenGL is still provided for
older hardware.

You can change the API at runtime without restarting the compositor.

## Explicit Sync

Jay supports explicit sync for compatibility with Nvidia hardware.

## Clipboard Managers

Jay supports clipboard managers via `zwlr_data_control_manager_v1`.

## Privilege Separation

Jay splits protocols into unprivileged and privileged protocols.
By default, applications only have access to unprivileged protocols.

You can explicitly opt into giving applications access to privileged protocols via the Jay CLI or shortcuts.

## Push to Talk

Jay's shortcut system allows you to execute an action when a key is pressed and to execute a different action when the key is released.

## VR

Jay supports leasing VR headsets to applications.

## Adaptive Sync

Jay supports adaptive sync with configurable cursor refresh rates.

## Tearing

Jay supports tearing presentation for games.

## Low Input Latency

Jay uses frame scheduling to achieve input latency as low as 1.5 ms.

## Protocol Support

Jay supports the following wayland protocols:

| Global                                               | Version         | Privileged    |
|------------------------------------------------------|:----------------|---------------|
| ext_data_control_manager_v1                          | 1               | Yes           |
| ext_foreign_toplevel_image_capture_source_manager_v1 | 1               |               |
| ext_foreign_toplevel_list_v1                         | 1               | Yes           |
| ext_idle_notifier_v1                                 | 2               | Yes           |
| ext_image_copy_capture_manager_v1                    | 1[^composited]  | Yes           |
| ext_output_image_capture_source_manager_v1           | 1               |               |
| ext_session_lock_manager_v1                          | 1               | Yes           |
| ext_transient_seat_manager_v1                        | 1[^ts_rejected] | Yes           |
| ext_workspace_manager_v1                             | 1               | Yes           |
| jay_tray_v1                                          | 1               |               |
| org_kde_kwin_server_decoration_manager               | 1               |               |
| wl_compositor                                        | 6               |               |
| wl_data_device_manager                               | 3               |               |
| wl_drm                                               | 2               |               |
| wl_fixes                                             | 1               |               |
| wl_output                                            | 4               |               |
| wl_seat                                              | 9               |               |
| wl_shm                                               | 2               |               |
| wl_subcompositor                                     | 1               |               |
| wp_alpha_modifier_v1                                 | 1               |               |
| wp_color_manager_v1                                  | 1[^color_mng]   |               |
| wp_commit_timing_manager_v1                          | 1               |               |
| wp_content_type_manager_v1                           | 1               |               |
| wp_cursor_shape_manager_v1                           | 2               |               |
| wp_drm_lease_device_v1                               | 1               |               |
| wp_fifo_manager_v1                                   | 1               |               |
| wp_fractional_scale_manager_v1                       | 1               |               |
| wp_linux_drm_syncobj_manager_v1                      | 1               |               |
| wp_presentation                                      | 2               |               |
| wp_security_context_manager_v1                       | 1               |               |
| wp_single_pixel_buffer_manager_v1                    | 1               |               |
| wp_tearing_control_manager_v1                        | 1               |               |
| wp_viewporter                                        | 1               |               |
| xdg_activation_v1                                    | 1               |               |
| xdg_toplevel_drag_manager_v1                         | 1               |               |
| xdg_wm_base                                          | 6               |               |
| xdg_wm_dialog_v1                                     | 1               |               |
| zwlr_data_control_manager_v1                         | 2               | Yes           |
| zwlr_layer_shell_v1                                  | 5               | No[^lsaccess] |
| zwlr_screencopy_manager_v1                           | 3               | Yes           |
| zwp_idle_inhibit_manager_v1                          | 1               |               |
| zwp_input_method_manager_v2                          | 1               | Yes           |
| zwp_linux_dmabuf_v1                                  | 5               |               |
| zwp_pointer_constraints_v1                           | 1               |               |
| zwp_pointer_gestures_v1                              | 3               |               |
| zwp_primary_selection_device_manager_v1              | 1               |               |
| zwp_relative_pointer_manager_v1                      | 1               |               |
| zwp_tablet_manager_v2                                | 1               |               |
| zwp_text_input_manager_v3                            | 1               |               |
| zwp_virtual_keyboard_manager_v1                      | 1               | Yes           |
| zxdg_decoration_manager_v1                           | 1               |               |
| zxdg_output_manager_v1                               | 3               |               |

[^lsaccess]: Sandboxes can restrict access to this protocol.
[^ts_rejected]: Seat creation is always rejected.
[^composited]: Cursors are always composited.
[^color_mng]: Only SRGB is supported.
