# Mouse Interactions

Jay supports a number of mouse-based interactions that might be hard to discover. This
file documents all of them.

- Inside a tiled container, the tiles can be resized by dragging the separators.

- Floating windows can be resized by dragging the borders.

- Floating windows can be moved by dragging the title.

- Tiles inside containers can be moved by dragging the title.

  - Dragging them onto an existing workspace moves them to that workspace.

  - Dragging them onto the bar outside an existing workspace creates a new workspace.

- Workspaces can be moved by dragging their titles with the mouse.

- In a container in mono layout, scrolling over the title switches between tiles.

- Scrolling over the bar switches between workspaces.

- Double clicking on a tile title/floating window title switches between floating and
  tiling.

- Right clicking on any title in a container switches the container between mono and tiled
  layout.

- Right clicking on the title of a floating window pins/unpins the window. (Pinned windows
  are visible on all workspaces.)

- When the pin icon is visible on a floating window, left clicking the icon pins/unpins
  the window.

- When selecting a toplevel (noticeable by the purple overlay), right clicking on the
  title of any tile in a container selects that container.

- Any long running mouse interaction can be canceled by pressing escape.

## Window Management Mode

Window-management mode makes more interactions available. See the configurable
`window-management-key`.

- In window-management mode, floating windows, tiles, and popups can be resized by
  dragging their contents with the right mouse button.

- In window-management mode, floating windows, tiles, popups, and fullscreen windows can
  be moved by dragging their contents with the left mouse button.

- Entering window-management mode disables all pointer constraints and can therefore be
  used to move the pointer out of windows that have grabbed the pointer.
