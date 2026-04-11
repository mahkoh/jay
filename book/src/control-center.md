# Control Center

The control center is Jay's built-in graphical interface for inspecting and
modifying compositor settings. It provides a convenient alternative to editing
configuration files or running CLI commands -- most settings that can be changed
in `config.toml` or via the CLI can also be changed here.

> [!NOTE]
> Changes made in the control center are not persisted across compositor
> restarts. To make settings permanent, add them to your `config.toml`.

> [!TIP]
> The control center consumes GPU and CPU resources while open. Close it when
> not in use to avoid reducing compositor performance.

## Opening the Control Center

- Press `alt-c` (the default shortcut for the `open-control-center` action).
- Run the CLI command:

  ```shell
  ~$ jay control-center
  ```

## Interface Overview

The control center window has a sidebar on the left listing all available panes
and a central panel area on the right.

- **Click** a pane name in the sidebar to open it.
- **Multiple panes** can be open at the same time -- they appear as tabs in the
  panel area.
- **Drag** pane tabs to rearrange them or to split the panel area into
  side-by-side or stacked layouts.
- **Close** a pane by clicking its X button or middle-clicking its tab.

## Panes

### Compositor

General information and top-level controls for the running compositor.

Repository
: Link to the Jay GitHub repository

Version
: The running Jay version

PID
: The compositor process ID

WAYLAND_DISPLAY
: The Wayland socket name (shown when available)

Config DIR
: Path to the active configuration directory (shown when available)

Libei Socket
: Toggle the libei input emulation socket

LIBEI_SOCKET
: The socket name (shown when the Libei Socket toggle is enabled)

Workspace Display Order
: Dropdown to select how workspaces are ordered in the bar

Log Level
: Dropdown to change the active log level at runtime (shown when the logger is available)

Log File
: Click to copy the log file path to the clipboard (shown when the logger is available)

Session Management
: Toggle the `xdg_session_manager_v1` protocol (see
  [Session Management](configuration/misc.md#session-management))

Buttons at the bottom:

- **Quit** -- stop the compositor.
- **Reload Config** -- reload the configuration file.
- **Switch to VT** -- switch to another virtual terminal (with a numeric input
  to select which one).

### Outputs

The Outputs pane is the largest and most interactive pane. It has two sub-views:
a visual **arrangement editor** and per-connector **settings**.

#### Arrangement editor

A 2D preview of your monitor layout. Monitors are drawn as labeled rectangles
at their configured positions and sizes.

- **Click** a monitor to select it (highlighted with a shadow).
- **Drag** a selected monitor to reposition it.
- **Scroll** to zoom in and out.
- **Middle-click drag** or **right-click drag** to pan the viewport.
- **Arrow keys** nudge the selected monitor by 1 pixel.
- **Snap to neighbor** -- when enabled, dragged monitors snap to the edges of
  neighboring monitors within a 10-pixel threshold. Hold **Shift** to
  temporarily invert the snapping behavior.
- **Guide lines** -- optional horizontal and vertical lines at the edges of all
  monitors, helping you align them precisely.

A **Zoom To Fit** checkbox in the top bar auto-scales the view to fit all
monitors. It is disabled when you manually pan or zoom.

Arrangement settings (accessible via the Settings button):

Show guide lines
: Draw alignment guide lines

Snap to neighbor
: Snap edges when dragging (hold Shift to invert)

Show arrangement area
: Toggle the visual arrangement sub-pane

Layout
: How the arrangement and settings are split: Auto, Vertical, or Horizontal

Show disconnected heads
: Include outputs that are no longer connected

Show disabled heads
: Include outputs that are disabled

#### Staged changes

Changes made in the Outputs pane are **staged** -- they are not applied
immediately. Three buttons in the top bar control the workflow:

- **Test** -- validates the staged changes against the display backend without
  applying them. Errors are shown in the pane.
- **Commit** -- applies all staged changes. The pane title shows `Outputs (*)`
  when there are uncommitted changes.
- **Reset** -- discards all staged changes and reverts to the live state.
  This is displayed as a checkbox; checking it resets the staged changes.

When a staged value differs from the live value, the current (live) value is
shown alongside with a `^ current` annotation.

#### Per-connector settings

Each connected display appears as a collapsible section with the connector name,
manufacturer, and model. Inside:

Serial Number
: Read-only identifier

Enabled
: Toggle the connector on or off

Position
: X and Y coordinates in compositor space

Scale
: Fractional scaling factor, with +/- buttons for fine adjustment

Mode
: Resolution and refresh rate (dropdown when multiple modes are available)

Physical Size (mm)
: Read-only physical dimensions in millimeters

Size
: Read-only computed pixel dimensions of the output

Transform
: Rotation and mirroring (none, rotate-90, rotate-180, rotate-270, flip, and flipped rotations)

Custom Brightness
: Toggle whether to use a custom SDR content brightness

Brightness
: Brightness value in cd/m^2 (shown when Custom Brightness is enabled)

Colorimetry
: Color space (depends on monitor capabilities)

EOTF
: Transfer function (depends on monitor capabilities)

Format
: Framebuffer pixel format

Tearing
: Tearing mode. When set to a "Fullscreen" mode, a **Limit Windows** checkbox appears; inside that, a **Requests Tearing** checkbox filters by whether the window has requested tearing.

VRR Active
: Read-only indicator of whether VRR is currently active (only shown when the monitor supports VRR)

VRR
: Variable refresh rate mode (only shown when the monitor supports VRR). When set to a "Fullscreen" mode, a **Limit Windows** checkbox appears; inside that, a **Limit Content Types** checkbox enables filtering by content type (**Photos**, **Videos**, **Games** checkboxes).

Non-desktop
: Read-only indicator (Yes/No) of whether the connector is inherently non-desktop

Override
: Force the connector to be treated as desktop or non-desktop

Blend Space
: How colors are blended during compositing (sRGB or linear)

Use Native Gamut
: Use the display's advertised color primaries instead of assuming sRGB

Native Gamut
: Read-only CIE xy primaries for red, green, blue, and white point

Limit Cursor HZ
: Toggle to limit cursor-triggered refresh rate when VRR is active

Cursor HZ
: Cursor refresh rate value (shown when Limit Cursor HZ is enabled)

Flip Margin (ms)
: Read-only page-flip margin for this connector

### Virtual Outputs

Manage headless virtual outputs. These are useful for screen sharing, testing,
or running applications on a display without a physical monitor.

- View the list of existing virtual outputs.
- **Add** a new virtual output by entering a name and clicking Add.
- **Remove** an existing virtual output by clicking its X button.

### GPUs

Inspect and configure graphics cards (DRM devices). Each GPU appears as a
collapsible section showing its device path and model name.

Vendor
: Read-only GPU vendor name

Model
: Read-only GPU model name

Devnode
: Read-only device path

Syspath
: Read-only sysfs path

PCI ID
: Read-only vendor:model in hex

Dev
: Read-only major:minor device numbers

API
: Dropdown to select the graphics API -- Vulkan (recommended) or the legacy OpenGL renderer

Primary Device
: Checkbox to make this GPU the render device

Direct Scanout
: Toggle direct scanout (bypasses composition for lower latency)

Flip Margin
: Adjust the page-flip margin in milliseconds, with +/- buttons for 0.1 ms steps

Connectors
: List of display connectors attached to this GPU

### Input

The Input pane is divided into per-seat and per-device sections.

#### Per-seat settings

Each seat (typically just `default`) appears as a collapsible section:

Repeat Rate
: Key repeat speed, with +/- 20 buttons

Repeat Delay
: Initial delay before key repeat begins, with +/- 20 buttons

Cursor Size
: Size of the seat cursor in pixels

Simple IM
: Toggle the built-in XCompose-based input method

Hardware Cursor
: Toggle hardware cursor rendering

Pointer Revert Key
: Text field for the keysym name of the cancel key

Focus Follows Mouse
: Toggle whether moving the pointer over a window gives it focus

Fallback Output Mode
: Dropdown to choose between cursor-based and focus-based output selection

Below the settings grid:

- **Focus History** -- checkboxes for "Only Visible" and "Same Workspace".
- **Reload Simple IM** -- button to reload XCompose files without restarting.

##### Keymap management

Each seat has a full keymap management section:

- **Copy Keymap** -- copies the current keymap text to the clipboard.
- **Load Default Keymap** -- restores the compositor's default keymap.
- **Backup / Restore Keymap** -- save and restore a keymap backup.
- **Load Keymap from Clipboard** -- paste a keymap from the clipboard.
- **Create Keymap from Names** -- build a keymap from RMLVO (Rules, Model,
  Layout, Variant, Options) fields. Rules and Model have a text input and a
  "Default" checkbox; Layouts, Variants, and Options have text inputs only.
  Click **Load** to apply.

#### Per-device settings

Each input device appears as a collapsible section. The available settings
depend on the device's capabilities:

Seat
: Dropdown to assign the device to a seat, with a Detach button. Shown for all devices.

Syspath / Devnode
: Read-only device paths. Shown for all devices.

Capabilities
: Read-only list (e.g. Keyboard, Pointer, Touch). Shown for all devices.

Natural Scrolling
: Toggle scroll direction. Shown for devices that support it.

Scroll Distance (px)
: Pixels per legacy scroll event. Shown for pointer devices.

Accel Profile
: Dropdown: Flat or Adaptive. Shown for devices with acceleration.

Accel Speed
: Numeric input (-1.0 to 1.0). Shown for devices with acceleration.

Click Method
: Dropdown: none, button-areas, clickfinger. Shown for devices that support it.

Tap Enabled
: Toggle tap-to-click. Shown for touchpads.

Tap Drag Enabled
: Toggle tap-and-drag. Shown for touchpads.

Tap Drag Lock Enabled
: Toggle tap-drag lock. Shown for touchpads.

Left Handed
: Swap primary and secondary buttons. Shown for devices that support it.

Middle Button Emulation
: Simultaneous left+right produces middle click. Shown for devices that support it.

Output
: Dropdown to map the device to a specific output (only has effect for touch and tablet devices), with a Detach button. Shown for all devices.

Transform Matrix
: 2x2 matrix applied to relative motion. Shown for pointer devices.

Calibration Matrix
: 2x3 matrix for absolute input calibration. Shown for devices that support it.

Device Keymap
: Override the seat keymap for this device, with full keymap management UI and a "Use Seat Keymap" button to revert. Shown for keyboards.

### Idle

Configure the screensaver and idle behavior:

Interval
: Minutes and seconds of inactivity before the on-idle action fires

Grace period
: Minutes and seconds of the warning phase (screen goes black but is not yet locked)

Inhibitors
: Collapsible list showing which applications are currently preventing idle (e.g. video players), with a count in the header

### Look and Feel

Visual customization with live preview. Changes take effect immediately.

Show Bar
: Toggle the status bar

Bar Position
: Dropdown to select the bar position

Show Titles
: Toggle window title bars

Primary Selection
: Toggle middle-click paste (requires application restart to take effect)

UI Drag
: Toggle whether workspaces and tiles can be dragged

UI Drag Threshold (px)
: Minimum distance in pixels before a drag begins

Float Pin Icon
: Show the pin icon on floating windows even when not pinned

Float Above Fullscreen
: Show floating windows above fullscreen windows

Font
: Text field for the main compositor font family

Title Font
: Override font for window title bars (empty = use main font)

Bar Font
: Override font for the status bar (empty = use main font)

Three reset buttons at the bottom: **Reset Sizes**, **Reset Colors**, and
**Reset Fonts**.

#### Sizes

A collapsible section with numeric inputs for every theme size: border widths,
title heights, bar height, gaps, and other spacing values.

#### Colors

A collapsible section with **color pickers** for every theme color. Click a
color swatch to open a full RGBA color picker with sliders and hex input.
This includes colors for backgrounds, borders, text, the status bar, focused
and unfocused windows, attention indicators, and more.

### Clients

Inspect and manage connected Wayland clients.

A **Filter** toggle at the top enables the composable filter builder (see
[Filtering](#filtering) below). When filtering is off, all clients are shown.

Each client appears as a collapsible section showing its ID and process name.
Expand it to see:

ID
: Client identifier

PID
: Process ID

UID
: User ID

comm
: Process name

exe
: Executable path

Sandboxed
: Whether the client is sandboxed (only shown for sandboxed clients)

Secure
: Whether the client uses the privileged socket (only shown for secure clients)

Xwayland
: Shown only for X11 clients

Sandbox Engine
: Sandbox engine name (shown when sandboxed)

App ID
: Sandbox application ID (shown when sandboxed)

Instance ID
: Sandbox instance ID (shown when sandboxed)

Tag
: The connection tag, if any

Kill
: Button to forcefully disconnect the client

Capabilities
: Collapsible list of effective Wayland capabilities

Windows
: Collapsible list of all windows owned by this client

Click the **open in new pane** icon on any client to open a dedicated pane for
that client, allowing you to keep it visible while browsing other panes.

### Window Search

Search and filter windows across the compositor using the composable filter
builder (see [Filtering](#filtering) below).

Each matching window appears as a collapsible section showing its title. Expand
it to see:

ID
: Window identifier

Title
: Window title

Workspace
: Which workspace the window is on

Type
: Container, xdg_toplevel, X Window, or Placeholder

Tag
: Toplevel tag (set via window rules); only shown for xdg_toplevel windows

X11 properties
: Class, Instance, and Role (only shown for Xwayland windows)

App ID
: Application identifier

Floating
: Whether the window is floating

Visible
: Whether the window is visible

Urgent
: Whether the window has the urgency flag

Fullscreen
: Whether the window is fullscreen

Content Type
: The content type hint (photo, video, game), if set

Client
: Full client details (same as the Clients pane)

Click the **open in new pane** icon on any window to open a dedicated pane for
that window.

### Xwayland

Manage the Xwayland compatibility layer for running X11 applications:

Enabled
: Toggle Xwayland on or off

Scaling Mode
: Dropdown: `default` or `downscaled` (renders at highest integer scale then downscales for sharper text on HiDPI)

DISPLAY
: Read-only X11 display number (only shown when Xwayland is running)

Running
: Whether Xwayland is currently running

PID
: Xwayland process ID (only shown when Xwayland is running)

Kill
: Button to forcefully terminate Xwayland (only shown when Xwayland is running)

Client
: Collapsible section with full client details for the Xwayland process (only shown when Xwayland is running)

### Color Management

Configure the Wayland color management protocol:

Enabled
: Toggle the color management protocol for clients

Available
: Read-only indicator of whether color management is available with the current renderer and hardware

## Filtering

The **Clients** and **Window Search** panes share a composable filter system
for narrowing down results. The filter builder works as follows:

At the top level, select a combinator or a leaf criterion from the dropdown:

- **Not** -- inverts a single child criterion.
- **All** -- all child criteria must match (AND).
- **Any** -- at least one child criterion must match (OR).
- **Exactly(n)** -- exactly *n* child criteria must match (with a numeric input
  for *n*).

Compound criteria contain a list of children. Click **Add** to append a new
criterion; click the X button on any child to remove it. Criteria can be nested
to arbitrary depth.

Leaf criteria vary by context:

**Client criteria:** Comm, Exe, Tag, Sandbox Engine, Sandbox App ID, Sandbox
Instance ID (all regex-matched text fields with a "Regex" checkbox), Sandboxed,
Is Xwayland (boolean), UID, PID (numeric inputs).

**Window criteria:** Title, App ID, Tag, Workspace, X Class, X Instance, X Role
(all regex-matched text fields), Floating, Visible, Urgent, Fullscreen
(boolean), Content Types (checkboxes for Photo, Video, Game), and **Client**
(a nested client criterion builder for filtering by the owning client's
properties).

Text-matching criteria have a **Regex** checkbox. When unchecked, the input is
matched as a literal string. When checked, it is treated as a regular
expression. Invalid regex patterns show an error message.
