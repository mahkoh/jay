# Status Bar

Jay includes a built-in bar that displays workspace tabs, status text, tray
icons (via [wl-tray-bridge](https://github.com/mahkoh/wl-tray-bridge)), and a
clock. The status text is provided by an external program that you configure
in the `[status]` table.

## Configuring a Status Program

The `[status]` table has three fields:

`format`
: Message format: `plain`, `pango`, or `i3bar`. Optional.

`exec`
: How to start the program (string, array, or table). Required.

`i3bar-separator`
: Separator between i3bar components (default `" | "`). Optional.

### Format

`plain`
: Plain text output

`pango`
: Output containing Pango markup

`i3bar`
: JSON output in i3bar protocol format

### Exec

The `exec` field accepts the same forms used elsewhere in the configuration:

- **String** -- the program name: `exec = "i3status"`
- **Array** -- program and arguments: `exec = ["i3status", "-c", "~/.config/i3status/config"]`
- **Table** -- full control over program, arguments, and environment (see the
  spec for details)

## Example: i3status

[i3status](https://i3wm.org/i3status/) is a popular status line generator.

```toml
[status]
format = "i3bar"
exec = "i3status"
```

> [!NOTE]
> i3status defaults to plain-text output. You must explicitly configure it to
> use i3bar format by adding `output_format = "i3bar"` to your
> `~/.config/i3status/config`:
>
> ```
> general {
>     output_format = "i3bar"
> }
> ```

## Example: Custom Script

A simple shell script that prints the date every second:

```toml
[status]
format = "plain"
exec = { shell = "while true; do date '+%Y-%m-%d %H:%M:%S'; sleep 1; done" }
```

## Changing the Status Program at Runtime

The `set-status` action lets you switch the status program from a shortcut.
Omit the `status` field to reset the status text to empty.

```toml
[shortcuts]
# Switch to i3status
alt-F10 = {
    type = "set-status",
    status = {
        format = "i3bar",
        exec = "i3status",
    },
}

# Clear the status text
alt-F11 = { type = "set-status" }
```

## Bar Appearance

The bar's visual appearance (height, background color, text color, position, and
font) is configured in the `[theme]` table. See the
[Theme & Appearance](theme.md) chapter for details. The bar can be shown or
hidden with the `show-bar` top-level setting or the `toggle-bar` action.
