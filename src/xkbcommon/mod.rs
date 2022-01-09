#![allow(non_camel_case_types, improper_ctypes)]

mod consts;

include!(concat!(env!("OUT_DIR"), "/xkbcommon_tys.rs"));

use bstr::{BStr, ByteSlice};
pub use consts::*;
use std::ffi::{CStr, VaList};
use std::ops::Deref;
use std::ptr;

use crate::utils::ptr_ext::PtrExt;
use thiserror::Error;
use uapi::c;

#[derive(Debug, Error)]
pub enum XkbCommonError {
    #[error("Could not create an xkbcommon context")]
    CreateContext,
    #[error("Could not create an xkbcommon state")]
    CreateState,
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
    fn xkb_keymap_unref(keymap: *mut xkb_keymap);
    fn xkb_state_unref(state: *mut xkb_state);
    fn xkb_state_new(keymap: *mut xkb_keymap) -> *mut xkb_state;
    fn xkb_state_update_key(
        state: *mut xkb_state,
        key: u32,
        direction: xkb_key_direction,
    ) -> xkb_state_component;
    fn xkb_state_serialize_mods(state: *mut xkb_state, components: xkb_state_component) -> u32;
    fn xkb_state_serialize_layout(state: *mut xkb_state, components: xkb_state_component) -> u32;
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

    pub fn state(&self) -> Result<XkbState, XkbCommonError> {
        let res = unsafe { xkb_state_new(self.keymap) };
        if res.is_null() {
            return Err(XkbCommonError::CreateState);
        }
        Ok(XkbState {
            state: res,
            mods: ModifierState {
                mods_depressed: 0,
                mods_latched: 0,
                mods_locked: 0,
                group: 0,
            },
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

#[derive(Copy, Clone, Debug)]
pub struct ModifierState {
    pub mods_depressed: u32,
    pub mods_latched: u32,
    pub mods_locked: u32,
    pub group: u32,
}

pub struct XkbState {
    state: *mut xkb_state,
    mods: ModifierState,
}

impl XkbState {
    pub fn mods(&self) -> ModifierState {
        self.mods
    }

    pub fn update(&mut self, key: u32, direction: XkbKeyDirection) -> Option<ModifierState> {
        unsafe {
            let changes = xkb_state_update_key(self.state, key + 8, direction.raw() as _);
            if changes != 0 {
                self.mods.mods_depressed =
                    xkb_state_serialize_mods(self.state, XKB_STATE_MODS_DEPRESSED.raw() as _);
                self.mods.mods_latched =
                    xkb_state_serialize_mods(self.state, XKB_STATE_MODS_LATCHED.raw() as _);
                self.mods.mods_locked =
                    xkb_state_serialize_mods(self.state, XKB_STATE_MODS_LOCKED.raw() as _);
                self.mods.group =
                    xkb_state_serialize_layout(self.state, XKB_STATE_LAYOUT_EFFECTIVE.raw() as _);
                Some(self.mods)
            } else {
                None
            }
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
