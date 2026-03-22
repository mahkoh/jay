# Startup Actions

Jay provides hooks that run actions at specific points during compositor
startup and during idle transitions.

## on-graphics-initialized

This hook runs after the GPU has been initialized and the compositor is ready
to display graphical content. It is the right place to start **graphical
applications** such as notification daemons, system tray bridges, status bars,
and similar programs.

```toml
on-graphics-initialized = { type = "exec", exec = "mako" }
```

To start multiple programs, use an array of actions:

```toml
on-graphics-initialized = [
    { type = "exec", exec = "mako" },
    { type = "exec", exec = "wl-tray-bridge" },
]
```

> [!NOTE]
> The built-in default configuration starts [mako](https://github.com/emersion/mako)
> (notification daemon) and [wl-tray-bridge](https://github.com/mahkoh/wl-tray-bridge)
> (system tray bridge) in `on-graphics-initialized`. Once you create a config
> file, these defaults are replaced -- include them in your config if you want
> to keep them.

This hook runs when the config is first loaded after compositor startup. It
does **not** re-run on config reload.

## on-startup

This hook runs as early as possible when the compositor starts -- **before**
graphics are initialized. Do not start graphical applications here; they will
likely fail to connect to the display.

Use `on-startup` for tasks like setting environment variables or other
non-graphical initialization:

```toml
on-startup = {
    type = "set-env",
    env = { XDG_CURRENT_DESKTOP = "jay" },
}
```

This hook has **no effect on config reload** -- it only runs once when the
compositor first starts.

## on-idle

This hook runs when the compositor transitions to the idle state (i.e., after
the configured idle timeout expires with no user input). The most common use
case is starting a screen locker.

```toml
on-idle = {
    type = "exec",
    exec = {
        prog = "swaylock",
        privileged = true,
    },
}
```

> [!NOTE]
> Screen lockers need `privileged = true` to access the privileged Wayland
> protocols required for locking the session.

You can combine idle with a grace period. The idle timeout and grace period are
configured separately in the `[idle]` section (see [Idle & Screen
Locking](idle.md)):

```toml
idle = { minutes = 10 }

on-idle = {
    type = "exec",
    exec = {
        prog = "swaylock",
        privileged = true,
    },
}
```

Like the other hooks, `on-idle` accepts arrays of actions:

```toml
on-idle = [
    { type = "exec", exec = ["notify-send", "Going idle..."] },
    {
        type = "exec",
        exec = {
            prog = "swaylock",
            privileged = true,
        },
    },
]
```
