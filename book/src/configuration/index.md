# Configuration Overview

Jay is configured through a single TOML file located at:

```
~/.config/jay/config.toml
```

If this file does not exist, Jay uses built-in defaults that provide a
reasonable starting configuration with common shortcuts, a US QWERTY keymap,
and other sensible settings.

> [!WARNING]
> Once `config.toml` exists, the **entire** built-in default configuration is
> replaced -- not merged. Even a completely empty file means no shortcuts, no
> startup actions, nothing. Always start from a full config rather than writing
> one from scratch.

## Initializing the config

The easiest way to get started is to let Jay write the defaults for you:

```shell
~$ jay config init
```

This creates `~/.config/jay/config.toml` pre-populated with the full default
configuration. You can then edit it to suit your needs.

If you already have a config file and want to reset it:

```shell
~$ jay config init --overwrite
```

The old file will be backed up to `config.toml.1` (or `.2`, `.3`, etc.) before
being replaced.

## Other config subcommands

Print the path to the config file:

```shell
~$ jay config path
```

Open the config directory in your file manager:

```shell
~$ jay config open-dir
```

## Reloading the configuration

By default, Jay does not automatically reload `config.toml` when it changes on
disk. To apply changes, trigger a reload manually:

- Press `alt-shift-r` (the default shortcut), or
- Use the `reload-config-toml` action in any other action context (e.g. a named
  action or a window rule).

Most settings take effect immediately on reload. A few exceptions (like
`log-level`, `explicit-sync`, and initial `drm-devices` settings) only apply
at compositor startup.

### Automatic reloading

To have Jay watch `config.toml` for changes and reload automatically, add:

```toml
auto-reload = true
```

When enabled, Jay uses inotify to monitor the config file and its parent
directories. Changes are debounced — the config is reloaded 400 ms after the
last write, so rapid successive saves don't cause multiple reloads. If the file
contents haven't actually changed, the reload is skipped.

Setting `auto-reload = false` will stop the watcher. Removing the key entirely
leaves the watcher state unchanged (if it was running, it keeps running until
the compositor restarts).

## Named actions

You can define reusable actions in the `[actions]` table and reference them
anywhere an action is accepted by prefixing the name with `$`:

```toml
[actions]
launch-terminal = { type = "exec", exec = "alacritty" }
launch-browser = { type = "exec", exec = "firefox" }

[shortcuts]
alt-Return = "$launch-terminal"
alt-b = "$launch-browser"
```

Named actions can reference other named actions. The `max-action-depth` setting
controls the maximum recursion depth to prevent infinite loops (default: 16):

```toml
max-action-depth = 32
```

## Composable actions

Anywhere an action is accepted, you can use an **array of actions** instead.
This applies to shortcuts, startup hooks, named actions, and any other action
field:

```toml
[shortcuts]
alt-q = [
    { type = "exec", exec = ["notify-send", "Goodbye!"] },
    "quit",
]
```

## Advanced: shared library configuration

For users who need programmatic configuration beyond what TOML offers, Jay also
supports configuration via a compiled Rust shared library using the
[jay-config](https://docs.rs/jay-config) crate. This is an advanced option --
the TOML config is sufficient for the vast majority of use cases.

## Full specification

This book covers the most common configuration options with explanations and
examples. For an exhaustive listing of every field, type, and action, see the
[auto-generated specification](https://github.com/mahkoh/jay/blob/master/toml-spec/spec/spec.generated.md).
