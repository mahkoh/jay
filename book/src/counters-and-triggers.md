# Counters & Triggers

Shortcuts, window rules, and client rules each react to one thing at a time: a
key press, a window, a client. **Counters** and **triggers** let you combine
them. A counter is a named integer that any action can change; a trigger runs
an action when a counter reaches a state you care about.

Together they answer questions no single rule can, such as "is *any* Alacritty
window open?" or "is `alt-x` held down *and* at least one browser window
visible?".

## Counters

A counter is just a name and an integer value. You never declare counters --
the first action that mentions a name creates it, starting at `0`.

Three actions change a counter:

`inc-counter`
: Increase the counter named `name` by `delta` (default `1`).

`dec-counter`
: Decrease the counter named `name` by `delta` (default `1`).

`set-counter`
: Set the counter named `name` to `value` (required).

```toml
[shortcuts]
alt-equal = { type = "inc-counter", name = "my-counter" }
alt-minus = { type = "dec-counter", name = "my-counter" }
alt-0     = { type = "set-counter", name = "my-counter", value = 0 }
```

`delta` may be any integer, including a negative one, and can be larger than
one:

```toml
[shortcuts]
alt-Prior = { type = "inc-counter", name = "my-counter", delta = 10 }
```

These are ordinary actions, so they work anywhere an action is accepted:
shortcuts, [startup hooks](configuration/startup.md), named actions,
[window and client rules](window-rules.md), and the `action`/`latch` of another
trigger.

## Triggers

Triggers live in the `[[triggers]]` array. Each one has a criterion and up to
two actions:

`match`
: The criterion that decides when the trigger is active. Required.

`action`
: Runs when the criterion starts matching. Optional.

`latch`
: Runs when the criterion stops matching. Optional.

```toml
[[triggers]]
match.counter.my-counter.gt = 0
action = { type = "exec", exec.shell = "notify-send 'above zero'" }
latch  = { type = "exec", exec.shell = "notify-send 'back to zero'" }
```

A trigger fires on the **transition**, not on every change. Raising
`my-counter` from `1` to `2` above changes nothing, because the criterion was
already matching. You can supply only `latch` if you only care about the
falling edge.

Triggers have no window attached to them. If you use an action that normally
applies to the matched window in a window rule -- `close`, `float`,
`enter-fullscreen`, and so on -- it applies to the currently focused window
instead.

## Counter criteria

`match.counter` is a table keyed by counter name. Each entry accepts six
comparisons, all optional:

`eq`
: The counter equals this value.

`ne`
: The counter does not equal this value.

`gt`
: The counter is greater than this value.

`ge`
: The counter is greater than or equal to this value.

`lt`
: The counter is less than this value.

`le`
: The counter is less than or equal to this value.

Listing several comparisons for one counter requires all of them to hold:

```toml
[[triggers]]
match.counter.my-counter = { ge = 2, ne = 5 }
```

This matches when the value is 2, 3, 4, 6, 7, … but not 5.

Naming several counters requires all of them to match:

```toml
[[triggers]]
match.counter.browsers.gt = 0
match.counter.terminals.gt = 0
```

## Combining criteria

Criteria compose with the same combinators used by
[window rules](window-rules.md#combining-criteria):

`all`
: Matches if every listed criterion matches.

`any`
: Matches if at least one listed criterion matches.

`not`
: Matches if the contained criterion does not match.

`exactly`
: Matches if exactly `num` of the criteria in `list` match.

```toml
[[triggers]]
match.any = [
    { counter.browsers.gt = 0 },
    { counter.terminals.gt = 0 },
]
```

A bare array is shorthand for `any`:

```toml
[[triggers]]
match = [
    { counter.browsers.gt = 0 },
    { counter.terminals.gt = 0 },
]
```

`exactly` gives you an exclusive-or when `num = 1`:

```toml
[[triggers]]
match.exactly.num = 1
match.exactly.list = [
    { counter.browsers.gt = 0 },
    { counter.terminals.gt = 0 },
]
```

This matches when a browser is open or a terminal is open, but not when both
are.

> [!WARNING]
> A `match` table with no fields matches unconditionally, so the trigger fires
> its action once as soon as the configuration is loaded. Make sure you have
> not misspelled a field name -- unknown keys are reported as warnings in the
> log and otherwise ignored.

## When triggers are evaluated

Every trigger is evaluated once when the configuration is loaded, and again
whenever one of the counters it mentions changes value. Since all counters
start at `0`, a criterion such as `lt = 1` or `eq = 0` is already satisfied at
load time and fires its action immediately.

> [!NOTE]
> Counter values do not survive a [config reload](configuration/index.md#reloading-the-configuration).
> Every counter starts at `0` again. Window and client rules are re-applied to
> existing windows and clients after a reload, so counts maintained by those
> rules rebuild themselves -- but a trigger watching such a counter will fire
> its action again as the count is restored.

## Loop protection

A trigger's action can change a counter, which can activate another trigger.
Jay caps how deeply this may nest with the top-level `max-trigger-depth`
setting, which defaults to `16`:

```toml
max-trigger-depth = 32
```

When the limit is reached, Jay logs an error and skips the action. Setting the
value to `0` or less disables triggers completely.

This is the trigger counterpart to `max-action-depth`, which limits recursion
between [named actions](configuration/index.md#named-actions).

## Practical examples

### React to the first and last window of an application

A window rule counts matching windows; the trigger reacts to the count
crossing zero:

```toml
[[windows]]
match.app-id = "Alacritty"
action = { type = "inc-counter", name = "num-alacritty" }
latch  = { type = "dec-counter", name = "num-alacritty" }

[[triggers]]
match.counter.num-alacritty.gt = 0
action = { type = "exec", exec.shell = "notify-send 'first alacritty opened'" }
latch  = { type = "exec", exec.shell = "notify-send 'last alacritty closed'" }
```

The window rule's `latch` runs when a window stops matching, which includes
the window being closed, so the counter follows the number of open Alacritty
windows.

### React to a range

```toml
[[triggers]]
match.counter.num-alacritty.ge = 2
match.counter.num-alacritty.le = 4
action = { type = "exec", exec.shell = "notify-send 'between 2 and 4 open'" }
```

### Combine a held key with a window state

`complex-shortcuts` can run an action on press and another on release, which
turns a key into a counter that is non-zero exactly while it is held:

```toml
[complex-shortcuts.alt-x]
action = { type = "inc-counter", name = "alt-x-held" }
latch  = { type = "dec-counter", name = "alt-x-held" }

[[windows]]
match.app-id = "Alacritty"
action = { type = "inc-counter", name = "num-alacritty" }
latch  = { type = "dec-counter", name = "num-alacritty" }

[[triggers]]
match.exactly.num = 1
match.exactly.list = [
    { counter.num-alacritty.ne = 0 },
    { counter.alt-x-held.ne = 0 },
]
action = { type = "exec", exec.shell = "notify-send 'exactly one of the two'" }
```

### Disable the idle timeout while a window is fullscreen

Keep the screen awake while anything is playing fullscreen, and restore the
normal [idle timeout](configuration/idle.md) once nothing is:

```toml
idle.minutes = 10

[[windows]]
match.fullscreen = true
action = { type = "inc-counter", name = "num-fullscreen" }
latch  = { type = "dec-counter", name = "num-fullscreen" }

[[triggers]]
match.counter.num-fullscreen.gt = 0
action = { type = "configure-idle", idle.minutes = 0 }
latch  = { type = "configure-idle", idle.minutes = 10 }
```

Setting `idle.minutes = 0` disables the idle timeout; the `latch` puts the
original value back, so keep it in sync with the top-level `idle` setting.

A window rule on its own cannot do this. Its `latch` fires whenever *a* window
stops being fullscreen, so leaving fullscreen in one of two fullscreen windows
would re-enable the idle timeout while the other is still playing. The counter
tracks how many windows are fullscreen, and the trigger only reacts when that
number moves away from or back to zero.

See [spec.generated.md](https://github.com/mahkoh/jay/blob/master/toml-spec/spec/spec.generated.md)
for the full specification of `Trigger`, `TriggerMatch`, `TriggerMatchCounter`,
and the `inc-counter`, `dec-counter`, and `set-counter` actions.
