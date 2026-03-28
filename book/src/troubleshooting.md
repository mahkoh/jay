# Troubleshooting

## Jay doesn't start / black screen

Jay requires **at least one working renderer** -- without one, no GPU can be
initialized and nothing will be displayed. It supports Vulkan (via libvulkan
plus a GPU-specific driver) and OpenGL (via libEGL and libGLESv2). These
libraries are loaded at runtime, not linked at build time. Vulkan is the
primary renderer and should be used whenever possible; the OpenGL renderer is
maintained only for backwards compatibility.

Check that the required libraries are installed:

```shell
~$ ls /usr/lib/libEGL.so* /usr/lib/libGLESv2.so* /usr/lib/libvulkan.so* 2>/dev/null
```

> [!NOTE]
> On some distributions (e.g. Fedora, Debian), 64-bit libraries live in
> `/usr/lib64/` or `/usr/lib/x86_64-linux-gnu/` instead.

If nothing is listed, install the appropriate packages. See the
[Installation](installation.md) chapter for package names by distribution.

**Nvidia users:**

- You need **Linux 6.7 or later** for explicit sync support, which is required
  for Nvidia GPUs.
- Make sure the Nvidia Vulkan ICD (Installable Client Driver) is installed.
  Jay's Vulkan renderer is the recommended option on Nvidia hardware.

## No applications open

The built-in default configuration binds:

- `Super_L` (left Windows key) -- open [Alacritty](https://alacritty.org/).
- `alt-p` -- open [bemenu](https://github.com/Cloudef/bemenu) as an
  application launcher.

If neither application is installed, nothing will happen when you press these
keys. Either install them or create a custom configuration with your preferred
applications:

```shell
~$ jay config init
```

Then edit `~/.config/jay/config.toml` to change the terminal and launcher
bindings.

> [!IMPORTANT]
> If you created a config file manually (e.g. by touching an empty file), it
> will have **no shortcuts at all**. Jay replaces the entire built-in default
> when any config file exists. Always use `jay config init` to start with a
> working configuration.

## Application doesn't have access to a protocol

Jay splits Wayland protocols into unprivileged and privileged. By default,
applications only have access to unprivileged protocols. If a program like a
screen locker, status bar, clipboard manager, or screen-capture tool is not
working, it likely needs access to one or more privileged protocols.

Common symptoms include:

- **swaylock** does nothing or fails to lock the screen (needs `session-lock`).
- **waybar** or **i3bar** shows no workspace information (needs
  `foreign-toplevel-list`).
- **wl-copy** / **cliphist** cannot access the clipboard (needs
  `data-control`).
- **grim** or **slurp** cannot capture the screen (needs `screencopy`).

**Quick fix -- grant all privileges:**

The simplest approach is to launch the program with full access to all
privileged protocols. In your config, set `privileged = true` in the exec
action:

```toml
on-idle = {
    type = "exec",
    exec = {
        prog = "swaylock",
        privileged = true,
    },
}
```

Or from the command line:

```shell
~$ jay run-privileged waybar
```

**Better fix -- grant only the capabilities needed:**

Use a client rule to grant specific capabilities:

```toml
[[clients]]
match.comm = "waybar"
capabilities = ["layer-shell", "foreign-toplevel-list"]
```

See [Granting Privileges](window-rules.md#granting-privileges) for the full
list of capabilities and more advanced approaches using connection tags.

## Wrong keyboard layout

The default keyboard layout is US QWERTY. To change it:

**Option 1 -- edit the config:**

```shell
~$ jay config init
```

Then edit the `keymap.rmlvo` section in `~/.config/jay/config.toml`:

```toml
[keymap.rmlvo]
layout = "de"
```

**Option 2 -- change it at runtime:**

```shell
~$ jay input seat default set-keymap-from-names -l de
~$ jay input seat default set-keymap-from-names -l us -v intl
```

This takes effect immediately but does not persist across restarts unless
configured in the config file.

## Screen sharing doesn't work

Screen sharing requires PipeWire and the Jay desktop portal.

**1. Check that PipeWire is running:**

```shell
~$ systemctl --user status pipewire
```

If it is not running, start it:

```shell
~$ systemctl --user start pipewire
```

**2. Check that the portal files are installed:**

Jay needs two files to be found by the XDG desktop portal framework:

- A portal definition file (e.g. `/usr/share/xdg-desktop-portal/portals/jay.portal`).
- A portal configuration file (e.g. `/usr/share/xdg-desktop-portal/jay-portals.conf`).

These files are included in the Jay repository under `etc/`. If you built Jay
from source and did not install them, copy them manually:

```shell
~$ sudo cp etc/jay.portal /usr/share/xdg-desktop-portal/portals/
~$ sudo cp etc/jay-portals.conf /usr/share/xdg-desktop-portal/
```

**3. Restart the portal:**

```shell
~$ systemctl --user restart xdg-desktop-portal
```

See the [Screen Sharing](screen-sharing.md) chapter for more details.

## X11 applications don't work

Jay uses Xwayland to run X11 applications.

**1. Install Xwayland:**

- Arch Linux: `sudo pacman -S xorg-xwayland`
- Fedora: `sudo dnf install xorg-x11-server-Xwayland`
- Debian/Ubuntu: `sudo apt install xwayland`

**2. Check your configuration:**

If you have a config file, make sure Xwayland is not explicitly disabled:

```toml
[xwayland]
# Make sure this is not set to false:
# enabled = false
```

Xwayland is enabled by default. You can also check its status at runtime:

```shell
~$ jay xwayland status
```

## Display manager doesn't show Jay

**1. Check that the session file exists:**

```shell
~$ ls /usr/share/wayland-sessions/jay.desktop
```

If it does not exist, create it:

```shell
~$ sudo tee /usr/share/wayland-sessions/jay.desktop > /dev/null << 'EOF'
[Desktop Entry]
Name=Jay
Comment=A Wayland Compositor
Exec=jay run
Type=Application
DesktopNames=jay
EOF
```

**2. Check that `jay` is in the system PATH:**

If you installed Jay via `cargo install`, the binary is at `~/.cargo/bin/jay`.
Display managers typically do not include `~/.cargo/bin` in their PATH. Either:

- Add `~/.cargo/bin` to the system PATH, or
- Create a symlink:

```shell
~$ sudo ln -s ~/.cargo/bin/jay /usr/local/bin/jay
```

## How to check logs

Open the log file in a pager:

```shell
~$ jay log
```

Follow the log in real time (like `tail -f`):

```shell
~$ jay log -f
```

Print just the log file path:

```shell
~$ jay log --path
```

Increase log verbosity at runtime:

```shell
~$ jay set-log-level debug
```

To set the log level at startup, add it to your config:

```toml
log-level = "debug"
```

> [!NOTE]
> The `log-level` config setting is read at startup and cannot be changed by
> reloading the configuration. Use `jay set-log-level` for runtime changes.

To automatically clean up old log files, see
[Log File Cleanup](configuration/misc.md#log-file-cleanup).

## Performance issues

If you experience dropped frames, stuttering, or high latency, try the
following:

**1. Use the Vulkan renderer:**

The Vulkan renderer is generally faster and supports more features (e.g. HDR,
direct scanout). Check your current API and switch if needed:

```shell
~$ jay randr show
~$ jay randr card card0 api vulkan
```

**2. Set `CAP_SYS_NICE`:**

Granting `CAP_SYS_NICE` allows Jay to use real-time scheduling and high-priority
Vulkan queues, which improves responsiveness under load:

```shell
~$ sudo setcap cap_sys_nice=p $(which jay)
```

> You need to re-run this command each time you update the Jay binary.

**3. Adjust the flip margin:**

The flip margin controls the time between initiating a page flip and the
display's vblank. A smaller margin reduces input latency but risks missed
frames. The default is 1.5 ms:

```shell
~$ jay randr card card0 timing set-flip-margin 1.5
```

If you see missed frames, try increasing it. If you want lower latency, try
decreasing it -- Jay will dynamically increase it if the margin is too small.

**4. Enable direct scanout:**

Direct scanout allows fullscreen applications to bypass the compositor's
rendering pipeline entirely, reducing both latency and GPU usage:

```shell
~$ jay randr card card0 direct-scanout enable
```
