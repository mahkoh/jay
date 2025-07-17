# Window and Client Rules

Jay supports powerful window and client rules similar to i3.

## Example

```toml
# Move spotify to workspace 3 and fullscreen it.
[[windows]]
match.client.sandbox-app-id = "com.spotify.Client"
action = [
    { type = "move-to-workspace", name = "3" },
    "enter-fullscreen",
]

# Spawn the Chromium screen sharing window, the GIMP splash screen, and the
# JetBrains splash screen floating and without focus stealing.
[[windows]]
match.any = [
    { title-regex = 'is sharing (your screen|a window)\.$', client.comm = "chromium" },
    { title = "GIMP Startup", app-id = "gimp" },
    { title = "splash", x-class-regex = "^jetbrains-(clion|rustrover)$" }
]
initial-tile-state = "floating"
auto-focus = false

# Spawn the JetBrains project selector floating.
[[windows]]
match.title-regex = "^Welcome to (RustRover|CLion)$"
match.x-class-regex = "^jetbrains-(clion|rustrover)$"
initial-tile-state = "floating"
```

## General Principles

Each rule consists of three components:

1. Criteria that determine which clients/windows the rule applies to.
2. An action to execute when a client/window starts matching the rule.
3. An action to execute when a client/window stops matching the rule.

Each rule can be assigned a name which allows other rules to refer to it.

Additionally, rules have ad-hoc properties for things that are not easily
expressed via actions, such as whether a window should be mapped floating or
tiled.

```toml
[[windows]]
name = "..."       # the rule name
match = { }        # the rule criteria
action = "..."     # the action to run on start
latch = "..."      # the action to run on stop
```

Rules are re-evaluated whenever any of the referenced criteria changes. That is,
if you have the following rule

```toml
[[windows]]
match.title = "VIM"
action = "enter-fullscreen"
```

then the window will enter fullscreen whenever title changes from something that
is not `VIM` to `VIM`. For window rules, if you only want to match windows that
have just been mapped, you can set the `just-mapped` criterion to `true`:

```toml
[[windows]]
match.title = "VIM"
match.just-mapped = true
action = "enter-fullscreen"
```

This is similar to the `initial-title` criterion found in some other
compositors.

Rules can trigger each other. For example:

```toml
[[windows]]
match.fullscreen = false
action = "enter-fullscreen"

[[windows]]
match.fullscreen = true
action = "exit-fullscreen"
```

This causes an infinite repetition of switching between windowed and fullscreen.
Jay prevents such loops from locking up the compositor by never performing more
than 1000 action callbacks before yielding to other work. However, they will
still cause the compositor to use 100% CPU and will likely cause affected
clients to be killed, since they won't be able to receive wayland messages fast
enough.

## Combining Criteria

Criteria can be combined with the following operations:

- `any` - match if any of a number of criteria match
- `all` - match if all of a number of criteria match
- `not` - match if a criterion does not match
- `exactly` - match if an exact number of criteria match
- `name` - match if another window rule with that name matches

```toml
# match windows that have the title `chromium` or `spotify`
match.any = [
    { title = "chromium" },
    { title = "spotify" },
]

# match windows whose title match both `chro` and `mium`
match.all = [
    { title-regex = "chro" },
    { title-regex = "mium" },
]

# match windows whose title is not `firefox`
match.not.title = "firefox"

# match windows whose title is `VIM` or whose clients are sandboxed, but not
# both
match.exactly.num = 1
match.exactly.list = [
    { title = "VIM" },
    { client.sandboxed = true },
]

# match if another rule called `another-rule-name` matches
match.name = "another-rule-name"
```

A criterion object has multiple fields, for example

```toml
match.title = "abc"
match.app-id = "xyz"
```

These fields are implicitly combined with `all` operator. That is, this behaves
just like

```toml
match.all = [
    { title = "abc" },
    { app-id = "xyz" },
]
```

## Finding Criteria Values

To determine which values to use in criteria, the `jay` executable provides the
subcommands `jay clients` and `jay tree` to inspect currently active clients and
open windows. For example

```text
~$ jay tree query select-window
- xdg-toplevel:
    id: 258ae697663a1b8abc7e4da9570ad36f
    pos: 1920x36 + 1920x1044
    client:
      id: 15
      uid: 1000
      pid: 2159136
      comm: chromium
      exe: /usr/lib/chromium/chromium
    title: YouTube - Chromium
    app-id: chromium
    workspace: 2
    visible
```

In this case, `select-window` allows you to interactively select a window and
then prints its properties.

## Client Rules

```toml
# start executable `b` whenever a client with executable `A` connects
[[clients]]
match.exe = "A"
action = { type = "exec", exec = "b" }
```

All properties that can be referred to in client criteria are currently
constant over the lifetime of the client.

### Client Criteria

The full specification of client criteria can be found in
[spec.generated.md](../toml-spec/spec/spec.generated.md).

- `sandboxed` - Matches clients that are/aren't sandboxed.
- `sandbox-engine`, `sandbox-engine-regex` - Matches the sandbox engine that was
  used to wrap this client. Usually `org.flatpak`.
- `sandbox-app-id`, `sandbox-app-id-regex` - Matches the app-id provided by the
  sandbox engine
- `sandbox-instance-id-id`, `sandbox-instance-id-regex` - Matches the
  instance-id provided by the sandbox engine
- `uid`, `pid` - Matches the UID/PID of the client.
- `is-xwayland` - Matches if the client is/isn't Xwayland.
- `comm`, `comm-regex` - Matches the `/proc/self/comm` of the client.
- `exe`, `exe-regex` - Matches the `/proc/self/exe` of the client.

## Window Rules

## Ad-hoc Window Rules

Rule actions are evaluated asynchronously. For window rules, this means that
they are evaluated after the window has been mapped but before it is displayed
for the first time. This makes them ill-suited for things that need to be fixed
during the mapping process. Ad-hoc window rules can be used to bridge this gap:

```toml
[[windows]]
match.title = "chromium"
initial-tile-state = "floating"
auto-focus = false
```

The `initial-tile-state` rule can be used to define whether the window is mapped
tiled or floating. If no such rule exists, this is determined via heuristics.
If multiple such rules exist and match a window, the compositor picks one at
random.

The `auto-focus` rule determines if the window is automatically focused when it
is mapped. If no such rule exists, newly mapped windows always get the keyboard
focus except in some cases involving Xwayland. If multiple such rules exist and
match a window, then the window _does not_ get the focus if _any_ of them is set
to `false`.

## Window Criteria

The full specification of window criteria can be found in
[spec.generated.md](../toml-spec/spec/spec.generated.md).

- `types` - Matches the type of a window. Currently there are four types:
  containers, placeholders, xdg toplevels, and X windows. If the rule does not
  contain such a criterion, the rule will only match windows created by clients,
  that is, xdg toplevels and X windows.
- `client` - This is a client criterion. See above.
- `title`, `title-regex` - Matches the title of the window.
- `app-id`, `app-id-regex` - Matches the XDG app-id of the window.
- `floating` - Matches if the window is/isn't floating.
- `visible` - Matches if the window is/isn't visible.
- `urgent` - Matches if the window wants/doesn't want attentions.
- `focused` - Matches if the window is/isn't focused.
- `fullscreen` - Matches if the window is/isn't fullscreen.
- `just-mapped` - Matches if the window has/hasn't just been mapped. This is
  true for a single frame after the window has been mapped.
- `tag`, `tag-regex` - Matches the XDG toplevel tag of the window.
- `x-class`, `x-class-regex` - Matches the X class of the window.
- `x-instance`, `x-instance-regex` - Matches the X instance of the window.
- `x-role`, `x-role-regex` - Matches the X role of the window.
- `workspace`, `workspace-regex` - Matches the workspace of the window.
- `content-types` - Matches the content type of a window. Currently there are
  three types: photos, videos, and games.
