# Transactions

When Jay changes the size or position of windows -- for example when you resize a
tile, toggle fullscreen, or rearrange your layout -- the affected applications
have to redraw themselves at their new size. Jay does not update the screen right
away. Instead it asks each affected application to redraw, waits until they are
ready, and then shows the new layout for all of them at once. Such a coordinated
update is called a *transaction*.

This avoids visual glitches during layout changes, such as a gap between a window
and its server-side decorations, stale or stretched content while an application
catches up, or two windows that are resized together briefly being out of step.
This behavior is sometimes called "every frame is perfect".

Because an application could be slow or unresponsive, Jay never waits forever. If
the involved applications do not respond in time, the change is shown anyway --
so a single misbehaving application can never freeze your desktop.

## Configuration

The `[transactions]` table controls how long Jay waits:

`transaction-timeout`
: The longest Jay delays showing a layout change while waiting for the affected
  applications to redraw. Default: 50 ms.

`configure-timeout`
: The longest Jay waits for an application to acknowledge a size change before it
  stops holding back further changes to that window. Default: 50 ms.

`timeout`
: A shorthand that sets both of the above at once.

Each timeout is a table with `millis` and/or `micros` fields, both of which
default to 0:

```toml
[transactions]
transaction-timeout.millis = 50
configure-timeout.millis = 50
```

To set both timeouts to the same value, use `timeout`:

```toml
[transactions]
timeout.millis = 30
```

> [!TIP]
> If applications on your system frequently look stretched or lag behind while
> resizing, a slightly larger timeout gives them more time to redraw. A smaller
> timeout keeps the desktop feeling responsive when an application is slow, at the
> cost of more transient glitches.

> [!NOTE]
> A timeout of 0 effectively disables the delay: layout changes are shown
> immediately, without waiting for applications to catch up.

> [!WARNING]
> Avoid setting a long timeout. Mouse interactions -- clicking, resizing,
> dragging -- act on the new layout as soon as it is computed, not on the layout
> currently shown on screen. With a long timeout the visible layout can lag
> noticeably behind, so the mouse may appear to act on windows where they *will*
> be rather than where you still see them, which is confusing.

## Runtime configuration

The timeouts can also be changed while Jay is running, from the control center's
**Compositor** pane, using the **Transaction Timeout** and **Configure Timeout**
fields.
