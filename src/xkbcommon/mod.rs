#![allow(non_camel_case_types)]

mod consts;

include!(concat!(env!("OUT_DIR"), "/xkbcommon_tys.rs"));

use bstr::{BStr, ByteSlice};
pub use consts::*;
use std::ffi::{CStr, VaList};
use std::ops::Deref;
use std::ptr;

use crate::utils::ptr_ext::PtrExt;
use libloading::Library;
use thiserror::Error;
use uapi::c;
use xcb_dl::ffi::xcb_connection_t;

#[derive(Debug, Error)]
pub enum XkbCommonError {
    #[error("xkbcommon-x11 could not be loaded")]
    LoadXkbCommonX11(#[source] libloading::Error),
    #[error("One of the xkbcommon-x11 symbols could not be loaded")]
    LoadXkbCommonX11Sym(#[source] libloading::Error),
    #[error("Could not create keymap from X11 device")]
    CreateKeymapFromDevice,
    #[error("Could not create state from X11 device")]
    CreateStateFromDevice,
    #[error("Could not create an xkbcommon context")]
    CreateContext,
    #[error("Could not create keymap from names")]
    KeymapFromNames,
    #[error("Could not convert the keymap to a string")]
    AsStr,
}

struct xkb_context;
struct xkb_keymap;
struct xkb_state;

#[repr(C)]
struct xkb_rule_names {
    rules: *const c::c_char,
    model: *const c::c_char,
    layout: *const c::c_char,
    variant: *const c::c_char,
    options: *const c::c_char,
}

impl Default for xkb_rule_names {
    fn default() -> Self {
        Self {
            rules: ptr::null(),
            model: ptr::null(),
            layout: ptr::null(),
            variant: ptr::null(),
            options: ptr::null(),
        }
    }
}

#[link(name = "xkbcommon")]
extern "C" {
    fn xkb_context_new(flags: xkb_context_flags) -> *mut xkb_context;
    fn xkb_context_unref(context: *mut xkb_context);
    fn xkb_context_set_log_fn(
        context: *mut xkb_context,
        log_fn: unsafe extern "C" fn(
            context: *mut xkb_context,
            level: xkb_log_level,
            format: *const c::c_char,
            args: VaList,
        ),
    );
    fn xkb_keymap_new_from_names(
        context: *mut xkb_context,
        name: *const xkb_rule_names,
        flags: xkb_keymap_compile_flags,
    ) -> *mut xkb_keymap;
    fn xkb_keymap_get_as_string(
        keymap: *mut xkb_keymap,
        format: xkb_keymap_format,
    ) -> *mut c::c_char;
    fn xkb_keymap_ref(keymap: *mut xkb_keymap) -> *mut xkb_keymap;
    fn xkb_keymap_unref(keymap: *mut xkb_keymap);
    fn xkb_state_unref(state: *mut xkb_state);
    fn xkb_state_get_keymap(state: *mut xkb_state) -> *mut xkb_keymap;
}

pub struct XkbContext {
    context: *mut xkb_context,
}

impl XkbContext {
    pub fn new() -> Result<Self, XkbCommonError> {
        let res = unsafe { xkb_context_new(XKB_CONTEXT_NO_FLAGS.raw() as _) };
        if res.is_null() {
            return Err(XkbCommonError::CreateContext);
        }
        unsafe {
            xkb_context_set_log_fn(res, xkbcommon_logger);
        }
        Ok(Self { context: res })
    }

    pub fn default_keymap(&self) -> Result<XkbKeymap, XkbCommonError> {
        unsafe {
            let names = Default::default();
            let keymap = xkb_keymap_new_from_names(self.context, &names, 0);
            if keymap.is_null() {
                return Err(XkbCommonError::KeymapFromNames);
            }
            Ok(XkbKeymap { keymap })
        }
    }
}

impl Drop for XkbContext {
    fn drop(&mut self) {
        unsafe {
            xkb_context_unref(self.context);
        }
    }
}

pub struct XkbKeymap {
    keymap: *mut xkb_keymap,
}

impl XkbKeymap {
    pub fn as_str(&self) -> Result<XkbKeymapStr, XkbCommonError> {
        let res =
            unsafe { xkb_keymap_get_as_string(self.keymap, XKB_KEYMAP_FORMAT_TEXT_V1.raw() as _) };
        if res.is_null() {
            return Err(XkbCommonError::AsStr);
        }
        Ok(XkbKeymapStr {
            s: unsafe { CStr::from_ptr(res).to_bytes().as_bstr() },
        })
    }
}

impl Drop for XkbKeymap {
    fn drop(&mut self) {
        unsafe {
            xkb_keymap_unref(self.keymap);
        }
    }
}

pub struct XkbKeymapStr {
    s: *const BStr,
}

impl Deref for XkbKeymapStr {
    type Target = BStr;

    fn deref(&self) -> &Self::Target {
        unsafe { self.s.deref() }
    }
}

impl Drop for XkbKeymapStr {
    fn drop(&mut self) {
        unsafe { c::free(self.s as _) }
    }
}

pub struct XkbState {
    state: *mut xkb_state,
}

impl XkbState {
    pub fn keymap(&self) -> XkbKeymap {
        unsafe {
            let res = xkb_state_get_keymap(self.state);
            xkb_keymap_ref(res);
            XkbKeymap { keymap: res }
        }
    }
}

impl Drop for XkbState {
    fn drop(&mut self) {
        unsafe {
            xkb_state_unref(self.state);
        }
    }
}

pub struct XkbCommonX11 {
    library: Library,
    fns: XkbCommonX11Fns,
}

struct XkbCommonX11Fns {
    xkb_x11_keymap_new_from_device: unsafe fn(
        context: *mut xkb_context,
        c: *mut xcb_connection_t,
        device_id: i32,
        flags: xkb_x11_setup_xkb_extension_flags,
    ) -> *mut xkb_keymap,
    xkb_x11_state_new_from_device: unsafe fn(
        keymap: *mut xkb_keymap,
        c: *mut xcb_connection_t,
        device_id: i32,
    ) -> *mut xkb_state,
}

impl XkbCommonX11 {
    pub fn load() -> Result<Self, XkbCommonError> {
        let library = unsafe {
            match Library::new("libxkbcommon-x11.so") {
                Ok(l) => l,
                Err(e) => return Err(XkbCommonError::LoadXkbCommonX11(e)),
            }
        };
        let fns = match get_xkbcommon_x11_fns(&library) {
            Ok(f) => f,
            Err(e) => return Err(XkbCommonError::LoadXkbCommonX11Sym(e)),
        };
        Ok(Self { library, fns })
    }

    pub unsafe fn keymap_from_device(
        &self,
        context: &XkbContext,
        c: *mut xcb_connection_t,
        device_id: i32,
        flags: XkbX11SetupXkbExtensionFlags,
    ) -> Result<XkbKeymap, XkbCommonError> {
        let res = (self.fns.xkb_x11_keymap_new_from_device)(
            context.context,
            c,
            device_id,
            flags.raw() as _,
        );
        if res.is_null() {
            return Err(XkbCommonError::CreateKeymapFromDevice);
        }
        Ok(XkbKeymap { keymap: res })
    }

    pub unsafe fn state_from_device(
        &self,
        keymap: &XkbKeymap,
        c: *mut xcb_connection_t,
        device_id: i32,
    ) -> Result<XkbState, XkbCommonError> {
        let res = (self.fns.xkb_x11_state_new_from_device)(keymap.keymap, c, device_id);
        if res.is_null() {
            return Err(XkbCommonError::CreateStateFromDevice);
        }
        Ok(XkbState { state: res })
    }
}

fn get_xkbcommon_x11_fns(lib: &Library) -> Result<XkbCommonX11Fns, libloading::Error> {
    macro_rules! syms {
        ($($sym:ident,)*) => {
            Ok(XkbCommonX11Fns {
                $(
                    $sym: std::mem::transmute(lib.get::<usize>(concat!(stringify!($sym), "\0").as_bytes())?.into_raw().into_raw()),
                )*
            })
        }
    }
    unsafe {
        syms! {
            xkb_x11_keymap_new_from_device,
            xkb_x11_state_new_from_device,
        }
    }
}

unsafe extern "C" fn xkbcommon_logger(
    _ctx: *mut xkb_context,
    level: xkb_log_level,
    format: *const c::c_char,
    args: VaList,
) {
    extern "C" {
        fn vasprintf(buf: *mut *mut c::c_char, fmt: *const c::c_char, args: VaList) -> c::c_int;
    }
    let mut buf = ptr::null_mut();
    let res = vasprintf(&mut buf, format, args);
    if res < 0 {
        log::warn!("Could not vasprintf");
    }
    let buf = std::slice::from_raw_parts(buf as *const u8, res as usize);
    let buf = buf.as_bstr();
    let level = match XkbLogLevel(level) {
        XKB_LOG_LEVEL_CRITICAL | XKB_LOG_LEVEL_ERROR => log::Level::Error,
        XKB_LOG_LEVEL_WARNING => log::Level::Warn,
        XKB_LOG_LEVEL_INFO => log::Level::Info,
        XKB_LOG_LEVEL_DEBUG => log::Level::Debug,
        _ => log::Level::Error,
    };
    log::log!(level, "xkbcommon: {}", buf);
}
