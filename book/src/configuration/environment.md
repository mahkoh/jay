# Environment Variables

Jay can set environment variables that are inherited by all programs it spawns.

## Setting environment variables at startup

Use the `[env]` table in your config to define variables that apply to every
application launched by the compositor:

```toml
[env]
GTK_THEME = "Adwaita:dark"
QT_QPA_PLATFORM = "wayland"
MOZ_ENABLE_WAYLAND = "1"
```

These variables are set when the config is loaded. Programs started after that
point will inherit them.

## Changing variables at runtime

### set-env

Use the `set-env` action to add or update environment variables while the
compositor is running. This affects all programs started **after** the action
runs:

```toml
[shortcuts]
alt-F11 = {
    type = "set-env",
    env = { GTK_THEME = "Adwaita:dark" },
}
alt-F12 = {
    type = "set-env",
    env = { GTK_THEME = "Adwaita" },
}
```

### unset-env

Use the `unset-env` action to remove environment variables:

```toml
[shortcuts]
alt-F10 = { type = "unset-env", env = ["GTK_THEME"] }
```

You can unset multiple variables at once with an array:

```toml
[shortcuts]
alt-F10 = {
    type = "unset-env",
    env = ["GTK_THEME", "QT_QPA_PLATFORM"],
}
```

## Per-process environment variables

When using the table form of the `exec` action, you can set environment
variables that apply only to that specific process:

```toml
[shortcuts]
alt-Return = {
    type = "exec",
    exec = {
        prog = "alacritty",
        env = { TERM = "xterm-256color" },
    },
}
```

These per-process variables are merged with (and override) the global
environment for that single execution.
