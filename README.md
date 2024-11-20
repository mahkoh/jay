# Jay

[![crates.io](https://img.shields.io/crates/v/jay-compositor.svg)](http://crates.io/crates/jay-compositor)

This a fork of [Jay](https://github.com/mahkoh/jay) adding a modal by window feature.

You have to adjust the config as follow :
- The Jay Global's is required, and serves to create Global shortcuts or tunnels.
- A Tunnel will replace the key pressed by a sequence of defined keys.
- When you are in an AppMod you can only use the shortcuts/tunnels of itself or of Jay Global
- All shortcut pointing on a command that does not exist will try create a tunnel

The config details from main repo (See [config.md](./docs/config.md).)

```toml
# The keymap that is used for shortcuts and also sent to clients.
keymap = """
    xkb_keymap {
        xkb_keycodes { include "evdev+aliases(azerty)" };
        xkb_types    { include "complete"              };
        xkb_compat   { include "complete"              };
        xkb_symbols  { include "pc+fr+inet(evdev)"     };
    };
    """


[[shortcuts]]
app_name = "Jay"
mod_name = "Global"

alt-q = "quit"
Super_L = "set_app_mod(Jay, Window)"
ctrl-alt-F1 = { type = "switch-to-vt", num = 1 }
ctrl-alt-F2 = { type = "switch-to-vt", num = 2 }
ctrl-alt-F3 = { type = "switch-to-vt", num = 3 }
ctrl-alt-F4 = { type = "switch-to-vt", num = 4 }
ctrl-alt-F5 = { type = "switch-to-vt", num = 5 }
ctrl-alt-F6 = { type = "switch-to-vt", num = 6 }
ctrl-alt-F7 = { type = "switch-to-vt", num = 7 }


[[shortcuts]]
app_name = "Jay"
mod_name = "Insert"

# Escape = "set_app_mod(, )"
ctrl-Escape = "set_app_mod(, )"


[[shortcuts]]
app_name = "Jay"
mod_name = "Window"

# The focus-X actions move the keyboard focus to next window on the X.
h = "focus-left"
j = "focus-down"
k = "focus-up"
l = "focus-right"

r = "reload-config-toml"

Return = { type = "exec", exec = "alacritty" }
t = { type = "exec", exec = "alacritty" }
i = "set_app_mod(,)"


[[shortcuts]]
app_name = "firefox-developer-edition"
mod_name = "Init"

# No command named "ctrl-t" so it tries to create a tunnel instead.
o = "ctrl-t"
h = "ctrl-shift-Tab"
l = "shift-Tab"
i = "set_app_mod(Jay, Insert)"



[[shortcuts]]
app_name = "Alacritty"
mod_name = "Init"

# Write firefox and press return
f = "f i r e f o x Return"
c = "ctrl-c"
d = "ctrl-d"
i = "set_app_mod(Jay, Insert)"
```
