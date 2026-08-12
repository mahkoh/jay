# Tracing Wayland Messages

`jay trace` prints the Wayland messages exchanged between the compositor and
one or more clients. It is like `strace -p`, but for Wayland connections. Use
it to see what an application is asking the compositor to do.

Unlike `WAYLAND_DEBUG=1`, tracing is attached from the outside: the application
does not have to be restarted, does not need any special environment, and does
not have to cooperate.

> [!WARNING]
> A trace contains everything the client sends and receives, including key
> events, window titles, and clipboard activity. Treat trace output as
> sensitive.

## Selecting Clients

Trace a single client by its ID, or click the window you are interested in:

```shell
~$ jay trace id 42
~$ jay trace select-window
```

The ID is the one `jay clients` reports. Both of these commands exit on their
own once the traced client disconnects.

Trace every client instead:

```shell
~$ jay trace all
```

Or select clients by their properties, using a
[match expression](cli.md#match-expressions):

```shell
~$ jay trace match -e 'comm = "chromium"'
~$ jay trace match -e 'sandbox-app-id = "com.spotify.Client"'
```

`jay trace all` and `jay trace match` run until interrupted, and they also pick
up clients that connect later. You can therefore start the trace before
launching the application you want to look at:

```shell
~$ jay trace match -e 'comm = "alacritty"' &
~$ alacritty
```

> [!NOTE]
> A match with no criteria matches every client, so
> `jay trace match -e ''` is the same as `jay trace all`.

## Reading the Output

Each message is printed on one line, in a format similar to `WAYLAND_DEBUG=1`:

```
[12:20:30.035290] {7} -> wl_display#1.get_registry(registry: wl_registry#2)
[12:20:30.035290] {7}    wl_registry#2.global(name: 35, interface: "wl_shm", version: 2)
```

A line consists of:

- The UTC time at which the compositor processed the message. The compositor
  handles many messages in one go, so consecutive lines often carry the same
  timestamp.
- The client ID in braces. This is the same ID that `jay clients` shows.
- `->` for a message sent by the client, two blank spaces for a message sent
  by the compositor.
- The interface, object ID, and name of the message.
- The arguments, each prefixed with its parameter name.

Colors are used when the output goes to a terminal.

Arguments are formatted from Jay's own message definitions rather than the
standard XML protocol definitions, so some of them read differently than under
`WAYLAND_DEBUG=1`:

- The contents of arrays are printed instead of being elided.
- An array of `u16` or `u32` values prints those values rather than the
  underlying bytes.
- A `u64` argument that the XML definitions split into `hi` and `lo` halves
  prints as a single parameter.
- File descriptors print as `fd`. The descriptor itself is not part of the
  trace.

## Object IDs

Wayland clients reuse object IDs, so under `WAYLAND_DEBUG=1` the same number
can refer to a number of unrelated objects over the life of a connection. Jay
assigns every object a unique ID instead:

```
[12:41:30.070469] {22} -> wl_display#1.sync(callback: wl_callback#6)
[12:41:30.070469] {22}    wl_callback#6.done(callback_data: 0)
[12:41:30.070504] {22} -> wl_display#1.sync(callback: wl_callback#7)
[12:41:30.070504] {22}    wl_callback#7.done(callback_data: 0)
```

The second `wl_callback` gets ID 7 even though the client reused ID 6. This
way `wl_surface#50` always refers to the same object throughout the trace,
which makes the output much easier to follow.

`--raw-ids`
: Print the IDs that actually appear on the wire, as `WAYLAND_DEBUG=1` does.

## Dropped Messages

The compositor writes the trace into a fixed-size buffer that `jay trace`
reads from. If `jay trace` does not keep up, the compositor drops messages
rather than slowing the client down. A message whose arguments are too large
for the buffer is dropped as well.

Dropped messages are counted and reported on the next message that gets
through:

```
Missed 22 messages from client 21
```

The count is not lost silently, but the dropped messages themselves are gone.
If you see these lines, write the trace to a file with `-o` instead of to a
terminal or a slow command, and narrow the trace to the clients you are
actually interested in.

> [!TIP]
> This also makes it safe to stop the output of a terminal with `ctrl-s` while
> you look at what has already been printed, and to resume it with `ctrl-q`
> afterwards. Neither the traced client nor the compositor is slowed down; the
> messages produced in the meantime are simply dropped.

## Writing to Files

`-o <PREFIX>`, `--output <PREFIX>`
: Write to `<PREFIX>.<client id>.<suffix>` instead of standard output, one file
  per client. The suffix is `txt`, or `jsonl` with `--json`.

`-c`, `--combine`
: Write every client to a single `<PREFIX>.<suffix>` file. Requires `-o`.

```shell
~$ jay trace -o logs all       # logs.1234.txt, logs.1235.txt, ...
~$ jay trace -o logs -c all    # logs.txt
```

Output written with `-o` is never colored, whether it goes to a file or to a
command.

## Piping to a Command

If the argument to `-o` begins with a pipe, `|`, the rest of it is not a file
prefix but a shell command. Jay runs it through `$SHELL -c` and writes the
trace to its standard input:

```shell
~$ jay --json trace -o '| jq -r .msg' select-window
```

The command inherits Jay's standard output and standard error, so in the
example above the message names appear on your terminal.

Without `-c`, one instance of the command is started per traced client, and
`JAY_CLIENT_ID` is set in its environment to the ID of that client. That is
what makes per-client output possible from a single invocation:

```shell
~$ jay --json trace -o '| jq -r .inf > $JAY_CLIENT_ID' all
```

With `-c`, a single instance receives the traces of every client and
`JAY_CLIENT_ID` is not set:

```shell
~$ jay --json trace -o '| grep -F wl_pointer' -c all
```

> [!TIP]
> Use single quotes around the argument. `$JAY_CLIENT_ID` is expanded by the
> shell that Jay starts, not by the shell you type the command into.

Unless `-c` is used, tracing of a client stops when the command reading its
output exits.

## JSONL Format

With the global `--json` flag the trace is emitted as JSONL -- one JSON object
per line. Every object carries a `t` field naming the record type and a `cl`
field with the client ID. Unlike the query commands, trace records always
contain every field, so `--all-json-fields` has no effect.

`{"t":"n", ...}`
: A client was attached to the trace. Carries an `info` object describing it.

`{"t":"m", ...}`
: A Wayland message.

`{"t":"x", ...}`
: Messages were [dropped](#dropped-messages). Carries the number of dropped
  messages in `n`.

`{"t":"d", ...}`
: The client disconnected and was detached from the trace.

An attach record looks like this:

```json
{
  "t": "n",
  "cl": 21,
  "info": {
    "id": 21,
    "sandboxed": false,
    "sandbox_engine": null,
    "sandbox_app_id": null,
    "sandbox_instance_id": null,
    "uid": 1000,
    "pid": 4711,
    "is_xwayland": false,
    "comm": "firefox",
    "exe": "/usr/lib/firefox/firefox",
    "tag": null,
    "connect_time_us": 1786451740500000,
    "now_us": 1786451740511460
  }
}
```

Its `id` is the client ID and the rest of the fields are the ones `jay clients`
reports, plus two timestamps. `sandbox_engine`, `sandbox_app_id`,
`sandbox_instance_id`, and `tag` are `null` when unset. `connect_time_us` is
when the client connected and `now_us` is when the trace was attached, both in
microseconds since the Unix epoch.

A message record looks like this:

```json
{
  "t": "m",
  "cl": 21,
  "us": 1786451740511460,
  "inf": "wl_display",
  "id": 1,
  "msg": "get_registry",
  "args": {
    "registry": 2
  }
}
```

`us`
: The time the compositor processed the message, in microseconds since the
  Unix epoch.

`inf`, `id`
: The interface and object ID. The ID is unique unless `--raw-ids` was passed.

`msg`
: The name of the message.

`args`
: The arguments, keyed by parameter name. Object IDs and numeric arguments are
  numbers, fixed-point arguments are numbers with a fractional part, booleans
  are `true` or `false`, strings are strings, and arrays are arrays of numbers.
  A nullable string that is unset, and any file descriptor, is `null`.

A dropped-message record carries the count in `n`:

```json
{
  "t": "x",
  "cl": 21,
  "n": 22
}
```

A detach record carries nothing beyond the client ID:

```json
{
  "t": "d",
  "cl": 21
}
```

> [!NOTE]
> The direction of a message is not part of the JSONL output -- there is no
> equivalent of the `->` arrow in the text format.

Because every record is a standalone JSON object, the stream composes with the
usual tools. To watch a single interface live:

```shell
~$ jay --json trace select-window | jq -r 'select(.inf == "wl_pointer") | .msg'
```

Or to record a session and count messages by interface afterwards:

```shell
~$ jay --json trace -o traces -c all
~$ jq -r 'select(.t == "m") | .inf' traces.jsonl | sort | uniq -c | sort -rn
```
