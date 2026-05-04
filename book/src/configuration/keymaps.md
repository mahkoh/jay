# Keymaps & Repeat Rate

Jay uses XKB keymaps for keyboard layout configuration. The default keymap is
US QWERTY.

## Setting the keymap

There are several ways to define a keymap.

### Using RMLVO names (recommended)

The simplest approach is to specify the layout using RMLVO (Rules, Model,
Layout, Variants, Options) names:

```toml
keymap.rmlvo = { layout = "de" }
```

You can specify any combination of RMLVO fields:

```toml
keymap.rmlvo = {
    layout = "us,de",
    variants = "dvorak,",
    options = "grp:ctrl_space_toggle",
}
```

All fields are optional. When a field is omitted, Jay checks the corresponding
environment variable, then falls back to a default:

| Field      | Environment Variable     | Default  |
|------------|--------------------------|----------|
| `rules`    | `XKB_DEFAULT_RULES`      | `evdev`  |
| `model`    | `XKB_DEFAULT_MODEL`      | `pc105`  |
| `layout`   | `XKB_DEFAULT_LAYOUT`     | `us`     |
| `variants` | `XKB_DEFAULT_VARIANTS`   | *(none)* |
| `options`  | `XKB_DEFAULT_OPTIONS`    | *(none)* |

### Using a raw XKB string

You can provide a complete XKB keymap as a multi-line string. See the
[ArchWiki XKB guide](https://wiki.archlinux.org/title/X_keyboard_extension)
for background on the format.

```toml
keymap = """
  xkb_keymap {
      xkb_keycodes { include "evdev+aliases(qwerty)" };
      xkb_types    { include "complete"              };
      xkb_compat   { include "complete"              };
      xkb_symbols  { include "pc+us+inet(evdev)"     };
  };
  """
```

### Loading from a file

Point to an XKB file. Relative paths are resolved from the config directory
(`~/.config/jay/`):

```toml
keymap.path = "./my-keymap.xkb"
```

## Named keymaps

You can define multiple named keymaps and switch between them at runtime.
Define them with the `[[keymaps]]` array, then select the default with
`keymap.name`:

```toml
keymap.name = "laptop"

[[keymaps]]
name = "laptop"
rmlvo = { layout = "us" }

[[keymaps]]
name = "external"
rmlvo = { layout = "de", options = "compose:ralt" }
```

Each entry in `[[keymaps]]` must have a `name` and exactly one of `map`,
`path`, or `rmlvo`.

### Switching keymaps at runtime

Use the `set-keymap` action to switch between named keymaps:

```toml
[shortcuts]
alt-F9  = { type = "set-keymap", keymap.name = "laptop" }
alt-F10 = { type = "set-keymap", keymap.name = "external" }
```

The `set-keymap` action also supports inline keymap definitions with
`shortcuts-group`:

```toml
[shortcuts]
alt-F11 = {
    type = "set-keymap",
    keymap.rmlvo = {
        layout = "us,ru",
        options = "grp:ctrl_space_toggle",
    },
    keymap.shortcuts-group = 0,
}
```

You can also switch keymaps from the command line:

```shell
~$ jay input seat default set-keymap-from-names --layout de
```

## Repeat rate

The repeat rate controls how keys behave when held down. It has two parameters:

- `rate` -- number of key repeats per second
- `delay` -- milliseconds to wait before repeating begins

```toml
repeat-rate = { rate = 25, delay = 250 }
```

### Changing repeat rate at runtime

Use the `set-repeat-rate` action:

```toml
[shortcuts]
alt-F11 = {
    type = "set-repeat-rate",
    rate = { rate = 40, delay = 200 },
}
```

Or from the command line:

```shell
~$ jay input seat default set-repeat-rate 40 200
```

## Shortcuts group

By default, Jay resolves shortcuts using the currently active keymap group.
This means that when you switch to a non-Latin layout (e.g. Russian), shortcuts
defined for Latin keysyms stop working because the active group produces
different keysyms.

The `shortcuts-group` option pins shortcut resolution to a specific group index,
regardless of which group is active for typing. This way, shortcuts always
resolve using the specified group while the active group is used normally for
text input.

```toml
keymap.rmlvo = {
    layout = "us,ru",
    options = "grp:ctrl_space_toggle",
}
keymap.shortcuts-group = 0
```

With this configuration, shortcuts always use group 0 (US) even when group 1
(Russian) is active for typing.

The value can be:
- An integer (0-based group index)
- The string `"active"` (the default -- use whichever group is currently active)

The `shortcuts-group` option can be used with any keymap definition method
(`map`, `path`, or `rmlvo`):

```toml
[[keymaps]]
name = "multilingual"
rmlvo = {
    layout = "us,de,ru",
    options = "grp:ctrl_space_toggle",
}
shortcuts-group = 0
```

## Per-device keymaps

You can override the keymap for specific input devices using the `[[inputs]]`
array. For example, to use a different layout for an external keyboard:

```toml
[[inputs]]
match.name = "My External Keyboard"
keymap.rmlvo = { layout = "de" }
```

See the [Input Devices](inputs.md) chapter for more on matching and configuring
individual devices.
