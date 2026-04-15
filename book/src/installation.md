# Installation

## Compile-Time Dependencies

The following libraries must be installed before building Jay. They are linked
at build time.

| Library       | Arch Linux     | Fedora               | Debian / Ubuntu      |
|---------------|----------------|----------------------|----------------------|
| libinput      | `libinput`     | `libinput-devel`     | `libinput-dev`       |
| libgbm        | `mesa`         | `mesa-libgbm-devel`  | `libgbm-dev`         |
| libudev       | `systemd-libs` | `systemd-devel`      | `libudev-dev`        |
| libpangocairo | `pango`        | `pango-devel`        | `libpango1.0-dev`    |
| libfontconfig | `fontconfig`   | `fontconfig-devel`   | `libfontconfig-dev`  |

**One-liner install commands:**

Arch Linux:

```shell
~$ sudo pacman -S libinput mesa systemd-libs pango fontconfig
```

Fedora:

```shell
~$ sudo dnf install libinput-devel mesa-libgbm-devel systemd-devel pango-devel fontconfig-devel
```

Debian / Ubuntu:

```shell
~$ sudo apt install libinput-dev libgbm-dev libudev-dev libpango1.0-dev libfontconfig-dev
```

You also need a C compiler (GCC or Clang) and the latest stable version of
Rust. Install Rust with [rustup](https://rustup.rs/):

```shell
~$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Runtime Dependencies

These libraries are loaded dynamically at runtime. Jay requires **at least one**
working renderer -- without one, no GPU can be initialized and nothing will be
displayed.

| Library            | Purpose                       | Arch Linux           | Fedora                          | Debian / Ubuntu        |
|--------------------|-------------------------------|----------------------|---------------------------------|------------------------|
| libEGL + libGLESv2 | OpenGL renderer (legacy)      | `mesa` or `libglvnd` | `mesa-libEGL` + `mesa-libGLES`  | `libegl1` + `libgles2` |
| libvulkan          | Vulkan renderer (recommended) | `vulkan-icd-loader`  | `vulkan-loader`                 | `libvulkan1`           |

For Vulkan, you also need the driver for your GPU:

| GPU    | Arch Linux      | Fedora                | Debian / Ubuntu       |
|--------|-----------------|-----------------------|-----------------------|
| AMD    | `vulkan-radeon` | `mesa-vulkan-drivers` | `mesa-vulkan-drivers` |
| Intel  | `vulkan-intel`  | `mesa-vulkan-drivers` | `mesa-vulkan-drivers` |
| Nvidia | `nvidia-utils`  | `xorg-x11-drv-nvidia` | `nvidia-vulkan-icd`   |

### Optional Runtime Dependencies

- **Linux 6.7 or later** -- required for explicit sync (needed for Nvidia GPUs).
- **Xwayland** -- required for running X11 applications.
- **PipeWire** -- required for screen sharing.
- **logind** (part of systemd) -- required when running Jay from a virtual terminal or display manager.
- **libsqlite3** (`libsqlite3.so`) -- required for session management. Loaded
  from `sqlite` (Arch Linux), `sqlite-libs` (Fedora), or `libsqlite3-0` (Debian /
  Ubuntu).

## Building

### AUR (Arch Linux)

Arch Linux users can install Jay from the AUR. Two packages are available:

- **[jay](https://aur.archlinux.org/packages/jay)** -- builds the latest
  released version.
- **[jay-git](https://aur.archlinux.org/packages/jay-git)** -- builds from the
  latest commit on the master branch.

Install with your preferred AUR helper, for example:

```shell
~$ yay -S jay
```

The AUR packages handle all compile-time dependencies automatically.

### From crates.io (recommended)

```shell
~$ cargo install --locked jay-compositor
```

This installs the `jay` binary to `~/.cargo/bin/jay`.

### From git (latest development version)

```shell
~$ cargo install --locked --git https://github.com/mahkoh/jay.git jay-compositor
```

### From a local clone

```shell
~$ git clone https://github.com/mahkoh/jay.git
~$ cd jay
~$ cargo build --release
```

The binary is then available at `./target/release/jay`.

## CAP_SYS_NICE (Optional)

Granting `CAP_SYS_NICE` to the Jay binary can improve responsiveness when the
CPU or GPU are under heavy load:

```shell
~$ sudo setcap cap_sys_nice=p $(which jay)
```

When this capability is available, Jay will elevate its scheduler to `SCHED_RR`
(real-time round-robin) and create Vulkan queues with the highest available
priority. Both of these help Jay maintain smooth frame delivery under
contention.

Jay drops all capabilities almost immediately after startup. A dedicated thread
retains `CAP_SYS_NICE` solely for creating elevated Vulkan queues later.

> [!NOTE]
> You need to re-run the `setcap` command each time you update the Jay binary.

### SCHED_RR and config.so

Running untrusted code at real-time priority would be a security risk. To
prevent this, Jay restricts which `config.so` files can be loaded when the
scheduler has been elevated to `SCHED_RR`.

A `config.so` is considered **privileged** if it is owned by `root:root` and is
not group-writable or world-writable. Privileged config files are assumed to
come from a trusted source (e.g. a package manager) and are always allowed.

For unprivileged `config.so` files (any other ownership or with loose
permissions), Jay enforces mutual exclusion with `SCHED_RR`:

- If an unprivileged `config.so` exists in the config directory, Jay skips the
  `SCHED_RR` elevation (elevated Vulkan queues are still created).
- If Jay has already elevated to `SCHED_RR`, it refuses to load an unprivileged
  `config.so`.

You can also skip `SCHED_RR` explicitly by setting `JAY_NO_REALTIME=1`:

```shell
~$ JAY_NO_REALTIME=1 jay run
```

This still allows elevated Vulkan queues and does not affect `config.so`
loading.

The mutual exclusion can be overridden at compile time by building Jay with
`JAY_ALLOW_REALTIME_CONFIG_SO=1`.

## Recommended Applications

The following applications work well with Jay:

- **[Alacritty](https://alacritty.org/)** -- the default terminal emulator in the built-in configuration.
- **[bemenu](https://github.com/Cloudef/bemenu)** -- the default application launcher in the built-in configuration.
- **[xdg-desktop-portal-gtk4](https://github.com/mahkoh/xdg-desktop-portal-gtk4)** -- a file-picker portal with thumbnail support. Used automatically when installed.
- **[wl-tray-bridge](https://github.com/mahkoh/wl-tray-bridge)** -- shows D-Bus StatusNotifierItem applications as tray icons.
- **[mako](https://github.com/emersion/mako)** -- a notification daemon. Launched automatically by the default configuration.
- **[window-to-tray](https://github.com/mahkoh/wl-proxy/tree/master/apps/window-to-tray)** -- run most Wayland applications as tray applications (e.g. `window-to-tray pavucontrol-qt`).
