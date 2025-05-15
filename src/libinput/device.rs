use {
    crate::libinput::{
        LibInput,
        consts::{
            AccelProfile, ConfigClickMethod, ConfigDragLockState, ConfigDragState,
            ConfigMiddleEmulationState, ConfigTapState, DeviceCapability,
            LIBINPUT_CONFIG_DRAG_DISABLED, LIBINPUT_CONFIG_DRAG_ENABLED,
            LIBINPUT_CONFIG_DRAG_LOCK_DISABLED, LIBINPUT_CONFIG_DRAG_LOCK_ENABLED,
            LIBINPUT_CONFIG_MIDDLE_EMULATION_DISABLED, LIBINPUT_CONFIG_MIDDLE_EMULATION_ENABLED,
            LIBINPUT_CONFIG_TAP_DISABLED, LIBINPUT_CONFIG_TAP_ENABLED,
        },
        sys::{
            libinput_device, libinput_device_config_accel_get_profile,
            libinput_device_config_accel_get_speed, libinput_device_config_accel_is_available,
            libinput_device_config_accel_set_profile, libinput_device_config_accel_set_speed,
            libinput_device_config_calibration_get_matrix,
            libinput_device_config_calibration_has_matrix,
            libinput_device_config_calibration_set_matrix, libinput_device_config_click_get_method,
            libinput_device_config_click_get_methods, libinput_device_config_click_set_method,
            libinput_device_config_left_handed_get,
            libinput_device_config_left_handed_is_available,
            libinput_device_config_left_handed_set,
            libinput_device_config_middle_emulation_get_enabled,
            libinput_device_config_middle_emulation_is_available,
            libinput_device_config_middle_emulation_set_enabled,
            libinput_device_config_scroll_get_natural_scroll_enabled,
            libinput_device_config_scroll_has_natural_scroll,
            libinput_device_config_scroll_set_natural_scroll_enabled,
            libinput_device_config_tap_get_drag_enabled,
            libinput_device_config_tap_get_drag_lock_enabled,
            libinput_device_config_tap_get_enabled, libinput_device_config_tap_get_finger_count,
            libinput_device_config_tap_set_drag_enabled,
            libinput_device_config_tap_set_drag_lock_enabled,
            libinput_device_config_tap_set_enabled, libinput_device_get_device_group,
            libinput_device_get_id_bustype, libinput_device_get_id_product,
            libinput_device_get_id_vendor, libinput_device_get_name, libinput_device_get_user_data,
            libinput_device_group, libinput_device_group_get_user_data,
            libinput_device_group_set_user_data, libinput_device_has_capability,
            libinput_device_set_user_data, libinput_device_tablet_pad_get_mode_group,
            libinput_device_tablet_pad_get_num_buttons, libinput_device_tablet_pad_get_num_dials,
            libinput_device_tablet_pad_get_num_mode_groups,
            libinput_device_tablet_pad_get_num_rings, libinput_device_tablet_pad_get_num_strips,
            libinput_device_unref, libinput_path_remove_device, libinput_tablet_pad_mode_group,
            libinput_tablet_pad_mode_group_get_index, libinput_tablet_pad_mode_group_get_mode,
            libinput_tablet_pad_mode_group_get_num_modes,
            libinput_tablet_pad_mode_group_has_button, libinput_tablet_pad_mode_group_has_dial,
            libinput_tablet_pad_mode_group_has_ring, libinput_tablet_pad_mode_group_has_strip,
        },
    },
    bstr::ByteSlice,
    std::{ffi::CStr, marker::PhantomData, rc::Rc},
};

pub struct LibInputDevice<'a> {
    pub(super) dev: *mut libinput_device,
    pub(super) _phantom: PhantomData<&'a ()>,
}

pub struct LibInputDeviceGroup<'a> {
    pub(super) group: *mut libinput_device_group,
    pub(super) _phantom: PhantomData<&'a ()>,
}

pub struct LibInputTabletPadModeGroup<'a> {
    pub(super) group: *mut libinput_tablet_pad_mode_group,
    pub(super) _phantom: PhantomData<&'a ()>,
}

pub struct RegisteredDevice {
    pub(super) _li: Rc<LibInput>,
    pub(super) dev: *mut libinput_device,
}

impl<'a> LibInputDevice<'a> {
    pub fn set_slot(&self, slot: usize) {
        self.set_slot_(slot + 1)
    }

    pub fn unset_slot(&self) {
        self.set_slot_(0)
    }

    fn set_slot_(&self, slot: usize) {
        unsafe {
            libinput_device_set_user_data(self.dev, slot as _);
        }
    }

    pub fn slot(&self) -> Option<usize> {
        let res = unsafe { libinput_device_get_user_data(self.dev) as usize };
        if res == 0 { None } else { Some(res - 1) }
    }

    pub fn has_cap(&self, cap: DeviceCapability) -> bool {
        let res = unsafe { libinput_device_has_capability(self.dev, cap.raw() as _) };
        res != 0
    }

    pub fn left_handed_available(&self) -> bool {
        unsafe { libinput_device_config_left_handed_is_available(self.dev) != 0 }
    }

    pub fn left_handed(&self) -> bool {
        unsafe { libinput_device_config_left_handed_get(self.dev) != 0 }
    }

    pub fn set_left_handed(&self, left_handed: bool) {
        unsafe {
            libinput_device_config_left_handed_set(self.dev, left_handed as _);
        }
    }

    pub fn accel_available(&self) -> bool {
        unsafe { libinput_device_config_accel_is_available(self.dev) != 0 }
    }

    pub fn accel_profile(&self) -> AccelProfile {
        unsafe { AccelProfile(libinput_device_config_accel_get_profile(self.dev)) }
    }

    pub fn accel_speed(&self) -> f64 {
        unsafe { libinput_device_config_accel_get_speed(self.dev) }
    }

    pub fn set_accel_profile(&self, profile: AccelProfile) {
        unsafe {
            libinput_device_config_accel_set_profile(self.dev, profile.raw() as _);
        }
    }

    pub fn set_accel_speed(&self, speed: f64) {
        unsafe {
            libinput_device_config_accel_set_speed(self.dev, speed);
        }
    }

    pub fn name(&self) -> String {
        unsafe {
            let name = libinput_device_get_name(self.dev);
            CStr::from_ptr(name).to_bytes().as_bstr().to_string()
        }
    }

    pub fn set_tap_enabled(&self, enabled: bool) {
        let enabled = match enabled {
            true => LIBINPUT_CONFIG_TAP_ENABLED,
            false => LIBINPUT_CONFIG_TAP_DISABLED,
        };
        unsafe {
            libinput_device_config_tap_set_enabled(self.dev, enabled.raw() as _);
        }
    }

    pub fn tap_available(&self) -> bool {
        unsafe { libinput_device_config_tap_get_finger_count(self.dev) != 0 }
    }

    pub fn tap_enabled(&self) -> bool {
        let enabled = unsafe { ConfigTapState(libinput_device_config_tap_get_enabled(self.dev)) };
        match enabled {
            LIBINPUT_CONFIG_TAP_ENABLED => true,
            _ => false,
        }
    }

    pub fn set_drag_enabled(&self, enabled: bool) {
        let enabled = match enabled {
            true => LIBINPUT_CONFIG_DRAG_ENABLED,
            false => LIBINPUT_CONFIG_DRAG_DISABLED,
        };
        unsafe {
            libinput_device_config_tap_set_drag_enabled(self.dev, enabled.raw() as _);
        }
    }

    pub fn drag_enabled(&self) -> bool {
        let enabled =
            unsafe { ConfigDragState(libinput_device_config_tap_get_drag_enabled(self.dev)) };
        match enabled {
            LIBINPUT_CONFIG_DRAG_ENABLED => true,
            _ => false,
        }
    }

    pub fn set_drag_lock_enabled(&self, enabled: bool) {
        let enabled = match enabled {
            true => LIBINPUT_CONFIG_DRAG_LOCK_ENABLED,
            false => LIBINPUT_CONFIG_DRAG_LOCK_DISABLED,
        };
        unsafe {
            libinput_device_config_tap_set_drag_lock_enabled(self.dev, enabled.raw() as _);
        }
    }

    pub fn drag_lock_enabled(&self) -> bool {
        let enabled = unsafe {
            ConfigDragLockState(libinput_device_config_tap_get_drag_lock_enabled(self.dev))
        };
        match enabled {
            LIBINPUT_CONFIG_DRAG_LOCK_ENABLED => true,
            _ => false,
        }
    }

    pub fn set_natural_scrolling_enabled(&self, enabled: bool) {
        unsafe {
            libinput_device_config_scroll_set_natural_scroll_enabled(self.dev, enabled as _);
        }
    }

    pub fn natural_scrolling_enabled(&self) -> bool {
        unsafe { libinput_device_config_scroll_get_natural_scroll_enabled(self.dev) != 0 }
    }

    pub fn has_natural_scrolling(&self) -> bool {
        unsafe { libinput_device_config_scroll_has_natural_scroll(self.dev) != 0 }
    }

    pub fn has_click_methods(&self) -> bool {
        unsafe { libinput_device_config_click_get_methods(self.dev) != 0 }
    }

    pub fn click_method(&self) -> ConfigClickMethod {
        unsafe { ConfigClickMethod(libinput_device_config_click_get_method(self.dev)) }
    }

    pub fn set_click_method(&self, method: ConfigClickMethod) {
        unsafe {
            libinput_device_config_click_set_method(self.dev, method.raw() as _);
        }
    }

    pub fn set_middle_button_emulation_enabled(&self, enabled: bool) {
        let enabled = match enabled {
            true => LIBINPUT_CONFIG_MIDDLE_EMULATION_ENABLED,
            false => LIBINPUT_CONFIG_MIDDLE_EMULATION_DISABLED,
        };
        unsafe {
            libinput_device_config_middle_emulation_set_enabled(self.dev, enabled.raw() as _);
        }
    }

    pub fn middle_button_emulation_enabled(&self) -> bool {
        let enabled = unsafe {
            ConfigMiddleEmulationState(libinput_device_config_middle_emulation_get_enabled(
                self.dev,
            ))
        };
        match enabled {
            LIBINPUT_CONFIG_MIDDLE_EMULATION_ENABLED => true,
            _ => false,
        }
    }

    pub fn middle_button_emulation_available(&self) -> bool {
        unsafe { libinput_device_config_middle_emulation_is_available(self.dev) != 0 }
    }

    pub fn device_group(&self) -> LibInputDeviceGroup<'_> {
        LibInputDeviceGroup {
            group: unsafe { libinput_device_get_device_group(self.dev) },
            _phantom: Default::default(),
        }
    }

    pub fn product(&self) -> u32 {
        unsafe { libinput_device_get_id_product(self.dev) as u32 }
    }

    pub fn vendor(&self) -> u32 {
        unsafe { libinput_device_get_id_vendor(self.dev) as u32 }
    }

    pub fn bustype(&self) -> Option<u32> {
        libinput_device_get_id_bustype.map(|f| unsafe { f(self.dev) as u32 })
    }

    pub fn pad_num_buttons(&self) -> u32 {
        match unsafe { libinput_device_tablet_pad_get_num_buttons(self.dev) } {
            -1 => 0,
            n => n as u32,
        }
    }

    pub fn pad_num_rings(&self) -> u32 {
        match unsafe { libinput_device_tablet_pad_get_num_rings(self.dev) } {
            -1 => 0,
            n => n as u32,
        }
    }

    pub fn pad_num_strips(&self) -> u32 {
        match unsafe { libinput_device_tablet_pad_get_num_strips(self.dev) } {
            -1 => 0,
            n => n as u32,
        }
    }

    pub fn pad_num_dials(&self) -> u32 {
        match unsafe {
            libinput_device_tablet_pad_get_num_dials
                .map(|f| f(self.dev))
                .unwrap_or_default()
        } {
            -1 => 0,
            n => n as u32,
        }
    }

    pub fn pad_num_mode_groups(&self) -> u32 {
        match unsafe { libinput_device_tablet_pad_get_num_mode_groups(self.dev) } {
            -1 => 0,
            n => n as u32,
        }
    }

    pub fn pad_mode_group(&self, group: u32) -> Option<LibInputTabletPadModeGroup<'_>> {
        let group = unsafe { libinput_device_tablet_pad_get_mode_group(self.dev, group as _) };
        if group.is_null() {
            return None;
        }
        Some(LibInputTabletPadModeGroup {
            group,
            _phantom: Default::default(),
        })
    }

    pub fn has_calibration_matrix(&self) -> bool {
        unsafe { libinput_device_config_calibration_has_matrix(self.dev) != 0 }
    }

    pub fn set_calibration_matrix(&self, m: [[f32; 3]; 2]) {
        let m = [m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2]];
        unsafe {
            libinput_device_config_calibration_set_matrix(self.dev, &m);
        }
    }

    pub fn get_calibration_matrix(&self) -> [[f32; 3]; 2] {
        let mut m = [0.0; 6];
        unsafe {
            libinput_device_config_calibration_get_matrix(self.dev, &mut m);
        }
        [[m[0], m[1], m[2]], [m[3], m[4], m[5]]]
    }
}

impl<'a> LibInputDeviceGroup<'a> {
    pub fn user_data(&self) -> usize {
        unsafe { libinput_device_group_get_user_data(self.group) }
    }

    pub fn set_user_data(&self, user_data: usize) {
        unsafe { libinput_device_group_set_user_data(self.group, user_data) }
    }
}

impl<'a> LibInputTabletPadModeGroup<'a> {
    pub fn index(&self) -> u32 {
        unsafe { libinput_tablet_pad_mode_group_get_index(self.group) as u32 }
    }

    pub fn num_modes(&self) -> u32 {
        unsafe { libinput_tablet_pad_mode_group_get_num_modes(self.group) as u32 }
    }

    pub fn mode(&self) -> u32 {
        unsafe { libinput_tablet_pad_mode_group_get_mode(self.group) as u32 }
    }

    pub fn has_button(&self, button: u32) -> bool {
        unsafe { libinput_tablet_pad_mode_group_has_button(self.group, button as _) != 0 }
    }

    pub fn has_ring(&self, ring: u32) -> bool {
        unsafe { libinput_tablet_pad_mode_group_has_ring(self.group, ring as _) != 0 }
    }

    pub fn has_strip(&self, strip: u32) -> bool {
        unsafe { libinput_tablet_pad_mode_group_has_strip(self.group, strip as _) != 0 }
    }

    pub fn has_dial(&self, dial: u32) -> bool {
        unsafe {
            libinput_tablet_pad_mode_group_has_dial
                .map(|f| f(self.group, dial as _))
                .unwrap_or_default()
                != 0
        }
    }
}

impl RegisteredDevice {
    pub fn device(&self) -> LibInputDevice {
        LibInputDevice {
            dev: self.dev,
            _phantom: Default::default(),
        }
    }
}

impl Drop for RegisteredDevice {
    fn drop(&mut self) {
        unsafe {
            libinput_path_remove_device(self.dev);
            libinput_device_unref(self.dev);
        }
    }
}
