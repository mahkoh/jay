#![allow(non_camel_case_types)]

use {
    crate::utils::oserror::OsError,
    std::{ffi::CStr, marker::PhantomData, ptr, rc::Rc},
    thiserror::Error,
    uapi::{c, Errno, IntoUstr},
};

#[repr(transparent)]
struct udev(u8);
#[repr(transparent)]
struct udev_monitor(u8);
#[repr(transparent)]
struct udev_enumerate(u8);
#[repr(transparent)]
struct udev_list_entry(u8);
#[repr(transparent)]
struct udev_device(u8);

#[link(name = "udev")]
extern "C" {
    fn udev_new() -> *mut udev;
    fn udev_unref(udev: *mut udev) -> *mut udev;

    fn udev_monitor_new_from_netlink(udev: *mut udev, name: *const c::c_char) -> *mut udev_monitor;
    fn udev_monitor_get_fd(udev_monitor: *mut udev_monitor) -> c::c_int;
    fn udev_monitor_unref(udev_monitor: *mut udev_monitor) -> *mut udev_monitor;
    fn udev_monitor_enable_receiving(udev_monitor: *mut udev_monitor) -> c::c_int;
    fn udev_monitor_filter_add_match_subsystem_devtype(
        udev_monitor: *mut udev_monitor,
        subsystem: *const c::c_char,
        devtype: *const c::c_char,
    ) -> c::c_int;
    fn udev_monitor_receive_device(udev_monitor: *mut udev_monitor) -> *mut udev_device;

    fn udev_enumerate_new(udev: *mut udev) -> *mut udev_enumerate;
    fn udev_enumerate_unref(udev_enumerate: *mut udev_enumerate) -> *mut udev_enumerate;
    fn udev_enumerate_add_match_subsystem(
        udev_enumerate: *mut udev_enumerate,
        subsystem: *const c::c_char,
    ) -> c::c_int;
    fn udev_enumerate_scan_devices(udev_enumerate: *mut udev_enumerate) -> c::c_int;
    fn udev_enumerate_get_list_entry(udev_enumerate: *mut udev_enumerate) -> *mut udev_list_entry;

    fn udev_list_entry_get_next(list_entry: *mut udev_list_entry) -> *mut udev_list_entry;
    fn udev_list_entry_get_name(list_entry: *mut udev_list_entry) -> *const c::c_char;
    #[expect(dead_code)]
    fn udev_list_entry_get_value(list_entry: *mut udev_list_entry) -> *const c::c_char;

    fn udev_device_new_from_syspath(udev: *mut udev, syspath: *const c::c_char)
        -> *mut udev_device;
    fn udev_device_ref(udev_device: *mut udev_device) -> *mut udev_device;
    fn udev_device_unref(udev_device: *mut udev_device) -> *mut udev_device;
    fn udev_device_get_sysname(udev_device: *mut udev_device) -> *const c::c_char;
    fn udev_device_get_is_initialized(udev_device: *mut udev_device) -> c::c_int;
    fn udev_device_get_devnode(udev_device: *mut udev_device) -> *const c::c_char;
    fn udev_device_get_syspath(udev_device: *mut udev_device) -> *const c::c_char;
    // fn udev_device_get_devtype(udev_device: *mut udev_device) -> *const c::c_char;
    fn udev_device_get_devnum(udev_device: *mut udev_device) -> c::dev_t;
    fn udev_device_get_action(udev_device: *mut udev_device) -> *const c::c_char;
    fn udev_device_get_subsystem(udev_device: *mut udev_device) -> *const c::c_char;
    fn udev_device_new_from_devnum(
        udev: *mut udev,
        ty: c::c_char,
        devnum: c::dev_t,
    ) -> *mut udev_device;
    fn udev_device_get_parent(udev_device: *mut udev_device) -> *mut udev_device;
    fn udev_device_get_property_value(
        udev_device: *mut udev_device,
        key: *const c::c_char,
    ) -> *const c::c_char;
}

#[derive(Debug, Error)]
pub enum UdevError {
    #[error("Could not create a new udev instance")]
    New(#[source] OsError),
    #[error("Could not create a new udev_monitor instance")]
    NewMonitor(#[source] OsError),
    #[error("Could not create a new udev_enumerate instance")]
    NewEnumerate(#[source] OsError),
    #[error("Could not enable receiving on a udev_monitor")]
    EnableReceiving(#[source] OsError),
    #[error("Could not add a match rule to a udev_monitor")]
    MonitorAddMatch(#[source] OsError),
    #[error("Could not add a match rule to a udev_enumerate")]
    EnumerateAddMatch(#[source] OsError),
    #[error("Could not list devices of a udev_enumerate")]
    EnumerateGetListEntry(#[source] OsError),
    #[error("Could not scan devices of a udev_enumerate")]
    ScanDevices(#[source] OsError),
    #[error("Could not create a udev_device from a syspath")]
    DeviceFromSyspath(#[source] OsError),
    #[error("Could not create a udev_device from a devnum")]
    DeviceFromDevnum(#[source] OsError),
    #[error("Could not get the device parent")]
    DeviceParent(#[source] OsError),
}

pub struct Udev {
    udev: *mut udev,
}

pub struct UdevMonitor {
    udev: Rc<Udev>,
    monitor: *mut udev_monitor,
}

pub struct UdevEnumerate {
    _udev: Rc<Udev>,
    enumerate: *mut udev_enumerate,
}

pub struct UdevListEntry<'a> {
    list_entry: *mut udev_list_entry,
    _phantom: PhantomData<&'a mut ()>,
}

pub struct UdevDevice {
    udev: Rc<Udev>,
    device: *mut udev_device,
}

pub enum UdevDeviceType {
    Character,
    #[expect(dead_code)]
    Block,
}

impl Udev {
    pub fn new() -> Result<Self, UdevError> {
        let res = unsafe { udev_new() };
        if res.is_null() {
            return Err(UdevError::New(Errno::default().into()));
        }
        Ok(Self { udev: res })
    }

    pub fn create_monitor(self: &Rc<Self>) -> Result<UdevMonitor, UdevError> {
        let res = unsafe { udev_monitor_new_from_netlink(self.udev, "udev\0".as_ptr() as _) };
        if res.is_null() {
            return Err(UdevError::NewMonitor(Errno::default().into()));
        }
        Ok(UdevMonitor {
            udev: self.clone(),
            monitor: res,
        })
    }

    pub fn create_enumerate(self: &Rc<Self>) -> Result<UdevEnumerate, UdevError> {
        let res = unsafe { udev_enumerate_new(self.udev) };
        if res.is_null() {
            return Err(UdevError::NewEnumerate(Errno::default().into()));
        }
        Ok(UdevEnumerate {
            _udev: self.clone(),
            enumerate: res,
        })
    }

    pub fn create_device_from_syspath<'a>(
        self: &Rc<Self>,
        syspath: impl IntoUstr<'a>,
    ) -> Result<UdevDevice, UdevError> {
        let syspath = syspath.into_ustr();
        let res = unsafe { udev_device_new_from_syspath(self.udev, syspath.as_ptr()) };
        if res.is_null() {
            return Err(UdevError::DeviceFromSyspath(Errno::default().into()));
        }
        Ok(UdevDevice {
            udev: self.clone(),
            device: res,
        })
    }

    pub fn create_device_from_devnum(
        self: &Rc<Self>,
        ty: UdevDeviceType,
        devnum: c::dev_t,
    ) -> Result<UdevDevice, UdevError> {
        let ty = match ty {
            UdevDeviceType::Character => b'c',
            UdevDeviceType::Block => b'b',
        };
        let res = unsafe { udev_device_new_from_devnum(self.udev, ty as _, devnum) };
        if res.is_null() {
            return Err(UdevError::DeviceFromDevnum(Errno::default().into()));
        }
        Ok(UdevDevice {
            udev: self.clone(),
            device: res,
        })
    }
}

impl Drop for Udev {
    fn drop(&mut self) {
        unsafe {
            udev_unref(self.udev);
        }
    }
}

impl UdevMonitor {
    pub fn fd(&self) -> c::c_int {
        unsafe { udev_monitor_get_fd(self.monitor) }
    }

    pub fn enable_receiving(&self) -> Result<(), UdevError> {
        let res = unsafe { udev_monitor_enable_receiving(self.monitor) };
        if res < 0 {
            Err(UdevError::EnableReceiving(Errno(-res).into()))
        } else {
            Ok(())
        }
    }

    pub fn add_match_subsystem_devtype(
        &self,
        subsystem: Option<&str>,
        devtype: Option<&str>,
    ) -> Result<(), UdevError> {
        let subsystem = subsystem.map(|s| s.into_ustr());
        let devtype = devtype.map(|s| s.into_ustr());
        let res = unsafe {
            udev_monitor_filter_add_match_subsystem_devtype(
                self.monitor,
                subsystem
                    .as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(ptr::null()),
                devtype.as_ref().map(|s| s.as_ptr()).unwrap_or(ptr::null()),
            )
        };
        if res < 0 {
            Err(UdevError::MonitorAddMatch(Errno(-res).into()))
        } else {
            Ok(())
        }
    }

    pub fn receive_device(&self) -> Option<UdevDevice> {
        let res = unsafe { udev_monitor_receive_device(self.monitor) };
        if res.is_null() {
            None
        } else {
            Some(UdevDevice {
                udev: self.udev.clone(),
                device: res,
            })
        }
    }
}

impl Drop for UdevMonitor {
    fn drop(&mut self) {
        unsafe {
            udev_monitor_unref(self.monitor);
        }
    }
}

impl UdevEnumerate {
    pub fn add_match_subsystem(&self, subsystem: &[u8]) -> Result<(), UdevError> {
        let subsystem = subsystem.into_ustr();
        let res = unsafe { udev_enumerate_add_match_subsystem(self.enumerate, subsystem.as_ptr()) };
        if res < 0 {
            Err(UdevError::EnumerateAddMatch(Errno(-res).into()))
        } else {
            Ok(())
        }
    }

    pub fn scan_devices(&self) -> Result<(), UdevError> {
        let res = unsafe { udev_enumerate_scan_devices(self.enumerate) };
        if res < 0 {
            Err(UdevError::ScanDevices(Errno(-res).into()))
        } else {
            Ok(())
        }
    }

    pub fn get_list_entry(&mut self) -> Result<Option<UdevListEntry>, UdevError> {
        let res = unsafe { udev_enumerate_get_list_entry(self.enumerate) };
        if res.is_null() {
            let err = Errno::default();
            if err.0 == c::ENODATA {
                Ok(None)
            } else {
                Err(UdevError::EnumerateGetListEntry(err.into()))
            }
        } else {
            Ok(Some(UdevListEntry {
                list_entry: res,
                _phantom: Default::default(),
            }))
        }
    }
}

impl Drop for UdevEnumerate {
    fn drop(&mut self) {
        unsafe {
            udev_enumerate_unref(self.enumerate);
        }
    }
}

impl<'a> UdevListEntry<'a> {
    pub fn next(self) -> Option<Self> {
        unsafe {
            let res = udev_list_entry_get_next(self.list_entry);
            if res.is_null() {
                None
            } else {
                Some(Self {
                    list_entry: res,
                    _phantom: Default::default(),
                })
            }
        }
    }

    pub fn name(&self) -> &CStr {
        unsafe {
            let s = udev_list_entry_get_name(self.list_entry);
            CStr::from_ptr(s)
        }
    }
}

macro_rules! strfn {
    ($f:ident, $raw:ident) => {
        pub fn $f(&self) -> Option<&CStr> {
            let res = unsafe { $raw(self.device) };
            if res.is_null() {
                None
            } else {
                unsafe { Some(CStr::from_ptr(res)) }
            }
        }
    };
}

impl UdevDevice {
    strfn!(sysname, udev_device_get_sysname);
    strfn!(syspath, udev_device_get_syspath);
    strfn!(devnode, udev_device_get_devnode);
    // strfn!(devtype, udev_device_get_devtype);
    strfn!(action, udev_device_get_action);
    strfn!(subsystem, udev_device_get_subsystem);

    pub fn devnum(&self) -> c::dev_t {
        unsafe { udev_device_get_devnum(self.device) }
    }

    pub fn parent(&self) -> Result<UdevDevice, UdevError> {
        let res = unsafe { udev_device_get_parent(self.device) };
        if res.is_null() {
            return Err(UdevError::DeviceParent(Errno::default().into()));
        }
        unsafe {
            udev_device_ref(res);
        }
        Ok(UdevDevice {
            udev: self.udev.clone(),
            device: res,
        })
    }

    pub fn is_initialized(&self) -> bool {
        unsafe { udev_device_get_is_initialized(self.device) != 0 }
    }

    fn get_property(&self, prop: &CStr) -> Option<&CStr> {
        let prop = unsafe { udev_device_get_property_value(self.device, prop.as_ptr()) };
        if prop.is_null() {
            None
        } else {
            unsafe { Some(CStr::from_ptr(prop)) }
        }
    }

    pub fn vendor(&self) -> Option<&CStr> {
        self.get_property(c"ID_VENDOR_FROM_DATABASE")
    }

    pub fn model(&self) -> Option<&CStr> {
        self.get_property(c"ID_MODEL_FROM_DATABASE")
    }

    pub fn pci_id(&self) -> Option<&CStr> {
        self.get_property(c"PCI_ID")
    }
}

impl Drop for UdevDevice {
    fn drop(&mut self) {
        unsafe {
            udev_device_unref(self.device);
        }
    }
}
