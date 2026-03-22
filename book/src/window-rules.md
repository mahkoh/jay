# Window & Client Rules

Jay supports powerful, reactive rules for controlling how clients connect and
how windows behave. Rules are defined in the `[[clients]]` and `[[windows]]`
arrays in your configuration file.

## Client Rules

Client rules operate on Wayland clients (processes). They are defined with
`[[clients]]` entries:

```toml
[[clients]]
match.exe = "/usr/bin/firefox"
action = { type = "exec", exec = ["notify-send", "Firefox connected"] }
```

### Structure

Each client rule can have the following fields:

`name`
: A name for cross-referencing this rule from other rules.

`match`
: A `ClientMatch` specifying which clients this rule applies to.

`action`
: An action to run when a client starts matching.

`latch`
: An action to run when a client stops matching.

`capabilities`
: Wayland protocol access granted to matching clients.

`sandbox-bounding-capabilities`
: Upper bounds for protocols available to child sandboxes.

### Client Match Criteria

All client match criteria are constant over the lifetime of a client. If no
fields are set, all clients are matched. If multiple fields are set, they are
implicitly AND-combined.

`sandboxed`
: Whether the client is sandboxed (`true`/`false`).

`sandbox-engine` / `sandbox-engine-regex`
: The sandbox engine (e.g. `org.flatpak`).

`sandbox-app-id` / `sandbox-app-id-regex`
: The app ID provided by the sandbox.

`sandbox-instance-id` / `sandbox-instance-id-regex`
: The instance ID from the sandbox.

`uid`
: The user ID of the client.

`pid`
: The process ID of the client.

`is-xwayland`
: Whether the client is Xwayland (`true`/`false`).

`comm` / `comm-regex`
: The client's `/proc/pid/comm` value.

`exe` / `exe-regex`
: The client's `/proc/pid/exe` path.

`tag` / `tag-regex`
: The connection tag of the client.

### Granting Privileges

Jay splits Wayland protocols into unprivileged and privileged. By default,
applications only have access to unprivileged protocols. This means that tools
like screen lockers, status bars, screen-capture utilities, and clipboard
managers will not work unless you explicitly grant them the necessary
privileges.

See the [Protocol Support](features.md#protocol-support) table in the Features
chapter for the full list of protocols and whether they are privileged.

There are three ways to grant privileges, from simplest to most fine-grained.

#### 1. Grant all privileges via `privileged = true` (exec) or `jay run-privileged`

The simplest approach gives a program access to **all** privileged protocols.
This is appropriate for trusted tools like screen lockers where you don't want
to think about which specific protocols they need.

In the config, set `privileged = true` in the exec table:

```toml
on-idle = {
    type = "exec",
    exec = {
        prog = "swaylock",
        privileged = true,
    },
}
```

From the command line, use `jay run-privileged`:

```shell
~$ jay run-privileged waybar
```

Both methods connect the program to a privileged Wayland socket that grants
access to all privileged protocols.

#### 2. Grant capabilities via connection tags

Connection tags let you combine the CLI with client rules for precise control.
You tag a program at launch time, then write a client rule that matches
the tag and grants specific capabilities.

First, launch the program with a tag -- either from the command line:

```shell
~$ jay run-tagged bar waybar
```

Or from the config using the `tag` field in an exec action:

```toml
[shortcuts]
alt-w = {
    type = "exec",
    exec = {
        prog = "waybar",
        tag = "bar",
    },
}
```

Then write a client rule that matches the tag and grants capabilities:

```toml
[[clients]]
match.tag = "bar"
capabilities = ["layer-shell", "foreign-toplevel-list"]
```

This way, only the specific instance you launched with the tag receives the
privileges -- other programs with the same binary name do not.

Available capability values: `none`, `all`, `data-control`,
`virtual-keyboard`, `foreign-toplevel-list`, `idle-notifier`, `session-lock`,
`layer-shell`, `screencopy`, `seat-manager`, `drm-lease`, `input-method`,
`workspace-manager`, `foreign-toplevel-manager`, `head-manager`,
`gamma-control-manager`, `virtual-pointer`.

**Default capabilities:** unsandboxed clients receive `layer-shell` and
`drm-lease`. Sandboxed clients receive only `drm-lease`. If any client rule
matches, its capabilities **replace** the defaults entirely. If multiple rules
match, their capabilities are unioned together, but the defaults are not
included unless a matching rule also grants them.

#### 3. Grant capabilities via client match rules

Client rules can also match programs by properties like their executable name
instead of a tag. This is convenient when you always want a given program to
have certain capabilities, regardless of how it was launched:

```toml
[[clients]]
match.comm = "waybar"
capabilities = ["layer-shell", "foreign-toplevel-list"]

# Vim 9.2 uses the data-control protocol for seamless wayland integration.
[[clients]]
match.comm = "vim"
match.sandboxed = false
capabilities = "data-control"

# Older versions use wl-copy and wl-paste.
[[clients]]
match.any = [
    { comm = "wl-copy" },
    { comm = "wl-paste" },
]
match.sandboxed = false
capabilities = "data-control"
```

> [!NOTE]
> Client match criteria like `comm`, `exe`, and `pid` are checked when a
> client connects. Any process with a matching name receives the specified
> capabilities. If you need to restrict privileges to programs you launch
> yourself, use connection tags (method 2) instead.

#### Bounding capabilities (sandboxes)

Capabilities can never exceed the client's **bounding capabilities**. Use
`sandbox-bounding-capabilities` on a client rule to set the upper bound for
protocols available to sandboxes created by that client:

```toml
[[clients]]
match.comm = "flatpak-portal"
sandbox-bounding-capabilities = ["drm-lease", "layer-shell"]
```

## Window Rules

Window rules operate on individual windows. They are defined with `[[windows]]`
entries:

```toml
[[windows]]
match.app-id = "org.gnome.Nautilus"
initial-tile-state = "floating"
```

### Structure

Each window rule can have the following fields:

`name`
: A name for cross-referencing this rule from other rules.

`match`
: A `WindowMatch` specifying which windows this rule applies to.

`action`
: An action to run when a window starts matching.

`latch`
: An action to run when a window stops matching.

`initial-tile-state`
: `"floating"` or `"tiled"` -- force the initial tile state.

`auto-focus`
: `true`/`false` -- whether the window gets focus on map. Without a matching
  rule, newly mapped windows always receive focus (except for Xwayland
  override-redirect windows such as menus and tooltips, which bypass the
  normal mapping path).

The `initial-tile-state` and `auto-focus` fields are **ad-hoc properties**.
They are evaluated synchronously during the mapping process (before the window
is first displayed), unlike `action` which runs asynchronously after mapping.
If multiple rules match and any sets `auto-focus` to `false`, the window will
not be focused.

### Window Match Criteria

If no fields are set, all windows are matched. If multiple fields are set, they
are implicitly AND-combined. Without a `types` criterion, rules only match
client-created windows (XDG toplevels and X windows).

`types`
: Window type mask: `none`, `any`, `container`, `xdg-toplevel`, `x-window`, `client-window`.

`client`
: A nested `ClientMatch` -- matches the window's owning client.

`title` / `title-regex`
: The window title.

`app-id` / `app-id-regex`
: The XDG app ID.

`floating`
: Whether the window is floating.

`visible`
: Whether the window is visible.

`urgent`
: Whether the window has the urgency flag.

`focused`
: Whether the window has keyboard focus.

`fullscreen`
: Whether the window is fullscreen.

`just-mapped`
: `true` for one compositor iteration after the window maps.

`tag` / `tag-regex`
: The XDG toplevel tag.

`x-class` / `x-class-regex`
: The X11 class (X windows only).

`x-instance` / `x-instance-regex`
: The X11 instance (X windows only).

`x-role` / `x-role-regex`
: The X11 role (X windows only).

`workspace` / `workspace-regex`
: The workspace the window is on.

`content-types`
: Content type mask: `none`, `any`, `photo`, `video`, `game`.

## Combining Criteria

All match objects (both `ClientMatch` and `WindowMatch`) support the same
logical combinators.

### AND (Multiple Fields)

Multiple fields in one match table are implicitly AND-combined:

```toml
[[windows]]
match.title = "VIM"
match.app-id = "Alacritty"
```

This matches only windows whose title is `VIM` **and** whose app ID is
`Alacritty`.

### OR (Array of Matchers)

An array of match objects matches if **any** element matches:

```toml
[[windows]]
match.any = [
    { title = "chromium" },
    { title = "spotify" },
]
```

### NOT (Negation)

```toml
[[windows]]
match.not.title = "firefox"
```

### ALL (Explicit AND)

```toml
[[windows]]
match.all = [
    { title-regex = "chro" },
    { title-regex = "mium" },
]
```

### EXACTLY (N of M)

Match if exactly N of the listed criteria match:

```toml
[[windows]]
match.exactly.num = 1
match.exactly.list = [
    { title = "VIM" },
    { client.sandboxed = true },
]
```

### Cross-referencing Rules by Name

Rules can reference other rules by name:

```toml
[[windows]]
name = "spotify-windows"
match.client.sandbox-app-id = "com.spotify.Client"

[[windows]]
match.name = "spotify-windows"
action = "enter-fullscreen"
```

## Reactive Behavior

Rules are **re-evaluated dynamically** whenever any referenced criterion
changes. For example, if a rule matches on `title`, it is re-checked every time
the window title changes.

- **`action`** fires each time a window transitions from not-matching to
  matching.
- **`latch`** fires each time a window transitions from matching to
  not-matching.

### just-mapped

If you only want a rule to fire once when a window first appears, add
`just-mapped = true` to the match:

```toml
[[windows]]
match.title = "VIM"
match.just-mapped = true
action = "enter-fullscreen"
```

This is similar to the `initial-title` criterion found in some other
compositors.

### Loop Protection

Rules can trigger each other. For example, one rule could fullscreen a window
while another exits fullscreen on the same condition, creating a loop. Jay
prevents such loops from locking up the compositor by capping action callbacks
at 1000 iterations before yielding to other work. However, such loops will
still cause 100% CPU usage and will likely cause affected clients to be killed,
since they won't be able to receive Wayland messages fast enough.

## Practical Examples

### Force an App to Start Floating

```toml
[[windows]]
match.app-id = "pavucontrol"
initial-tile-state = "floating"
```

### Move a Specific App to a Workspace

```toml
[[windows]]
match.client.sandbox-app-id = "com.spotify.Client"
action = { type = "move-to-workspace", name = "3" }
```

### Float Splash Screens Without Stealing Focus

```toml
[[windows]]
match.any = [
    { title = "GIMP Startup", app-id = "gimp" },
    {
        title = "splash",
        x-class-regex = "^jetbrains-(clion|rustrover)$",
    },
]
initial-tile-state = "floating"
auto-focus = false
```

### Run a Command When a Window Appears

```toml
[[windows]]
match.app-id = "firefox"
match.just-mapped = true
action = {
    type = "exec",
    exec = ["notify-send", "Firefox window opened"],
}
```

### Grant Protocol Access to a Trusted App

```toml
[[clients]]
match.comm = "swaylock"
capabilities = ["session-lock", "layer-shell"]

[[clients]]
match.comm = "waybar"
capabilities = ["layer-shell", "foreign-toplevel-list"]
```

### Suppress Focus Stealing for Chromium Screen-Share Windows

```toml
[[windows]]
match.title-regex = 'is sharing (your screen|a window)\.$'
match.client.comm = "chromium"
initial-tile-state = "floating"
auto-focus = false
```

## Introspection

Jay provides several ways to discover the property values you need for writing
rules.

### jay tree

Interactively select a window and print its properties:

```shell
~$ jay tree query select-window
```

Example output:

```text
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

### jay clients

Inspect the client owning a window:

```shell
~$ jay clients show select-window
```

### Control Center Window Search

The control center (opened with `alt-c` by default, or `jay control-center`)
includes a **Window Search** pane where you can search and filter windows using
composable criteria -- helpful for experimenting with match expressions before
putting them in your config.

See [spec.generated.md](https://github.com/mahkoh/jay/blob/master/toml-spec/spec/spec.generated.md) for the full
specification of `WindowRule`, `WindowMatch`, `ClientRule`, `ClientMatch`, and
all available actions.
