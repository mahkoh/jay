# Env Variables Read by Jay

This chapter lists the environment variables that Jay reads. Most of them are
standard variables that you will already have set in a normal desktop session;
the `JAY_*` variables are Jay-specific and are only needed for advanced tuning
or debugging.

> [!NOTE]
> This page is about variables that influence Jay's *own* behavior. To set
> variables for the programs that Jay launches, see
> [Environment Variables](configuration/environment.md) in the configuration
> section.

## The `jay.env` file

Instead of exporting these variables in your session, you can set them in a
`jay.env` file inside Jay's [configuration directory](configuration/index.md),
for example `~/.config/jay/jay.env`. This is convenient in a Wayland session,
where the compositor is often launched directly by a display manager and there
is no obvious place to export variables beforehand.

The file is read by both the compositor and the `jay` command-line tools.

```shell
# ~/.config/jay/jay.env
JAY_LOG_LEVEL=debug
JAY_VULKAN_VALIDATION=1
XCURSOR_THEME=Adwaita
XCURSOR_SIZE=32
```

The format is one `KEY=value` assignment per line:

- Blank lines and lines beginning with `#` are ignored, as is a trailing
  `# comment` on an assignment.
- Values may be wrapped in single quotes, double quotes, or backticks.

> [!IMPORTANT]
> A variable set in the real environment always takes precedence over the same
> variable in `jay.env`. And because `jay.env` lives *inside* the configuration
> directory, the variables that locate that directory in the first place
> (`JAY_CONFIG_DIR`, `XDG_CONFIG_HOME`, and `HOME`) must be set in the real
> environment; setting them in `jay.env` does not change where the directory is
> found.

## Session and directories

`JAY_CONFIG_DIR`
: Overrides the configuration directory. Takes precedence over
  `XDG_CONFIG_HOME` and `HOME`. Can also be set per-invocation with the global
  `--config-dir` command-line option.

`HOME`
: Used as a fallback for several paths: the configuration directory
  (`$HOME/.config/jay`) when `XDG_CONFIG_HOME` is not set, the default cursor
  search path, and the X authority file (`$HOME/.Xauthority`).

`XDG_CONFIG_HOME`
: Location of the configuration directory. Jay loads its config from
  `$XDG_CONFIG_HOME/jay`. Takes precedence over `HOME`, but is itself
  overridden by `JAY_CONFIG_DIR`.

`XDG_RUNTIME_DIR`
: Directory for the session's runtime sockets. Jay creates its Wayland socket
  here, and locates the session's D-Bus and PipeWire sockets here in order to
  connect to them. This must be set for Jay and its tools to run.

`XDG_SESSION_ID`
: Identifies your logind session. Jay uses it to acquire the seat and manage
  virtual terminal switching when running on hardware (the DRM backend).

`DBUS_SESSION_BUS_ADDRESS`
: Address of the D-Bus session bus. If unset, Jay falls back to
  `$XDG_RUNTIME_DIR/bus`.

`SHELL`
: Used by the `exec` action when a command is run through a shell.

## Logging

`JAY_LOG_LEVEL`
: The initial log level of the compositor and the command-line tools. One of
  `trace`, `debug`, `info`, `warn`, `error`, or `off`. Defaults to `info`. The
  global `--log-level` command-line option, when given, takes precedence over
  this variable.

## Cursor theme

These standard Xcursor variables select the cursor theme used by the
compositor.

`XCURSOR_THEME`
: Name of the cursor theme to load.

`XCURSOR_SIZE`
: Cursor size in pixels. Defaults to `24` if unset or unparseable.

`XCURSOR_PATH`
: Colon-separated list of directories to search for cursor themes. Defaults to
  `~/.icons:/usr/share/icons:/usr/share/pixmaps:/usr/X11R6/lib/X11/icons`.

## Running nested inside an X server

When Jay runs as a window inside an existing X session (the X backend), it
connects to that X server using:

`DISPLAY`
: The X server to connect to (for example `:0`).

`XAUTHORITY`
: Path to the X authority file used to authenticate with that X server. Falls
  back to `$HOME/.Xauthority`.

## Command-line tools

`WAYLAND_DISPLAY`
: Name of the Wayland socket of the running compositor. The `jay` command-line
  tools (and `jay run-privileged`) use it to connect to the compositor.

## Advanced tuning and debugging

These variables are not needed for normal use. They exist for tuning behavior
on unusual hardware or for diagnosing problems.

> [!NOTE]
> The boolean `JAY_*` variables below are enabled by `1`, `true`, `on`, or
> `yes` (case-insensitive); any other value leaves them disabled.

`JAY_PRIME_METHODS`
: Comma-separated list controlling which methods Jay uses to copy the
  composited frame buffers between GPUs on multi-GPU systems. This only affects
  the frame buffers that are scanned out to a monitor, not other buffers.
  Recognized values are `direct-pull`, `indirect-pull`, `direct-push`,
  `direct-sampling`, and `udmabuf`. Listed methods are tried first, in the
  order given; prefix a name with `-` to disable it. Any methods not mentioned
  remain enabled and are tried afterwards.

`JAY_NO_CLIENT_PRIME`
: Set to `1` to disable PRIME copies of client buffers. When an application
  submits a buffer that lives on a different GPU than the one Jay renders with
  (or that Jay cannot otherwise import directly), Jay normally copies it onto a
  usable buffer so that it can be displayed. Setting this variable disables
  those copies, which is mainly useful for diagnosing problems with the copy
  path. This only affects client buffers; the copies of the composited frame
  buffers that are scanned out to a monitor are controlled by
  `JAY_PRIME_METHODS`.

`JAY_NO_REALTIME`
: Set to `1` to prevent Jay from requesting real-time scheduling priority.

`JAY_VULKAN_VALIDATION`
: Set to `1` to enable the Vulkan validation layers. Useful when debugging
  rendering issues with the Vulkan renderer.

`JAY_NO_DESCRIPTOR_HEAP`
: Set to `1` to disable the use of Vulkan descriptor heaps, even on devices
  that support them.
