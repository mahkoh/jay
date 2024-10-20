#![allow(non_camel_case_types)]

pub mod consts;
pub mod device;
pub mod event;
mod sys;

use {
    crate::{
        libinput::{
            consts::{
                LogPriority, LIBINPUT_LOG_PRIORITY_DEBUG, LIBINPUT_LOG_PRIORITY_ERROR,
                LIBINPUT_LOG_PRIORITY_INFO,
            },
            device::RegisteredDevice,
            event::LibInputEvent,
            sys::{
                libinput, libinput_device_ref, libinput_dispatch, libinput_get_event,
                libinput_get_fd, libinput_interface, libinput_log_priority,
                libinput_log_set_handler, libinput_log_set_priority, libinput_path_add_device,
                libinput_path_create_context, libinput_unref,
            },
        },
        udev::UdevError,
        utils::{errorfmt::ErrorFmt, oserror::OsError, ptr_ext::PtrExt, trim::AsciiTrim},
    },
    bstr::ByteSlice,
    isnt::std_1::primitive::IsntConstPtrExt,
    std::{ffi::CStr, rc::Rc},
    thiserror::Error,
    uapi::{c, Errno, IntoUstr, OwnedFd},
};

static INTERFACE: libinput_interface = libinput_interface {
    open_restricted,
    close_restricted,
};

unsafe extern "C" fn open_restricted(
    path: *const c::c_char,
    _flags: c::c_int,
    user_data: *mut c::c_void,
) -> c::c_int {
    let ud = (user_data as *const UserData).deref();
    match ud.adapter.open(CStr::from_ptr(path)) {
        Ok(f) => f.unwrap(),
        Err(e) => {
            log::error!("Could not open device for libinput: {}", ErrorFmt(e));
            -1
        }
    }
}

unsafe extern "C" fn close_restricted(fd: c::c_int, _user_data: *mut c::c_void) {
    drop(OwnedFd::new(fd));
}

struct UserData {
    adapter: Rc<dyn LibInputAdapter>,
}

pub trait LibInputAdapter {
    fn open(&self, path: &CStr) -> Result<OwnedFd, LibInputError>;
}

#[derive(Debug, Error)]
pub enum LibInputError {
    #[error("Could not create a libinput instance")]
    New,
    #[error("Could not open a libinput device")]
    Open,
    #[error("Could not dispatch libinput events")]
    Dispatch(#[source] OsError),
    #[error("The requested device is not available")]
    DeviceUnavailable,
    #[error("Dupfd failed")]
    DupFd(#[source] OsError),
    #[error("The udev subsystem produced an error")]
    Udev(#[from] UdevError),
    #[error("Stat failed")]
    Stat(#[source] OsError),
}

pub struct LibInput {
    _data: Box<UserData>,
    li: *mut libinput,
}

unsafe extern "C" {
    fn jay_libinput_log_handler_bridge();
}

impl LibInput {
    pub fn new(adapter: Rc<dyn LibInputAdapter>) -> Result<Self, LibInputError> {
        let mut ud = Box::new(UserData { adapter });
        let li = unsafe {
            libinput_path_create_context(&INTERFACE, &mut *ud as *mut _ as *mut c::c_void)
        };
        if li.is_null() {
            return Err(LibInputError::New);
        }
        unsafe {
            libinput_log_set_handler(li, jay_libinput_log_handler_bridge);
            let priority = if log::log_enabled!(log::Level::Debug) {
                LIBINPUT_LOG_PRIORITY_DEBUG
            } else if log::log_enabled!(log::Level::Info) {
                LIBINPUT_LOG_PRIORITY_INFO
            } else {
                LIBINPUT_LOG_PRIORITY_ERROR
            };
            libinput_log_set_priority(li, priority.raw() as _);
        }
        Ok(Self { _data: ud, li })
    }

    pub fn fd(&self) -> c::c_int {
        unsafe { libinput_get_fd(self.li) }
    }

    pub fn open<'a>(
        self: &Rc<Self>,
        path: impl IntoUstr<'a>,
    ) -> Result<RegisteredDevice, LibInputError> {
        let path = path.into_ustr();
        let res = unsafe { libinput_path_add_device(self.li, path.as_ptr()) };
        if res.is_null() {
            Err(LibInputError::Open)
        } else {
            unsafe {
                libinput_device_ref(res);
            }
            Ok(RegisteredDevice {
                _li: self.clone(),
                dev: res,
            })
        }
    }

    pub fn dispatch(&self) -> Result<(), LibInputError> {
        let res = unsafe { libinput_dispatch(self.li) };
        if res < 0 {
            Err(LibInputError::Dispatch(Errno(-res).into()))
        } else {
            Ok(())
        }
    }

    pub fn event(&self) -> Option<LibInputEvent> {
        let res = unsafe { libinput_get_event(self.li) };
        if res.is_null() {
            None
        } else {
            Some(LibInputEvent {
                event: res,
                _phantom: Default::default(),
            })
        }
    }
}

impl Drop for LibInput {
    fn drop(&mut self) {
        unsafe {
            libinput_unref(self.li);
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn jay_libinput_log_handler(
    _libinput: *mut libinput,
    priority: libinput_log_priority,
    line: *const c::c_char,
) {
    assert!(line.is_not_null());
    let str = CStr::from_ptr(line);
    let priority = match LogPriority(priority as _) {
        LIBINPUT_LOG_PRIORITY_DEBUG => log::Level::Debug,
        LIBINPUT_LOG_PRIORITY_INFO => log::Level::Info,
        LIBINPUT_LOG_PRIORITY_ERROR => log::Level::Error,
        _ => log::Level::Error,
    };
    log::log!(priority, "libinput: {}", str.to_bytes().trim().as_bstr());
}
