use crate::udev::Udev;
use crate::utils::ptr_ext::PtrExt;
use std::ops::DerefMut;
use std::rc::Rc;
use thiserror::Error;
use uapi::{c, OwnedFd};

#[link(name = "input")]
extern "C" {
    type libinput;

    fn libinput_path_create_context(
        interface: *const libinput_interface,
        user_data: *mut c::c_void,
    ) -> *mut libinput;
    fn libinput_unref(libinput: *mut libinput) -> *mut libinput;
    fn libinput_get_fd(libinput: *mut libinput) -> c::c_int;
}

#[repr(C)]
struct libinput_interface {
    open_restricted: unsafe extern "C" fn(
        path: *const c::c_char,
        flags: c::c_int,
        user_data: *mut c::c_void,
    ) -> c::c_int,
    close_restricted: unsafe extern "C" fn(fd: c::c_int, user_data: *mut c::c_void),
}

static INTERFACE: libinput_interface = libinput_interface {
    open_restricted,
    close_restricted,
};

unsafe extern "C" fn open_restricted(
    path: *const c::c_char,
    flags: c::c_int,
    user_data: *mut c::c_void,
) -> c::c_int {
    let ud = (user_data as *const UserData).deref();
    -1
}

unsafe extern "C" fn close_restricted(fd: c::c_int, _user_data: *mut c::c_void) {
    drop(OwnedFd::new(fd));
}

struct UserData {}

#[derive(Debug, Error)]
pub enum LibInputError {
    #[error("Could not create a libinput instance")]
    New,
}

pub struct LibInput {
    data: Box<UserData>,
    li: *mut libinput,
}

impl LibInput {
    pub fn new() -> Result<Self, LibInputError> {
        let mut ud = Box::new(UserData {});
        let li = unsafe {
            libinput_path_create_context(&INTERFACE, &mut *ud as *mut _ as *mut c::c_void)
        };
        if li.is_null() {
            return Err(LibInputError::New);
        }
        Ok(Self { data: ud, li })
    }

    pub fn fd(&self) -> c::c_int {
        unsafe { libinput_get_fd(self.li) }
    }
}

impl Drop for LibInput {
    fn drop(&mut self) {
        unsafe {
            libinput_unref(self.li);
        }
    }
}
