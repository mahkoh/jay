//! Tools for configuring input devices.

pub mod acceleration;
pub mod capability;

use {
    crate::{
        input::{acceleration::AccelProfile, capability::Capability},
        keyboard::{mods::Modifiers, Keymap},
        Axis, Direction, ModifiedKeySym, Workspace,
        _private::{ipc::WorkspaceSource, DEFAULT_SEAT_NAME},
        video::Connector,
    },
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

/// An input device.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct InputDevice(pub u64);

impl InputDevice {
    /// Assigns the input device to a seat.
    pub fn set_seat(self, seat: Seat) {
        get!().set_seat(self, seat)
    }

    /// Sets the keymap of the device.
    ///
    /// This overrides the keymap set for the seat. The keymap becomes active when a key
    /// on the device is pressed.
    ///
    /// Setting the invalid keymap reverts to the seat keymap.
    pub fn set_keymap(self, keymap: Keymap) {
        get!().set_device_keymap(self, keymap)
    }

    /// Returns whether the device has the specified capability.
    pub fn has_capability(self, cap: Capability) -> bool {
        get!(false).has_capability(self, cap)
    }

    /// Sets the device to be left handed.
    ///
    /// This has the effect of swapping the left and right mouse button. See the libinput
    /// documentation for more details.
    pub fn set_left_handed(self, left_handed: bool) {
        get!().set_left_handed(self, left_handed);
    }

    /// Sets the acceleration profile of the device.
    ///
    /// This corresponds to the libinput setting of the same name.
    pub fn set_accel_profile(self, profile: AccelProfile) {
        get!().set_accel_profile(self, profile);
    }

    /// Sets the acceleration speed of the device.
    ///
    /// This corresponds to the libinput setting of the same name.
    pub fn set_accel_speed(self, speed: f64) {
        get!().set_accel_speed(self, speed);
    }

    /// Sets the transformation matrix of the device.
    ///
    /// This is not a libinput setting but a setting of the compositor. It currently affects
    /// relative mouse motions in that the matrix is applied to the motion. To reduce the mouse
    /// speed to 35%, use the following matrix:
    ///
    /// ```text
    /// [
    ///     [0.35, 1.0],
    ///     [1.0, 0.35],
    /// ]
    /// ```
    ///
    /// This might give you better results than using `set_accel_profile` and `set_accel_speed`.
    pub fn set_transform_matrix(self, matrix: [[f64; 2]; 2]) {
        get!().set_transform_matrix(self, matrix);
    }

    /// Returns the name of the device.
    pub fn name(self) -> String {
        get!(String::new()).device_name(self)
    }

    /// Sets how many pixel to scroll per scroll wheel dedent.
    ///
    /// Default: `15.0`
    ///
    /// This setting has no effect on non-wheel input such as touchpads.
    ///
    /// Some mouse wheels support high-resolution scrolling without discrete steps. In
    /// this case a value proportional to this setting will be used.
    pub fn set_px_per_wheel_scroll(self, px: f64) {
        get!().set_px_per_wheel_scroll(self, px);
    }

    /// Sets whether tap-to-click is enabled for this device.
    ///
    /// See <https://wayland.freedesktop.org/libinput/doc/latest/tapping.html>
    pub fn set_tap_enabled(self, enabled: bool) {
        get!().set_input_tap_enabled(self, enabled);
    }

    /// Sets whether tap-and-drag is enabled for this device.
    ///
    /// See <https://wayland.freedesktop.org/libinput/doc/latest/tapping.html>
    pub fn set_drag_enabled(self, enabled: bool) {
        get!().set_input_drag_enabled(self, enabled);
    }

    /// Sets whether drag lock is enabled for this device.
    ///
    /// See <https://wayland.freedesktop.org/libinput/doc/latest/tapping.html>
    pub fn set_drag_lock_enabled(self, enabled: bool) {
        get!().set_input_drag_lock_enabled(self, enabled);
    }

    /// Sets whether natural scrolling is enabled for this device.
    ///
    /// See <https://wayland.freedesktop.org/libinput/doc/latest/scrolling.html>
    pub fn set_natural_scrolling_enabled(self, enabled: bool) {
        get!().set_input_natural_scrolling_enabled(self, enabled);
    }

    /// Returns the syspath of this device.
    ///
    /// E.g. `/sys/devices/pci0000:00/0000:00:08.1/0000:14:00.4/usb5/5-1/5-1.1/5-1.1.3/5-1.1.3:1.0`.
    pub fn syspath(self) -> String {
        get!(String::new()).input_device_syspath(self)
    }

    /// Returns the devnode of this device.
    ///
    /// E.g. `/dev/input/event7`.
    pub fn devnode(self) -> String {
        get!(String::new()).input_device_devnode(self)
    }

    /// Sets a callback that will be run if this device triggers a switch event.
    pub fn on_switch_event<F: FnMut(SwitchEvent) + 'static>(self, f: F) {
        get!().on_switch_event(self, f)
    }

    /// Maps this input device to a connector.
    ///
    /// The connector should be connected.
    ///
    /// This should be used for touch screens and graphics tablets.
    pub fn set_connector(self, connector: Connector) {
        get!().set_input_device_connector(self, connector);
    }

    /// Removes the mapping of this device to a connector.
    pub fn remove_mapping(self) {
        get!().remove_input_mapping(self);
    }
}

/// A seat.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Seat(pub u64);

impl Seat {
    pub const INVALID: Self = Self(0);

    /// Returns whether the seat is invalid.
    pub fn is_invalid(self) -> bool {
        self == Self::INVALID
    }

    #[doc(hidden)]
    pub fn raw(self) -> u64 {
        self.0
    }

    #[doc(hidden)]
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Sets whether this seat's cursor uses the hardware cursor if available.
    ///
    /// Only one seat at a time can use the hardware cursor. Setting this to `true` for a
    /// seat automatically unsets it for all other seats.
    ///
    /// By default, the first created seat uses the hardware cursor.
    pub fn use_hardware_cursor(self, use_hardware_cursor: bool) {
        get!().set_use_hardware_cursor(self, use_hardware_cursor);
    }

    /// Sets the size of the cursor theme.
    ///
    /// Default: 16.
    pub fn set_cursor_size(self, size: i32) {
        get!().set_cursor_size(self, size)
    }

    /// Creates a compositor-wide hotkey.
    ///
    /// The closure is invoked when the user presses the last key of the modified keysym.
    /// Note that the keysym is calculated without modifiers applied. To perform an action
    /// when `SHIFT+k` is pressed, use `SHIFT | SYM_k` not `SHIFT | SYM_K`.
    ///
    /// CapsLock and NumLock are ignored during modifier evaluation. Therefore, bindings
    /// containing these modifiers will never be invoked.
    pub fn bind<T: Into<ModifiedKeySym>, F: FnMut() + 'static>(self, mod_sym: T, f: F) {
        self.bind_masked(Modifiers(!0), mod_sym, f)
    }

    /// Creates a compositor-wide hotkey while ignoring some modifiers.
    ///
    /// This is similar to `bind` except that only the masked modifiers are considered.
    ///
    /// For example, if this function is invoked with `mod_mask = Modifiers::NONE` and
    /// `mod_sym = SYM_XF86AudioRaiseVolume`, then the callback will be invoked whenever
    /// `SYM_XF86AudioRaiseVolume` is pressed. Even if the user is simultaneously holding
    /// the shift key which would otherwise prevent the callback from taking effect.
    ///
    /// For example, if this function is invoked with `mod_mask = CTRL | SHIFT` and
    /// `mod_sym = CTRL | SYM_x`, then the callback will be invoked whenever the user
    /// presses `ctrl+x` without pressing the shift key. Even if the user is
    /// simultaneously holding the alt key.
    ///
    /// If `mod_sym` contains any modifiers, then these modifiers are automatically added
    /// to the mask. The synthetic `RELEASE` modifier is always added to the mask.
    pub fn bind_masked<T: Into<ModifiedKeySym>, F: FnMut() + 'static>(
        self,
        mod_mask: Modifiers,
        mod_sym: T,
        f: F,
    ) {
        get!().bind_masked(self, mod_mask, mod_sym.into(), f)
    }

    /// Registers a callback to be executed when the currently pressed key is released.
    ///
    /// This should only be called in callbacks for key-press binds.
    ///
    /// The callback will be executed once when the key is released regardless of any
    /// modifiers.
    pub fn latch<F: FnOnce() + 'static>(self, f: F) {
        get!().latch(self, f)
    }

    /// Unbinds a hotkey.
    pub fn unbind<T: Into<ModifiedKeySym>>(self, mod_sym: T) {
        get!().unbind(self, mod_sym.into())
    }

    /// Moves the keyboard focus of the seat in the specified direction.
    pub fn focus(self, direction: Direction) {
        get!().focus(self, direction)
    }

    /// Moves the focused window in the specified direction.
    pub fn move_(self, direction: Direction) {
        get!().move_(self, direction)
    }

    /// Sets the keymap of the seat.
    pub fn set_keymap(self, keymap: Keymap) {
        get!().seat_set_keymap(self, keymap)
    }

    /// Returns the repeat rate of the seat.
    ///
    /// The returned tuple is `(rate, delay)` where `rate` is the number of times keys repeat per second
    /// and `delay` is the time after the button press after which keys start repeating.
    pub fn repeat_rate(self) -> (i32, i32) {
        get!((25, 250)).seat_get_repeat_rate(self)
    }

    /// Sets the repeat rate of the seat.
    pub fn set_repeat_rate(self, rate: i32, delay: i32) {
        get!().seat_set_repeat_rate(self, rate, delay)
    }

    /// Returns whether the parent-container of the currently focused window is in mono-mode.
    pub fn mono(self) -> bool {
        get!(false).mono(self)
    }

    /// Sets whether the parent-container of the currently focused window is in mono-mode.
    pub fn set_mono(self, mono: bool) {
        get!().set_mono(self, mono)
    }

    /// Toggles whether the parent-container of the currently focused window is in mono-mode.
    pub fn toggle_mono(self) {
        self.set_mono(!self.mono());
    }

    /// Returns the split axis of the parent-container of the currently focused window.
    pub fn split(self) -> Axis {
        get!(Axis::Horizontal).split(self)
    }

    /// Sets the split axis of the parent-container of the currently focused window.
    pub fn set_split(self, axis: Axis) {
        get!().set_split(self, axis)
    }

    /// Toggles the split axis of the parent-container of the currently focused window.
    pub fn toggle_split(self) {
        self.set_split(self.split().other());
    }

    /// Returns the input devices assigned to this seat.
    pub fn input_devices(self) -> Vec<InputDevice> {
        get!().get_input_devices(Some(self))
    }

    /// Creates a new container with the specified split in place of the currently focused window.
    pub fn create_split(self, axis: Axis) {
        get!().create_split(self, axis);
    }

    /// Focuses the parent node of the currently focused window.
    pub fn focus_parent(self) {
        get!().focus_parent(self);
    }

    /// Requests the currently focused window to be closed.
    pub fn close(self) {
        get!().close(self);
    }

    /// Returns whether the currently focused window is floating.
    pub fn get_floating(self) -> bool {
        get!().get_floating(self)
    }
    /// Sets whether the currently focused window is floating.
    pub fn set_floating(self, floating: bool) {
        get!().set_floating(self, floating);
    }

    /// Toggles whether the currently focused window is floating.
    ///
    /// You can do the same by double-clicking on the header.
    pub fn toggle_floating(self) {
        get!().toggle_floating(self);
    }

    /// Returns the workspace that is currently active on the output that contains the seat's
    /// cursor.
    ///
    /// If no such workspace exists, `exists` returns `false` for the returned workspace.
    pub fn get_workspace(self) -> Workspace {
        get!(Workspace(0)).get_seat_workspace(self)
    }

    /// Shows the workspace and sets the keyboard focus of the seat to that workspace.
    ///
    /// If the workspace doesn't currently exist, it is created on the output that contains the
    /// seat's cursor.
    pub fn show_workspace(self, workspace: Workspace) {
        get!().show_workspace(self, workspace)
    }

    /// Moves the currently focused window to the workspace.
    pub fn set_workspace(self, workspace: Workspace) {
        get!().set_workspace(self, workspace)
    }

    /// Toggles whether the currently focused window is fullscreen.
    pub fn toggle_fullscreen(self) {
        let c = get!();
        c.set_fullscreen(self, !c.get_fullscreen(self));
    }
    /// Returns whether the currently focused window is fullscreen.
    pub fn fullscreen(self) -> bool {
        get!(false).get_fullscreen(self)
    }

    /// Sets whether the currently focused window is fullscreen.
    pub fn set_fullscreen(self, fullscreen: bool) {
        get!().set_fullscreen(self, fullscreen)
    }

    /// Disables the currently active pointer constraint on this seat.
    pub fn disable_pointer_constraint(self) {
        get!().disable_pointer_constraint(self)
    }

    /// Moves the currently focused workspace to another output.
    pub fn move_to_output(self, connector: Connector) {
        get!().move_to_output(WorkspaceSource::Seat(self), connector);
    }

    /// Set whether the current key event is forwarded to the focused client.
    ///
    /// This only has an effect if called from a keyboard shortcut.
    ///
    /// By default, release events are forwarded and press events are consumed. Note that
    /// consuming release events can cause clients to get stuck in the pressed state.
    pub fn set_forward(self, forward: bool) {
        get!().set_forward(self, forward);
    }

    /// This is a shorthand for `set_forward(true)`.
    pub fn forward(self) {
        self.set_forward(true)
    }

    /// This is a shorthand for `set_forward(false)`.
    pub fn consume(self) {
        self.set_forward(false)
    }

    /// Sets the focus-follows-mouse mode.
    pub fn set_focus_follows_mouse_mode(self, mode: FocusFollowsMouseMode) {
        get!().set_focus_follows_mouse_mode(self, mode);
    }
}

/// A focus-follows-mouse mode.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum FocusFollowsMouseMode {
    /// When the mouse moves and enters a toplevel, that toplevel gets the keyboard focus.
    True,
    /// The keyboard focus changes only when clicking on a window or the previously
    /// focused window becomes invisible.
    False,
}

/// Returns all seats.
pub fn get_seats() -> Vec<Seat> {
    get!().seats()
}

/// Returns all input devices.
pub fn input_devices() -> Vec<InputDevice> {
    get!().get_input_devices(None)
}

/// Returns or creates a seat.
///
/// Seats are identified by their name. If no seat with the name exists, a new seat will be created.
///
/// NOTE: You should prefer [`get_default_seat`] instead. Most applications cannot handle more than
/// one seat and will only process input from one of the seats.
pub fn get_seat(name: &str) -> Seat {
    get!(Seat(0)).get_seat(name)
}

/// Returns or creates the default seat.
///
/// This is equivalent to `get_seat("default")`.
pub fn get_default_seat() -> Seat {
    get_seat(DEFAULT_SEAT_NAME)
}

/// Sets a closure to run when a new seat has been created.
pub fn on_new_seat<F: FnMut(Seat) + 'static>(f: F) {
    get!().on_new_seat(f)
}

/// Sets a closure to run when a new input device has been added.
pub fn on_new_input_device<F: FnMut(InputDevice) + 'static>(f: F) {
    get!().on_new_input_device(f)
}

/// Sets a closure to run when an input device has been removed.
pub fn on_input_device_removed<F: FnMut(InputDevice) + 'static>(f: F) {
    get!().on_input_device_removed(f)
}

/// Sets the maximum time between two clicks to be registered as a double click by the
/// compositor.
///
/// This only affects interactions with the compositor UI and has no effect on
/// applications.
///
/// The default is 400 ms.
pub fn set_double_click_time(duration: Duration) {
    let usec = duration.as_micros().min(u64::MAX as u128);
    get!().set_double_click_interval(usec as u64)
}

/// Sets the maximum distance between two clicks to be registered as a double click by the
/// compositor.
///
/// This only affects interactions with the compositor UI and has no effect on
/// applications.
///
/// Setting a negative distance disables double clicks.
///
/// The default is 5.
pub fn set_double_click_distance(distance: i32) {
    get!().set_double_click_distance(distance)
}

/// Disables the creation of a default seat.
///
/// Unless this function is called at startup of the compositor, a seat called `default`
/// will automatically be created.
///
/// When a new input device is attached and a seat called `default` exists, the input
/// device is initially attached to this seat.
pub fn disable_default_seat() {
    get!().disable_default_seat();
}

/// An event generated by a switch.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum SwitchEvent {
    /// The lid of the device (usually a laptop) has been opened.
    ///
    /// This is the default state.
    LidOpened,
    /// The lid of the device (usually a laptop) has been closed.
    ///
    /// If the device is already in this state when the device is discovered, a synthetic
    /// event of this kind is generated.
    LidClosed,
    /// The device has been converted from tablet to laptop mode.
    ///
    /// This is the default state.
    ConvertedToLaptop,
    /// The device has been converted from laptop to tablet mode.
    ///
    /// If the device is already in this state when the device is discovered, a synthetic
    /// event of this kind is generated.
    ConvertedToTablet,
}
