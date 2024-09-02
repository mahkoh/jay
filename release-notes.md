# Unreleased

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
