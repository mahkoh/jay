use std::ffi::CStr;
use std::marker::PhantomData;
use std::ptr;
use std::rc::Rc;
use thiserror::Error;
use uapi::{c, Errno, IntoUstr};

#[link(name = "udev")]
extern "C" {
    type udev;
    type udev_monitor;
    type udev_enumerate;
    type udev_list_entry;
    type udev_device;

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
    #[allow(dead_code)]
    fn udev_list_entry_get_value(list_entry: *mut udev_list_entry) -> *const c::c_char;

    fn udev_device_new_from_syspath(udev: *mut udev, syspath: *const c::c_char)
        -> *mut udev_device;
    fn udev_device_unref(udev_device: *mut udev_device) -> *mut udev_device;
    fn udev_device_get_sysname(udev_device: *mut udev_device) -> *const c::c_char;
    fn udev_device_get_is_initialized(udev_device: *mut udev_device) -> c::c_int;
    fn udev_device_get_devnode(udev_device: *mut udev_device) -> *const c::c_char;
    fn udev_device_get_devnum(udev_device: *mut udev_device) -> c::dev_t;
}

#[derive(Debug, Error)]
pub enum UdevError {
    #[error("Could not create a new udev instance")]
    New(#[source] crate::utils::oserror::OsError),
    #[error("Could not create a new udev_monitor instance")]
    NewMonitor(#[source] crate::utils::oserror::OsError),
    #[error("Could not create a new udev_enumerate instance")]
    NewEnumerate(#[source] crate::utils::oserror::OsError),
    #[error("Could not enable receiving on a udev_monitor")]
    EnableReceiving(#[source] crate::utils::oserror::OsError),
    #[error("Could not add a match rule to a udev_monitor")]
    MonitorAddMatch(#[source] crate::utils::oserror::OsError),
    #[error("Could not add a match rule to a udev_enumerate")]
    EnumerateAddMatch(#[source] crate::utils::oserror::OsError),
    #[error("Could not list devices of a udev_enumerate")]
    EnumerateGetListEntry(#[source] crate::utils::oserror::OsError),
    #[error("Could not scan devices of a udev_enumerate")]
    ScanDevices(#[source] crate::utils::oserror::OsError),
    #[error("Could not create a udev_device from a syspath")]
    DeviceFromSyspath(#[source] crate::utils::oserror::OsError),
    #[error("Could not retrieve the sysname of a udev_device")]
    GetSysname(#[source] crate::utils::oserror::OsError),
    #[error("Could not retrieve the devnode of a udev_device")]
    GetDevnode(#[source] crate::utils::oserror::OsError),
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
    _udev: Rc<Udev>,
    device: *mut udev_device,
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
            _udev: self.clone(),
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
                _udev: self.udev.clone(),
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
    pub fn add_match_subsystem(&self, subsystem: &str) -> Result<(), UdevError> {
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

impl UdevDevice {
    pub fn sysname(&self) -> Result<&CStr, UdevError> {
        let res = unsafe { udev_device_get_sysname(self.device) };
        if res.is_null() {
            Err(UdevError::GetSysname(Errno::default().into()))
        } else {
            unsafe { Ok(CStr::from_ptr(res)) }
        }
    }

    pub fn devnode(&self) -> Result<&CStr, UdevError> {
        let res = unsafe { udev_device_get_devnode(self.device) };
        if res.is_null() {
            Err(UdevError::GetDevnode(Errno::default().into()))
        } else {
            unsafe { Ok(CStr::from_ptr(res)) }
        }
    }

    pub fn devnum(&self) -> c::dev_t {
        unsafe { udev_device_get_devnum(self.device) }
    }

    #[allow(dead_code)]
    pub fn is_initialized(&self) -> bool {
        unsafe { udev_device_get_is_initialized(self.device) != 0 }
    }
}

impl Drop for UdevDevice {
    fn drop(&mut self) {
        unsafe {
            udev_device_unref(self.device);
        }
    }
}
