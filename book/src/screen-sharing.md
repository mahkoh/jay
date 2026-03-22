# Screen Sharing

Jay supports screen sharing via
[xdg-desktop-portal](https://github.com/flatpak/xdg-desktop-portal). Three
capture types are available:

- **Window capture** -- share a single window.
- **Output capture** -- share an entire monitor.
- **Workspace capture** -- like output capture, but only a single workspace is
  shown.

## Requirements

[PipeWire](https://pipewire.org/) must be installed and running. Verify with:

```shell
~$ systemctl --user status pipewire
```

## Portal Setup

Jay implements its own portal backend for the `ScreenCast` and `RemoteDesktop`
interfaces. Two configuration files must be installed so that
`xdg-desktop-portal` knows to use Jay's backend.

### If the Repository is Checked Out

```shell
~$ sudo cp etc/jay.portal /usr/share/xdg-desktop-portal/portals/jay.portal
~$ sudo cp etc/jay-portals.conf /usr/share/xdg-desktop-portal/jay-portals.conf
```

### If Installed via cargo install

Create the files manually:

```shell
~$ sudo tee /usr/share/xdg-desktop-portal/portals/jay.portal > /dev/null << 'EOF'
[portal]
DBusName=org.freedesktop.impl.portal.desktop.jay
Interfaces=org.freedesktop.impl.portal.ScreenCast;org.freedesktop.impl.portal.RemoteDesktop;
EOF
```

```shell
~$ sudo tee /usr/share/xdg-desktop-portal/jay-portals.conf > /dev/null << 'EOF'
[preferred]
default=gtk
org.freedesktop.impl.portal.ScreenCast=jay
org.freedesktop.impl.portal.RemoteDesktop=jay
org.freedesktop.impl.portal.Inhibit=none
org.freedesktop.impl.portal.FileChooser=gtk4
EOF
```

### Restart the Portal

After installing the files, restart the portal service:

```shell
~$ systemctl --user restart xdg-desktop-portal
```

## Configuration

### workspace-capture

The top-level `workspace-capture` setting controls whether newly created
workspaces can be captured via workspace capture. The default is `true`:

```toml
workspace-capture = false
```

Set this to `false` if you want to prevent workspace-level capture by default.

### Capture Indicator Colors

When a window is being recorded, its title bar color changes to make the
capture visually obvious. You can customize these colors in the `[theme]`
table:

```toml
[theme]
captured-focused-title-bg-color = "#900000"
captured-unfocused-title-bg-color = "#5f0000"
```

- `captured-focused-title-bg-color` -- background color of focused title bars
  that are being recorded.
- `captured-unfocused-title-bg-color` -- background color of unfocused title
  bars that are being recorded.

## The jay portal Command

Jay's portal backend is normally started automatically when a screen-sharing
request comes in via D-Bus activation. If you need to start it manually for
debugging purposes:

```shell
~$ jay portal
```

## Troubleshooting

If screen sharing does not work:

1. Verify PipeWire is running: `systemctl --user status pipewire`
2. Verify the portal files are installed in `/usr/share/xdg-desktop-portal/`.
3. Restart the portal: `systemctl --user restart xdg-desktop-portal`
4. Check the Jay log for errors: `jay log`
