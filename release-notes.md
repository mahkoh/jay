# Unreleased

- Various bugfixes.
- Floating windows can now be configured to be shown above fullscreen windows
  by using the `enable-float-above-fullscreen` action.
- Implement xdg-toplevel-tag-v1.
- Implement tablet-v2 version 2.
- Floating windows can now be pinned. A pinned floating window stays visible on
  its output even when switching workspaces.
- The toml config can now contain named actions:

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
- Add client and window rules. This is described in detail in
  [window-and-client-rules.md](./docs/window-and-client-rules.md).
- Add client and tree CLI subcommands to inspect clients and windows, primarily
  to facilitate the writing of window and client rules.
- Jay now supports being started with CAP_SYS_NICE capabilities to improve
  responsiveness under high system load. This is described in detail in
  [setup.md](docs/setup.md).
- Implement wlr-foreign-toplevel-management-v1.

# 1.10.0 (2025-04-22)

- Various bugfixes.
- Implement cursor-shape-v1 version 2.
- Implement xdg-shell version 7.
- Implement color-management-v1.

  This release adds support for color management if the vulkan renderer is used
  and the vulkan driver supports the VK_EXT_descriptor_buffer extension. Color
  management is not available with the opengl renderer.

  There are two limitations:

  - No tone mapping is performed. That is, HDR content that is brighter than
    what the framebuffer can store will be clipped.
  - No gamut mapping is performed. That is, content that is more colorful than
    what the framebuffer can store will be clipped.

  The color management protocol is disabled by default. You can enable it on the
  command line via `jay color-management enable` or in the config file via
  `color-management.enabled = true`. If you are not using an OLED display, you
  probably want to keep it disabled and instead rely on the tone mapping
  performed by applications such as mpv. Changing this setting usually requires
  applications to be restarted.

  You can use this application to test various settings:
  https://github.com/mahkoh/wayland-color-test
- Outputs can now optionally use the BT.2020/PQ color space.

  On the command line this can be controlled via
  `jay randr output <OUTPUT> colors ...`. In the config file it can be
  controlled via the `color-space` and `transfer-function` fields in the output
  configuration.

  When using the PQ transfer function, the output should also be configured to
  use at least a 10 bpc framebuffer format.
- The reference brightness of outputs can now be configured via
  `jay randr output <OUTPUT> brightness ...` or in the config file via the
  `brightness` field in the output configuration.

  This is primarily useful when using the PQ transfer function and the display
  does not support configuring the brightness via its hardware overlay.

  This configuration has no effect unless the vulkan renderer is used and the
  vulkan driver supports the VK_EXT_descriptor_buffer extension.
- The vulkan renderer has been significantly improved to ensure high performance
  even with color management enabled:
  - VK_EXT_descriptor_buffer is used if available.
  - Only the damaged areas of the screen are rendered.
  - Draw calls are instanced so that only one draw call is required per texture
    even if there are multiple small damage areas.
  - Blending is now performed with a dedicated 16 bpc texture in linear space.
  - The blend buffer is deduplicated between outputs with the same size.
  - Areas of the screen where the topmost texture is opaque bypass the blend
    buffer and render directly to the frame buffer.

# 1.9.1 (2025-02-13)

This release updates the kbvm crate to fix an issue that would cause applications to
interpret the AltGr key as the Alt key.

Reported by @Honkeh in https://github.com/mahkoh/jay/issues/364.

# 1.9.0 (2025-01-27)

This release replaces xkbcommon by the kbvm crate.

This is a huge change in how input is handled. The intention is that this is completely
invisible to users.

Therefore this release contains only this change. You can downgrade to the previous
release to switch back to xkbcommon without any loss in functionality. In this case please
report what is broken.

# 1.8.0 (2025-01-27)

- Various bugfixes.
- Implement ext-data-control-v1.
- Implement wl-fixes.
- Implement ei_touchscreen v2.
- Implement idle-notification v2.
- Add an idle grace period. During the grace period, the screen goes black but is neither
  disabled nor locked. This is similar to how android handles going idle. The default is
  5 seconds.
- Implement ext-workspace-v1.

# 1.7.0 (2024-10-25)

- Various bugfixes.
- Tiles and workspaces can now be dragged with the mouse.
- Vulkan is now the default renderer.
- Emulate vblank events on the nvidia driver.
- Allow X windows to scale themselves.
- Implement ext-image-capture-source-v1.
- Implement ext-image-copy-capture-v1.
- Implement screencast session restoration.
- Fix screen sharing in zoom.
- Implement wp-fifo-v1.
- Implement wp-commit-timing-v1.
- Implement jay-tray-v1. You can get tray icons and menus by using
  https://github.com/mahkoh/wl-tray-bridge.

# 1.6.0 (2024-09-25)

- Various bugfixes.
- Improve compatibility Nvidia hardware.
- Implement format negotiation for screencasts.
- Allow configuring 6, 8, or 10 bit framebuffer formats.
- Upload shm textures on a separate thread in the Vulkan renderer.
- Disable implicit sync in KMS.
- Implement frame scheduling for KMS.
- The JAY_MAX_RENDER_TIME_NSEC environment variable has been removed.

# 1.5.0 (2024-09-02)

- Add fine-grained damage tracking.
- Add support for adaptive sync.
- Add support for tearing.
- Add support for touch input.
- Add support for libei.
- Add support for RemoteDesktop portal.

# 1.4.0 (2024-07-07)

- Add window management mode.
- Various bugfixes.

# 1.3.0 (2024-05-25)

- Add remaining layer-shell features.
- Add JAY_MAX_RENDER_TIME_NSEC environment variable.
  This can be used to delay rendering until shortly before a page flip, reducing input
  delay.
  This is an unstable feature that might change in the future.
- Various bugfixes.
- Improve performance of Vulkan renderer.

# 1.2.0 (2024-05-05)

- Add support for wp-security-manager-v1.
- Add support for xdg-dialog-v1.
- Add support for ext-transient-seat-v1.
- Add support for wp-drm-lease-v1.
- Focus-follows-mouse can now be disabled.
- Add support for pointer-gestures-unstable-v1.
- Configs can now handle switch events (laptop lid closed/opened).
- Add support for tablet-v2.
- Add support for linear framebuffers (hardware cursors/screensharing) on NVIDIA if the Vulkan renderer is used. (The OpenGL renderer does not support this.)

# 1.1.0 (2024-04-22)

- Screencasts now support window capture.
- Screencasts now support workspace capture.
- Add support for wp-alpha-modifier.
- Add support for per-device keymaps.
- Add support for virtual-keyboard-unstable-v1.
- Add support for zwp_input_method_manager_v2.
- Add support for zwp_text_input_manager_v3.
- Add support for push-to-talk.
- Various bugfixes.

# 1.0.3 (2024-04-11)

- Partially disable explicit sync on nvidia drivers.

# 1.0.2 (2024-04-10)

- Fixed a bug that caused the portal to fail.

# 1.0 (2024-04-07)

This is the first stable release of Jay.
