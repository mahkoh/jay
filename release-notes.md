# Unreleased

# 1.11.1 (2025-08-29)

## Fixes

This release fixes a bug that caused the compositor to abort on multi-GPU
systems in some situations. Thanks to @krakow10 for reporting this.

# 1.11.0 (2025-07-26)

## Fixes

As always, this release contains many bug fixes. Notably it changes the `top` layer in the
wlr-layer-shell-unstable-v1 protocol to be rendered below fullscreen windows. Previously
such layers were rendered on top of fullscreen windows which deviates from the behavior of
other compositors.

If this negatively affects your experience, try to configure the affected applications to
use the `overlay` layer instead.

Thanks to @disluckyguy for fixing this.

## New and Improved Protocols

This release implements the following new protocols:

- xdg-toplevel-tag-v1

  This protocol allows clients to add a string tag to their windows. These tags can be
  used in window rules (see below).

- wlr-foreign-toplevel-management-unstable-v1

  This protocol allows task bars to display and manage application windows.

  Thanks to @disluckyguy for implementing this.

- wlr-output-management-unstable-v1

  This protocol allows applications to manage the position, mode, etc. of outputs.

  Thanks to @disluckyguy for implementing this.

- pointer-warp-v1

  This protocol allows applications to warp the cursor within their own windows.
  Previously some applications abused the pointer-constraints-unstable-v1 protocol for the
  same purpose.

- tablet-v2, version 2

  This version of the tablet protocol adds support for pad dials.

- pointer-constraints-unstable-v1, position hints

  The implementation now honors position hints set by clients.

  Thanks to @tadeokondrak for suggesting this.

## Disabling the Built-In Bar

The built-in bar can now be disabled:

```toml
show-bar = true

[shortcuts]
alt-a = "show-bar"
alt-b = "hide-bar"
alt-c = "toggle-bar"
```

This can be useful if you want to use an external bar such as waybar.

## Client & Window Rules

Jay now supports client and window rules:

```toml
[[windows]]
match.content-types = ["video"]
action = "enter-fullscreen"
```

These rules are described in detail in
[window-and-client-rules.md](docs/window-and-client-rules.md).

## Window Management

This release contains many improvements related to how you can manage windows with the
mouse and keyboard.

- You can now show floating windows above fullscreen windows:

  ```toml
  alt-a = "enable-float-above-fullscreen"
  alt-b = "disable-float-above-fullscreen"
  alt-c = "toggle-float-above-fullscreen"
  ```

- Floating windows can now be pinned to an output. A pinned floating window remains
  visible even if you switch to a different workspace on the same output.

  You can pin a floating window by right-clicking on its title or with the following actions:

  ```toml
  alt-a = "pin-float"
  alt-b = "unpin-float-float"
  alt-c = "toggle-float-pinned"
  ```

  A pinned window has a pin icon drawn in front of its title. You can also configure the
  compositor so that a grayed-out version of the icon is always shown even if the window
  is not pinned:

  ```toml
  [float]
  show-pin-icon = true
  ```

  In this case you can also toggle between pinned and unpinned by left-clicking on the
  icon.

- Floating windows are now restacked when you click, touch, or press down a tablet tool
  anywhere inside of them. Previously this only happened when clicking on the window
  title.

  Thanks to @disluckyguy for suggesting this.

- Fullscreen windows can now be moved to other workspaces and outputs without first
  leaving fullscreen. You can do this with the usual keyboard shortcuts or in
  window-management mode by dragging the window to another output. Recall that you can
  configure a key to enable window-management mode as follows:

  ```toml
  window-management-key = "XF86Macro1"
  ```

  This will enable window-management mode while this key is being pressed.

- In window-management mode, tiled windows can now be dragged with the mouse to move them
  around. Previously this was possible by dragging the window title, but in window
  management mode you can now click anywhere within the window.

  Note that, in window-management mode, dragging the title will actually drag the
  container containing the window.

  Thanks to @Stoppedpuma for suggesting this.

- Jay allows you to always revert the pointer to its default state by pressing the Escape
  key. For example, if you're performing a drag-and-drop operation, you can press Escape
  to abort it. Some people want to use the Escape key legitimately without having this
  side effect on the pointer.

  You can now configure which key performs this operation:

  ```toml
  pointer-revert-key = "Escape"
  ```

  You can disable this feature altogether by setting it to `NoSymbol`.

  Thanks to @kotarac for suggesting this.

- You can now assign vim-like marks to windows and later jump back to them:

  ```toml
  [shortcuts]
  alt-a = "create-mark"
  alt-b = "jump-to-mark"
  ```

  When these actions are executed, the next key press marks the selected window with that
  key or jumps to the window that was previously marked with that key.

  Instead of selecting the key interactively, you can also specify it in the config
  itself:

  ```toml
  [shortcuts]
  alt-a = { type = "create-mark", id.key = "a" }
  ```

  The key names can be found in the `input-event-codes.h` file in your `/usr/include`
  directory. The names should have the `KEY_` prefix removed and must be written
  all-lowercase.

  Alternatively, you can use marks that are identified by a string name instead of a key:

  ```toml
  [shortcuts]
  alt-a = { type = "create-mark", id.name = "my mark name" }
  ```

  These marks live in their own namespace and cannot be accessed with the interactive key
  selection.

- Jay now maintains a focus history. The history allows you to navigate between your
  windows in the order in which you focused them:

  ```toml
  alt-a = "focus-prev"
  alt-b = "focus-next"
  ```

  You can customize the behavior of these actions with the following settings:

  ```toml
  [focus-history]
  only-visible = false
  same-workspace = true
  ```

  If `only-visible` is `true`, then actions will only move the focus to windows that are
  already visible. Otherwise it will make windows visible before focusing them, moving
  between tabs and workspaces as necessary.

  If `same-workspace` is `true`, then only windows on the same workspace will be focused.

  The defaults are `false` for both.

- When switching to a workspace, Jay will now put the focus on the last window that was
  previously focused on that workspace, which might be a floating window. Previously Jay
  would always put the focus on the last _tiled_ window that was previously focused.

- You can now navigate between tiled and floating layers without using the mouse:

  ```toml
  [shortcuts]
  alt-a = "focus-below"
  alt-b = "focus-above"
  ```

  This should allow you to put the keyboard focus on any window without having to use the
  mouse.

  Thanks to @Stoppedpuma for suggesting this.

## Toml Improvements

These improvements apply to the toml-based configuration:

- You can now assign names to actions:

  ```toml
  actions.xyz = [
      { type = "move-to-workspace", name = "1" },
      "enter-fullscreen",
  ]
  ```

  These actions can then be executed by prefixing their name with a `$`:

  ```toml
  [shortcuts]
  alt-a = "$xyz"
  ```

  This can be useful when the same action is used in multiple places.

  You can also re-define or un-define actions at runtime:

  ```toml
  [shortcuts]
  alt-a = {
    type = "define-action",
    name = "xyz",
    action = [
      { type = "move-to-workspace", name = "2" },
      "enter-fullscreen",
    ],
  }
  alt-b = { type = "undefine-action", name = "xyz" }
  ```

  These types of redefinitions can be used for a limited amount of dynamic behavior. For
  example, you can have the same key cycle between a number of workspaces.

- Jay now supports input modes:

  ```toml
  [shortcuts]
  alt-x = { type = "push-mode", name = "navigation" }

  [modes."navigation".shortcuts]
  Escape     = "pop-mode"
  w          = "focus-up"
  a          = "focus-left"
  s          = "focus-down"
  d          = "focus-right"
  q          = "focus-prev"
  e          = "focus-next"
  r          = "focus-above"
  f          = "focus-below"
  m          = "create-mark"
  apostrophe = "jump-to-mark"
  ```

  These modes allow you to define shortcuts that are overlayed on the normal shortcuts
  while the mode is active.

  If you have a foot pedal or same easy-to-reach key, you can use Jay's complex-shortcuts
  mechanism to enter the mode while you're holding the key down:

  ```toml
  [complex-shortcuts.XF86Macro2]
  action = { type = "push-mode", name = "navigation" } # Executes when the key is pressed
  latch = "pop-mode"                                   # Executes when the key is released
  ```

  If you prefer key chords, you can use the `latch-mode` action:

  ```toml
  [shortcuts]
  alt-x = { type = "latch-mode", name = "navigation" }
  ```

  This acts like `push-mode` except that the mode is automatically popped when the next
  shortcut is executed.

  By default, modes inherit from the default shortcuts, however, you can also configure
  them to inherit from another mode:

  ```toml
  [modes.m1.shortcuts]
  # ...

  [modes.m2]
  parent = "m1"
  [modes.m2.shortcuts]
  # ...
  ```

  In this case, the shortcuts that are active while the `m2` mode is active are normal
  shortcuts, overwritten by the `m1` shortcuts, overwritten by the `m2` shortcuts.

  Note that you can explicitly unset a shortcut by assigning it the `none` action.

## Support for `cap_sys_nice`

Jay now supports running with the `cap_sys_nice` capability:

```shell
~$ sudo setcap cap_sys_nice=+p /path/to/jay
```

If the compositor is started with this capability, it will set its own scheduler policy
to `SCHED_RR` which will allow the compositor to stay responsive even if the system is
under heavy load.

If the Vulkan renderer is used, it will also request a high-priority context which can
allow the kernel to give it priority when scheduling GPU operations.

Note that the capability and scheduler policy will not be inherited by child processes.

## Miscellaneous Changes

- The `jay-config` crate now supports `Seat::get_keyboard_output` and
  `Connector::workspaces` functions to retrieve the output that has the keyboard focus and
  retrieve the list of workspaces located on an output.

  Thanks to @khyperia for implementing this.

- The libinput click-method and middle-button-emulation settings can now be set via the
  configuration or on the command line.

- The use of hardware cursors can now be disabled in the toml configuration:

  ```toml
  use-hardware-cursor = false
  ```

- The primary selection protocol can now be disabled:

  ```toml
  middle-click-paste = true
  ```

  Thanks to @kotarac for suggesting this.

- Jay can now be compiled against musl libc.

  Thanks to @elde-n for implementing this.

- Jay now works with some status programs that produce incorrectly formatted i3status
  output.

  Thanks to @disluckyguy for implementing this.

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
