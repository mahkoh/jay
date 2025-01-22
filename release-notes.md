# Unreleased

- Various bugfixes.
- Add support fo ext-data-control-v1.
- Implement wl-fixes.
- Implement ei_touchscreen v2.

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
- Implement ext-tray-v1.

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
