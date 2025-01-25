#![allow(non_camel_case_types, improper_ctypes)]

mod consts;

include!(concat!(env!("OUT_DIR"), "/xkbcommon_tys.rs"));

pub use consts::*;
use {
    crate::utils::{
        errorfmt::ErrorFmt, oserror::OsError, ptr_ext::PtrExt, trim::AsciiTrim, vecset::VecSet,
    },
    bstr::{BStr, ByteSlice},
    isnt::std_1::primitive::IsntConstPtrExt,
    jay_config::keyboard::syms::KeySym,
    std::{
        cell::{Ref, RefCell},
        ffi::CStr,
        io::Write,
        ops::Deref,
        ptr,
        rc::Rc,
    },
    thiserror::Error,
    uapi::{c, Errno, OwnedFd},
};

#[derive(Debug, Error)]
pub enum XkbCommonError {
    #[error("Could not create an xkbcommon context")]
    CreateContext,
    #[error("Could not create an xkbcommon state")]
    CreateState,
    #[error("Could not create keymap from buffer")]
    KeymapFromBuffer,
    #[error("Could not convert the keymap to a string")]
    AsStr,
    #[error("Could not create a keymap memfd")]
    KeymapMemfd(#[source] OsError),
    #[error("Could not copy the keymap")]
    KeymapCopy(#[source] OsError),
}

struct xkb_context;
struct xkb_keymap;
struct xkb_state;
type xkb_keymap_key_iter_t =
    Option<unsafe extern "C" fn(keymap: *mut xkb_keymap, keycode: xkb_keycode_t, data: *mut Data)>;
#[derive(Copy, Clone)]
#[repr(C)]
struct Data {
    keycode: u32,
    sym_target: u32,
    group: u32,
}

type xkb_keycode_t = u32;
type xkb_layout_index_t = u32;
type xkb_level_index_t = u32;
type xkb_keysym_t = u32;
type xkb_mod_mask_t = u32;

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
unsafe extern "C" {
    fn xkb_context_new(flags: xkb_context_flags) -> *mut xkb_context;
    fn xkb_context_unref(context: *mut xkb_context);
    fn xkb_context_set_log_verbosity(context: *mut xkb_context, verbosity: c::c_int);
    fn xkb_context_set_log_fn(context: *mut xkb_context, log_fn: unsafe extern "C" fn());
    fn xkb_keymap_new_from_buffer(
        context: *mut xkb_context,
        buffer: *const u8,
        length: usize,
        format: xkb_keymap_format,
        flags: xkb_keymap_compile_flags,
    ) -> *mut xkb_keymap;
    fn xkb_keymap_get_as_string(
        keymap: *mut xkb_keymap,
        format: xkb_keymap_format,
    ) -> *mut c::c_char;
    fn xkb_keymap_unref(keymap: *mut xkb_keymap);
    // fn xkb_keymap_ref(keymap: *mut xkb_keymap) -> *mut xkb_keymap;
    fn xkb_keysym_get_name(keysym: xkb_keysym_t, buffer: *mut c::c_char, size: c::size_t) -> i32;
    fn xkb_keymap_key_get_name(keymap: *mut xkb_keymap, key: xkb_keycode_t) -> *const c::c_char;
    // fn xkb_keymap_key_by_name(keymap: *mut xkb_keymap, name: *const c::c_char) -> xkb_keycode_t;
    fn xkb_keymap_key_for_each(
        keymap: *mut xkb_keymap,
        iter: xkb_keymap_key_iter_t,
        data: *mut c::c_void,
    );
    fn xkb_keymap_key_get_syms_by_level(
        keymap: *mut xkb_keymap,
        key: xkb_keycode_t,
        layout: xkb_layout_index_t,
        level: xkb_level_index_t,
        syms_out: *mut *const xkb_keysym_t,
    ) -> c::c_int;
    fn xkb_state_unref(state: *mut xkb_state);
    fn xkb_state_new(keymap: *mut xkb_keymap) -> *mut xkb_state;
    fn xkb_state_update_key(
        state: *mut xkb_state,
        key: u32,
        direction: xkb_key_direction,
    ) -> xkb_state_component;
    fn xkb_state_serialize_mods(state: *mut xkb_state, components: xkb_state_component) -> u32;
    fn xkb_state_serialize_layout(state: *mut xkb_state, components: xkb_state_component) -> u32;
    fn xkb_state_update_mask(
        state: *mut xkb_state,
        depressed_mods: xkb_mod_mask_t,
        latched_mods: xkb_mod_mask_t,
        locked_mods: xkb_mod_mask_t,
        depressed_layout: xkb_layout_index_t,
        latched_layout: xkb_layout_index_t,
        locked_layout: xkb_layout_index_t,
    ) -> xkb_state_component;
}

pub struct XkbContext {
    context: *mut xkb_context,
    ids: KeymapIds,
}

unsafe extern "C" {
    fn jay_xkbcommon_log_handler_bridge();
}

linear_ids!(KeymapIds, KeymapId, u64);

impl XkbContext {
    pub fn new() -> Result<Self, XkbCommonError> {
        let res = unsafe { xkb_context_new(XKB_CONTEXT_NO_FLAGS.raw() as _) };
        if res.is_null() {
            return Err(XkbCommonError::CreateContext);
        }
        unsafe {
            xkb_context_set_log_verbosity(res, 10);
            xkb_context_set_log_fn(res, jay_xkbcommon_log_handler_bridge);
        }
        Ok(Self {
            context: res,
            ids: Default::default(),
        })
    }

    fn raw_to_map(&self, raw: *mut xkb_keymap) -> Result<Rc<XkbKeymap>, XkbCommonError> {
        let res = unsafe { xkb_keymap_get_as_string(raw, XKB_KEYMAP_FORMAT_TEXT_V1.raw() as _) };
        if res.is_null() {
            unsafe {
                xkb_keymap_unref(raw);
            }
            return Err(XkbCommonError::AsStr);
        }
        let str = XkbKeymapStr {
            s: unsafe { CStr::from_ptr(res).to_bytes().as_bstr() },
        };
        let mut memfd =
            uapi::memfd_create("keymap", c::MFD_CLOEXEC | c::MFD_ALLOW_SEALING).unwrap();
        memfd.write_all(str.as_bytes()).unwrap();
        memfd.write_all(&[0]).unwrap();
        uapi::lseek(memfd.raw(), 0, c::SEEK_SET).unwrap();
        uapi::fcntl_add_seals(
            memfd.raw(),
            c::F_SEAL_SEAL | c::F_SEAL_GROW | c::F_SEAL_SHRINK | c::F_SEAL_WRITE,
        )
        .unwrap();
        Ok(Rc::new(XkbKeymap {
            id: self.ids.next(),
            keymap: raw,
            map: Rc::new(memfd),
            map_len: str.len() + 1,
        }))
    }

    pub fn keymap_from_str<S>(&self, s: &S) -> Result<Rc<XkbKeymap>, XkbCommonError>
    where
        S: AsRef<[u8]> + ?Sized,
    {
        let s = s.as_ref();
        unsafe {
            let keymap = xkb_keymap_new_from_buffer(
                self.context,
                s.as_ptr(),
                s.len(),
                XKB_KEYMAP_FORMAT_TEXT_V1.raw(),
                0,
            );
            if keymap.is_null() {
                return Err(XkbCommonError::KeymapFromBuffer);
            }
            self.raw_to_map(keymap)
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
    pub id: KeymapId,
    keymap: *mut xkb_keymap,
    pub map: Rc<OwnedFd>,
    pub map_len: usize,
}

impl XkbKeymap {
    pub fn state(self: &Rc<Self>, id: KeyboardStateId) -> Result<XkbState, XkbCommonError> {
        let res = unsafe { xkb_state_new(self.keymap) };
        if res.is_null() {
            return Err(XkbCommonError::CreateState);
        }
        Ok(XkbState {
            map: self.clone(),
            state: res,
            kb_state: KeyboardState {
                id,
                map: self.map.clone(),
                map_len: self.map_len,
                pressed_keys: Default::default(),
                mods: Default::default(),
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

#[derive(Copy, Clone, Debug, Default)]
pub struct ModifierState {
    pub mods_depressed: u32,
    pub mods_latched: u32,
    pub mods_locked: u32,
    pub mods_effective: u32,
    pub group: u32,
}

linear_ids!(KeyboardStateIds, KeyboardStateId, u64);

pub struct KeyboardState {
    pub id: KeyboardStateId,
    pub map: Rc<OwnedFd>,
    pub map_len: usize,
    pub pressed_keys: VecSet<u32>,
    pub mods: ModifierState,
}

pub trait DynKeyboardState {
    fn borrow(&self) -> Ref<'_, KeyboardState>;
}

impl DynKeyboardState for RefCell<KeyboardState> {
    fn borrow(&self) -> Ref<'_, KeyboardState> {
        self.borrow()
    }
}

pub struct XkbState {
    map: Rc<XkbKeymap>,
    state: *mut xkb_state,
    pub kb_state: KeyboardState,
}

impl DynKeyboardState for RefCell<XkbState> {
    fn borrow(&self) -> Ref<'_, KeyboardState> {
        Ref::map(self.borrow(), |v| &v.kb_state)
    }
}

impl KeyboardState {
    pub fn create_new_keymap_fd(&self) -> Result<Rc<OwnedFd>, XkbCommonError> {
        let fd = match uapi::memfd_create("shared-keymap", c::MFD_CLOEXEC) {
            Ok(fd) => fd,
            Err(e) => return Err(XkbCommonError::KeymapMemfd(e.into())),
        };
        let target = self.map_len as c::off_t;
        let mut pos = 0;
        while pos < target {
            let rem = target - pos;
            let res = uapi::sendfile(fd.raw(), self.map.raw(), Some(&mut pos), rem as usize);
            match res {
                Ok(_) | Err(Errno(c::EINTR)) => {}
                Err(e) => return Err(XkbCommonError::KeymapCopy(e.into())),
            }
        }
        Ok(Rc::new(fd))
    }
}

impl XkbState {
    pub fn mods(&self) -> ModifierState {
        self.kb_state.mods
    }

    fn fetch(&mut self, changes: xkb_state_component) -> bool {
        unsafe {
            if changes != 0 {
                self.kb_state.mods.mods_depressed =
                    xkb_state_serialize_mods(self.state, XKB_STATE_MODS_DEPRESSED.raw() as _);
                self.kb_state.mods.mods_latched =
                    xkb_state_serialize_mods(self.state, XKB_STATE_MODS_LATCHED.raw() as _);
                self.kb_state.mods.mods_locked =
                    xkb_state_serialize_mods(self.state, XKB_STATE_MODS_LOCKED.raw() as _);
                self.kb_state.mods.mods_effective = self.kb_state.mods.mods_depressed
                    | self.kb_state.mods.mods_latched
                    | self.kb_state.mods.mods_locked;
                self.kb_state.mods.group =
                    xkb_state_serialize_layout(self.state, XKB_STATE_LAYOUT_EFFECTIVE.raw() as _);
                true
            } else {
                false
            }
        }
    }

    pub fn update(&mut self, key: u32, direction: XkbKeyDirection) -> bool {
        unsafe {
            let changes = xkb_state_update_key(self.state, key + 8, direction.raw() as _);
            self.fetch(changes)
        }
    }

    pub fn reset(&mut self) {
        let new_state = match self.map.state(self.kb_state.id) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Could not reset XKB state: {}", ErrorFmt(e));
                return;
            }
        };
        *self = new_state;
    }

    #[expect(dead_code)]
    pub fn set(
        &mut self,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) -> bool {
        unsafe {
            let changes = xkb_state_update_mask(
                self.state,
                mods_depressed,
                mods_latched,
                mods_locked,
                0,
                0,
                group,
            );
            self.fetch(changes)
        }
    }

    pub fn unmodified_keysyms(&self, key: u32) -> &[xkb_keysym_t] {
        let mut res = ptr::null();
        unsafe {
            let num = xkb_keymap_key_get_syms_by_level(
                self.map.keymap,
                key + consts::XKB_KEYCODE_MIN,
                self.kb_state.mods.group,
                0,
                &mut res,
            );
            if num > 0 {
                std::slice::from_raw_parts(res, num as usize)
            } else {
                &[]
            }
        }
    }

    #[expect(dead_code)]
    pub fn key_get_name(&self, key: u32) -> String {
        unsafe {
            let name = xkb_keymap_key_get_name(self.map.keymap, key + consts::XKB_KEYCODE_MIN);
            CStr::from_ptr(name).to_string_lossy().to_string()
        }
    }

    #[expect(dead_code)]
    pub fn keysym_get_name(&self, keysym: &KeySym) -> Option<String> {
        let size = 64;
        let mut buffer: Vec<u8> = vec![0; size];
        unsafe {
            let buffer = buffer.as_mut_ptr() as *mut c::c_char;
            let length = xkb_keysym_get_name(keysym.0, buffer, size);
            if length == -1 {
                None
            } else {
                Some(CStr::from_ptr(buffer).to_str().ok()?.to_string())
            }
        }
    }

    pub fn key_from_mod_keysym(&self, keysym: &KeySym) -> Option<u32> {
        let keycode = self.keycode_from_keysym(keysym);
        if keycode == consts::XKB_KEYCODE_INVALID
            || keycode < consts::XKB_KEYCODE_MIN
            || keycode > consts::XKB_KEYCODE_MAX
        {
            None
        } else {
            return Some(keycode - consts::XKB_KEYCODE_MIN);
        }
    }

    #[no_mangle]
    fn keycode_from_keysym(&self, keysym: &KeySym) -> u32 {
        use consts::XKB_KEYCODE_INVALID;
        let keymap = self.map.keymap;
        let data = Data {
            keycode: XKB_KEYCODE_INVALID,
            sym_target: keysym.0,
            group: self.kb_state.mods.group,
        };
        let data = Box::new(data);
        unsafe {
            let data = Box::into_raw(data) as *mut c::c_void;
            xkb_keymap_key_for_each(keymap, Some(search_keycode_by_keysym as _), data);
            let data = data as *mut Data;
            let keycode = (*data).keycode;
            keycode
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

#[no_mangle]
unsafe extern "C" fn search_keycode_by_keysym(
    keymap: *mut xkb_keymap,
    keycode: xkb_keycode_t,
    data: *mut Data,
) {
    let Data {
        sym_target, group, ..
    } = unsafe { *data as Data };
    let mut res = ptr::null();
    let num = unsafe { xkb_keymap_key_get_syms_by_level(keymap, keycode, group, 0, &mut res) };
    if num > 0 {
        let syms = unsafe { std::slice::from_raw_parts(res, num as usize) };
        for sym_found in syms {
            if *sym_found == sym_target {
                unsafe {
                    (*data).keycode = keycode.clone();
                }
                break;
            }
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn jay_xkbcommon_log_handler(
    _ctx: *mut xkb_context,
    level: xkb_log_level,
    line: *const c::c_char,
) {
    assert!(line.is_not_null());
    let buf = unsafe { CStr::from_ptr(line) };
    let level = match XkbLogLevel(level) {
        XKB_LOG_LEVEL_CRITICAL | XKB_LOG_LEVEL_ERROR => log::Level::Error,
        XKB_LOG_LEVEL_WARNING => log::Level::Warn,
        XKB_LOG_LEVEL_INFO => log::Level::Info,
        XKB_LOG_LEVEL_DEBUG => log::Level::Debug,
        _ => log::Level::Error,
    };
    log::log!(level, "xkbcommon: {}", buf.to_bytes().trim_end().as_bstr());
}
