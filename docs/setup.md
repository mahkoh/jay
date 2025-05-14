# Building

## Compile-time Dependencies

The following libraries must be installed before compiling Jay:

- libinput.so
- libgbm.so
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

## Running with CAP_SYS_NICE

Jay supports being started with CAP_SYS_NICE capabilities. For example, such
capabilities can be added to the binary via

```shell
~# setcap cap_sys_nice=p jay
```

If CAP_SYS_NICE is available, Jay will, by default, elevate its scheduler to
SCHED_RR and create Vulkan queues with the highest available priority. This can
improve responsiveness if the CPU or GPU are under high load.

If Jay is started with the environment variable `JAY_NO_REALTIME=1` or a
`config.so` exists, then Jay will not elevate its scheduler but will still
create elevated Vulkan queues.

Jay will drop all capabilities almost immediately after being started. Before
that, it will spawn a dedicated thread that retains the CAP_SYS_NICE capability
to create elevated Vulkan queues later.

If Jay has elevated its scheduler to SCHED_RR, then it will refuse to load
`config.so` configurations. Otherwise unprivileged applications would be able
to run arbitrary code with SCHED_RR by crafting a dedicated `config.so`. This
behavior can be overridden by compiling Jay with
`JAY_ALLOW_REALTIME_CONFIG_SO=1`.

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
