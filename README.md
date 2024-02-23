# Jay

Jay is a Wayland compositor written and configured in the rust programming
language with hot-reload support. Jay offers improved flexibility,
configurability, stability, and performance.

![screenshot.png](static/screenshot.png)

## Status

Jay is beta-quality software. For many people it should be possible to use Jay for most
of their work. Jay has a small integration test suite but sometimes there are regressions
that cause features to break. I'm currently looking for people willing to test Jay,
especially on Nvidia hardware.

### Working Features

The following features have been implemented and should work:

- Tiling windows
- Floating windows
- Fullscreen
- Multiple workspaces
- Multiple monitors
- Copy/paste including middle-click paste
- Screenshots
- Screencasting
- Keyboard shortcuts
- Theming
- Configuration reload
- XWayland
- Screensaver (paused during video playback)
- Notifications (via mako)
- Video playback with synced audio (via presentation time)
- Simple games that don't require cursor grabs
- GPU reset recovery
- Screen locking
- Monitor hotplug
- Fractional scaling
- Hardware cursors
- Pointer constraints
- Selecting the primary device in multi-GPU systems 
- An OpenGL backend
- A Vulkan backend

### Missing Features

The following features are known to be missing or broken and will be implemented
later:

- Touch and tablet support
- Damage tracking (any kind of damage causes a complete re-render currently)

## Native library dependencies

Jay is written in rust and will fetch all of its rust dependencies
automatically. It is however unavoidable that Jay depends on a number of native
libraries:

* **libinput.so**: For input event processing.
* **libgbm.so**: For graphics buffer allocation.
* **libxkbcommon.so**: For keymap handling.
* **libudev.so**: For device enumeration and hotplug support.
* **libpangocairo-1.0.so**: For text rendering.

These libraries are usually available on any Wayland-capable system.

## Runtime dependencies

At runtime, Jay depends on the following services being available on the system:

* **An up-to-date linux kernel and graphics drivers**: Jay makes aggressive use
  of linux features and might not work on older systems.
* **XWayland**: For XWayland support.
* **Pipewire**: For screencasting.
* **A running X server**: For the X backend. (Only required if you want to run
  Jay as an X client.)
* **Logind**: For the metal backend. (Only required if you want to run Jay from
  a TTY.)
* **libEGL.so**, **libGLESv2.so**: For the OpenGL backend.
* **libvulkan.so**: For the Vulkan backend.

## Building and Installing

Install the latest stable version of rustc and cargo. Follow the instructions on
https://rustup.rs or use the packages provided by your distribution. Note that
only the latest stable version is supported.

You can now build Jay using this command:
```sh
cargo build --release
```
The resulting binary will be located at `./target/release/jay`.

Alternatively, cargo can also install the binary for you:
```sh
cargo install --path .
```
This will install the binary at `$HOME/.cargo/bin/jay`. If you have not already
done so, you can add `$HOME/.cargo/bin` to your path.

## Running

You can run Jay as a freestanding compositor or as an application under X.

To start Jay as a freestanding compositor switch to a virtual terminal by
pressing `CTRL-ALT-F2` (or F3, F4, ...) and run
```sh
jay run
```

To start Jay as an X application, execute the same command from a terminal
emulator under X.

Before running Jay as a freestanding compositor, you might want to familiarize
yourself with the [default keyboard shortcuts][shortcuts]. In particular, you
can quit Jay by typing `ALT-q`.

[shortcuts]: ./default-config/src/lib.rs

## Configuration

Jay is configured using a shared library. A good starting point for your own
configuration is the [default config crate][default].

[default]: ./default-config

1. Copy this crate to a new directory.
2. In `Cargo.toml`
    - Update the path dependency to point to the correct directory.
    - Change the name of the crate to `my-jay-config`.
3. Make a useful change to `lib.rs`.
4. Build the crate with `cargo build`.
5. Move `target/debug/libmy_jay_config.so` to `$HOME/.config/jay/config.so`.

When you start Jay, you will be able to make use of your useful change. At
runtime you can repeat steps 3 to 5 and reload the configuration. By default,
the shortcut to reload the configuration is `ALT-r`.

If you want to see a more elaborate configuration, take a look at [my personal
configuration][personal].

[personal]: https://github.com/mahkoh/my-jay-config

## Screensharing

Jay supports [xdg-desktop-portal-wlr][xdpw] but Jay is not currently listed in
xdg-desktop-portal-wlr's wlr.portal file. To get screensharing to work, you have
to manually edit `/usr/share/xdg-desktop-portal/portals/wlr.portal` and add
`jay` to the `UseIn` list.

In the future, Jay will provide a desktop portal itself.

[xdpw]: https://github.com/emersion/xdg-desktop-portal-wlr

## License

Jay is free software licensed under the GNU General Public License v3.0.
