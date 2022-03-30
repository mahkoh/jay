use crate::libinput::sys::{libinput_device, libinput_device_get_user_data, libinput_device_has_capability, libinput_device_set_user_data, libinput_device_unref, libinput_path_remove_device};
use crate::libinput::LibInput;
use std::marker::PhantomData;
use std::rc::Rc;
use crate::libinput::consts::{DeviceCapability};

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
