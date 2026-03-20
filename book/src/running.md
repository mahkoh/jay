# Running Jay

## From a Virtual Terminal

Switch to a virtual terminal (e.g. `ctrl-alt-F2`), log in, and run:

```shell
~$ jay run
```

## From a Display Manager

To make Jay appear as a session option in your display manager (GDM, SDDM,
etc.), install the session file.

If you have the repository checked out:

```shell
~$ sudo cp etc/jay.desktop /usr/share/wayland-sessions/jay.desktop
```

Otherwise, create it manually:

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

Then log out and select **Jay** from the session list.

> [!NOTE]
> If you installed Jay via `cargo install`, the `jay` binary lives in
> `~/.cargo/bin/`. Your display manager may not include this directory in its
> `PATH`. Either add `~/.cargo/bin` to the system `PATH`, or copy/symlink the
> binary to `/usr/local/bin`:
>
> ```shell
> ~$ sudo ln -s ~/.cargo/bin/jay /usr/local/bin/jay
> ```

## The Control Center

Once Jay is running, press `alt-c` to open the
[control center](control-center.md) -- a built-in GUI that lets you inspect and
modify most compositor settings without editing config files or running CLI
commands. From the control center you can:

- Rearrange monitors with a visual drag-and-drop editor.
- Configure input devices -- acceleration, tap behavior, keymaps, and more.
- Switch GPUs and graphics APIs.
- Adjust theme colors, fonts, borders, and gaps with live color pickers.
- Manage idle timeouts and screen locking.
- Search and filter windows and clients.
- Toggle Xwayland and color management.

You can also open it from the command line with `jay control-center`. See the
[Control Center](control-center.md) chapter for a full tour of every pane.

## Default Keybindings

Jay ships with a built-in default configuration. The most important default
keybindings are listed below.

> [!NOTE]
> [Alacritty](https://alacritty.org/) and
> [bemenu](https://github.com/Cloudef/bemenu) must be installed for the default
> terminal and launcher bindings to work.

`Super_L` (left Windows key)
: Open Alacritty terminal

`alt-p`
: Open bemenu application launcher

`alt-q`
: Quit Jay

`alt-h` / `j` / `k` / `l`
: Move focus (left/down/up/right)

`alt-shift-h` / `j` / `k` / `l`
: Move focused window

`alt-d`
: Split horizontally

`alt-v`
: Split vertically

`alt-u`
: Toggle fullscreen

`alt-shift-f`
: Toggle floating

`alt-c`
: Open the control center

`alt-shift-c`
: Close focused window

`alt-t`
: Toggle split direction

`alt-m`
: Toggle mono (stacking) layout

`alt-f`
: Focus parent container

`alt-shift-r`
: Reload configuration

The defaults also include `ctrl-alt-F1` through `F12` for switching virtual
terminals, `alt-F1` through `F12` for switching workspaces, and
`alt-shift-F1` through `F12` for moving windows to workspaces.

Once you create a configuration file, the built-in defaults are entirely
replaced -- even an empty config file means no shortcuts. Run `jay config init`
to generate a config pre-populated with all the defaults. See the
[Configuration Overview](configuration/index.md) chapter for details.
