use {
    crate::libinput::{
        consts::{
            AccelProfile, ConfigDragLockState, ConfigDragState, ConfigTapState, DeviceCapability,
            LIBINPUT_CONFIG_DRAG_DISABLED, LIBINPUT_CONFIG_DRAG_ENABLED,
            LIBINPUT_CONFIG_DRAG_LOCK_DISABLED, LIBINPUT_CONFIG_DRAG_LOCK_ENABLED,
            LIBINPUT_CONFIG_TAP_DISABLED, LIBINPUT_CONFIG_TAP_ENABLED,
        },
        sys::{
            libinput_device, libinput_device_config_accel_set_profile,
            libinput_device_config_accel_set_speed, libinput_device_config_left_handed_set,
            libinput_device_config_scroll_get_natural_scroll_enabled,
            libinput_device_config_scroll_set_natural_scroll_enabled,
            libinput_device_config_tap_get_drag_enabled,
            libinput_device_config_tap_get_drag_lock_enabled,
            libinput_device_config_tap_get_enabled, libinput_device_config_tap_set_drag_enabled,
            libinput_device_config_tap_set_drag_lock_enabled,
            libinput_device_config_tap_set_enabled, libinput_device_get_name,
            libinput_device_get_user_data, libinput_device_has_capability,
            libinput_device_set_user_data, libinput_device_unref, libinput_path_remove_device,
        },
        LibInput,
    },
    bstr::ByteSlice,
    std::{ffi::CStr, marker::PhantomData, rc::Rc},
};

pub struct LibInputDevice<'a> {
    pub(super) dev: *mut libinput_device,
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
        if res == 0 {
            None
        } else {
            Some(res - 1)
        }
    }

    pub fn has_cap(&self, cap: DeviceCapability) -> bool {
        let res = unsafe { libinput_device_has_capability(self.dev, cap.raw() as _) };
        res != 0
    }

    pub fn set_left_handed(&self, left_handed: bool) {
        unsafe {
            libinput_device_config_left_handed_set(self.dev, left_handed as _);
        }
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
