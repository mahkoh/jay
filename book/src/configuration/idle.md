# Idle & Screen Locking

Jay can detect when the system is idle and automatically run an action -- most
commonly launching a screen locker. The idle timeout, grace period, and
on-idle action are all configurable.

## Idle timeout

Set how long the system must be idle before the on-idle action fires. Specify
`minutes` and/or `seconds`:

```toml
idle.minutes = 10
```

```toml
idle = { minutes = 5, seconds = 30 }
```

If all values are explicitly set to 0, the idle timeout is disabled entirely:

```toml
idle = { minutes = 0 }
```

> [!NOTE]
> The idle timeout defined in `config.toml` cannot be changed by reloading the
> configuration. Use `jay idle` or the `configure-idle` action to change it at
> runtime.

## Grace period

The grace period is a warning phase between the timeout expiring and the actual
idle event. During the grace period, the screen goes black but outputs are not
yet disabled and the `on-idle` action has not yet fired. Any input during this
period cancels the idle transition.

The default grace period is 5 seconds. Configure it with:

```toml
idle.grace-period.seconds = 3
```

```toml
idle.grace-period = { minutes = 0, seconds = 10 }
```

Set both values to 0 to disable the grace period (immediately fire the on-idle
action when the timeout expires):

```toml
idle.grace-period = { seconds = 0 }
```

## On-idle action

The `on-idle` field defines what happens when the idle timeout (plus grace
period) elapses. The most common use is launching a screen locker:

```toml
on-idle = {
    type = "exec",
    exec = {
        prog = "swaylock",
        privileged = true,
    },
}
```

> [!IMPORTANT]
> Screen lockers that use the Wayland session lock protocol (like swaylock)
> need `privileged = true` in the exec configuration. This grants the process
> the necessary permissions to lock the session.

You can also combine multiple actions:

```toml
on-idle = [
    {
        type = "exec",
        exec = {
            prog = "swaylock",
            privileged = true,
        },
    },
    { type = "exec", exec = ["notify-send", "System locked"] },
]
```

## Complete example

A typical idle and screen-locking setup:

```toml
idle = {
    minutes = 10,
    grace-period = { seconds = 5 },
}
on-idle = {
    type = "exec",
    exec = {
        prog = "swaylock",
        privileged = true,
    },
}
```

This means:
1. After 10 minutes of inactivity, the screen goes black (grace period begins).
2. If no input occurs within 5 seconds, swaylock is launched and outputs are
   disabled.
3. Any input during the grace period cancels the transition and restores the
   display.

## Runtime changes

### Checking idle status

```shell
~$ jay idle status
```

### Changing the idle timeout

```shell
~$ jay idle set 5m
~$ jay idle set 1m30s
~$ jay idle set disabled
```

### Changing the grace period

```shell
~$ jay idle set-grace-period 10s
~$ jay idle set-grace-period 0s
```

### Duration format

The CLI accepts durations in a flexible format:

| Example              | Meaning                |
|----------------------|------------------------|
| `1m`                 | 1 minute               |
| `1m5s`               | 1 minute 5 seconds     |
| `1min 5sec`          | 1 minute 5 seconds     |
| `90s`                | 90 seconds             |
| `disabled`           | Disable the timeout    |

### Unlocking

If the compositor is locked (e.g. the screen locker crashed), you can unlock
it from another TTY or by SSH-ing into the machine. You must set
`WAYLAND_DISPLAY` to the socket of the Jay compositor, since you are running
the command outside the compositor session:

```shell
~$ WAYLAND_DISPLAY=wayland-1 jay unlock
```

Use `jay pid` with the same `WAYLAND_DISPLAY` value to verify you are
targeting the correct compositor instance.

### Using shortcuts

The `configure-idle` action lets you change idle settings from a keybinding:

```toml
[shortcuts]
alt-F9 = {
    type = "configure-idle",
    idle = {
        minutes = 5,
        grace-period = { seconds = 3 },
    },
}
alt-F10 = {
    type = "configure-idle",
    idle = { minutes = 0 },
}
```

## Full reference

For the exhaustive list of all idle-related fields and types, see the
[auto-generated specification](https://github.com/mahkoh/jay/blob/master/toml-spec/spec/spec.generated.md).
