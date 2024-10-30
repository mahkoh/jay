# Building

## Compile-time Dependencies

The following libraries must be installed before compiling Jay:

- libinput.so
- libgbm.so
- libxkbcommon.so
- libudev.so
- libpangocairo-1.0.so

You must also have a C compiler (GCC or Clang) and the latest version of rust installed.
You can install rust with [rustup](https://rustup.rs/).

It is recommended that you install shaderc.
Otherwise it will be built from source which can take several minutes.

## Runtime Dependencies

Most of these dependencies are optional and will enable additional features.

- Linux 6.7: Required for explicit sync.
- Xwayland: Required for running X applications.
- Pipewire: Required for screen sharing.
- logind (part of systemd): Required when running Jay from a virtual terminal.
- libEGL.so and libGLESv2.so: Required for the OpenGL renderer.
- libvulkan.so: Required for the Vulkan renderer.

Note that Jay will not work if neither the OpenGL nor the Vulkan renderer are available.

## Compiling

To compile the latest stable version of Jay, run

```
cargo install --locked jay-compositor
```

This will install Jay under `$HOME/.cargo/bin/jay`.

If you want to use the latest version from git, run

```
cargo install --locked --git https://github.com/mahkoh/jay.git jay-compositor
```

If you only want to build Jay without installing it, run the following command from within this repository:

```
cargo build --release
```

The binary is then available under `./target/release/jay`.

# Setup

## Configuration

See [config.md](./config.md).

## Screen Sharing

This step is only required to enable screen sharing.

1. Copy `../etc/jay.portal` to `/usr/share/xdg-desktop-portal/portals/jay.portal`.
2. Copy `../etc/jay-portals.conf` to `/usr/share/xdg-desktop-portal/jay-portals.conf`.

Then restart `xdg-deskop-portal`.

# Running

1. Switch to a virtual terminal by pressing `ctrl-alt-F2` (or F3, F4, ...).
2. Run `jay run`.

If you have not yet changed the default configuration, you can

- quit Jay by pressing `alt-q`,
- start Alacritty by pressing the left Windows key.
