use {
    crate::fontconfig::consts::{FC_MATCH_PATTERN, FC_RESULT_MATCH},
    run_on_drop::on_drop,
    std::{
        borrow::Cow,
        ffi::{CStr, OsStr, c_char},
        os::{
            raw::{c_int, c_uchar},
            unix::ffi::OsStrExt,
        },
        path::PathBuf,
        ptr,
    },
    thiserror::Error,
    uapi::IntoUstr,
};

mod consts;

include!(concat!(env!("OUT_DIR"), "/fontconfig_tys.rs"));

#[derive(Debug, Error)]
pub enum FontconfigError {
    #[error("FcConfigGetCurrent returned NULL")]
    Init,
    #[error("Could not create a pattern")]
    CreatePattern,
    #[error("Could not find a match")]
    NoMatch,
    #[error("Match has no name")]
    NoName,
    #[error("Match has no file")]
    NoFile,
}

#[derive(Debug)]
pub struct Font {
    pub fullname: String,
    pub file: PathBuf,
    pub index: Option<i32>,
}

pub fn match_font(family: &str) -> Result<Font, FontconfigError> {
    thread_local! {
        static CONFIG: *mut FcConfig = FcConfigGetCurrent();
    }
    let config = CONFIG.with(|c| *c);
    if config.is_null() {
        return Err(FontconfigError::Init);
    }
    let family = family.into_ustr();
    let p = FcPatternCreate();
    if p.is_null() {
        return Err(FontconfigError::CreatePattern);
    }
    let _destroy_pattern = on_drop(|| unsafe { FcPatternDestroy(p) });
    let mut result = 0;
    let p = unsafe {
        FcPatternAddString(p, FC_FAMILY.as_ptr(), family.as_ptr() as _);
        FcConfigSubstitute(config, p, FC_MATCH_PATTERN.0 as _);
        FcDefaultSubstitute(p);
        FcFontMatch(config, p, &mut result)
    };
    if p.is_null() {
        return Err(FontconfigError::NoMatch);
    }
    let _destroy_pattern = on_drop(|| unsafe { FcPatternDestroy(p) });
    if result != FC_RESULT_MATCH.0 as FcResult {
        return Err(FontconfigError::NoMatch);
    }
    let get_cstr = |name: &CStr| {
        let mut out = ptr::null_mut();
        let res = unsafe { FcPatternGetString(p, name.as_ptr(), 0, &mut out) };
        if res != FC_RESULT_MATCH.0 as FcResult || out.is_null() {
            return None;
        }
        let cstr = unsafe { CStr::from_ptr(out.cast()) };
        Some(cstr)
    };
    let get_int = |name: &CStr| {
        let mut out = 0;
        let res = unsafe { FcPatternGetInteger(p, name.as_ptr(), 0, &mut out) };
        if res != FC_RESULT_MATCH.0 as FcResult {
            return None;
        }
        Some(out as i32)
    };
    Ok(Font {
        fullname: get_cstr(FC_FULLNAME)
            .map(CStr::to_string_lossy)
            .map(Cow::into_owned)
            .ok_or(FontconfigError::NoName)?,
        file: get_cstr(FC_FILE)
            .map(CStr::to_bytes)
            .map(OsStr::from_bytes)
            .map(Into::into)
            .ok_or(FontconfigError::NoFile)?,
        index: get_int(FC_INDEX),
    })
}

type FcBool = c_int;
type FcPattern = u8;
type FcConfig = u8;
type FcChar8 = c_uchar;
const FC_FAMILY: &CStr = c"family";
const FC_FULLNAME: &CStr = c"fullname";
const FC_FILE: &CStr = c"file";
const FC_INDEX: &CStr = c"index";

#[link(name = "fontconfig")]
unsafe extern "C" {
    safe fn FcConfigGetCurrent() -> *mut FcConfig;
    safe fn FcPatternCreate() -> *mut FcPattern;
    fn FcPatternDestroy(p: *mut FcPattern);
    fn FcPatternAddString(p: *mut FcPattern, object: *const c_char, s: *const FcChar8) -> FcBool;
    fn FcConfigSubstitute(config: *mut FcConfig, p: *mut FcPattern, kind: FcMatchKind) -> FcBool;
    fn FcDefaultSubstitute(p: *mut FcPattern);
    fn FcFontMatch(
        config: *mut FcConfig,
        p: *mut FcPattern,
        result: *mut FcResult,
    ) -> *mut FcPattern;
    fn FcPatternGetString(
        p: *mut FcPattern,
        object: *const c_char,
        id: c_int,
        s: *mut *mut FcChar8,
    ) -> FcResult;
    fn FcPatternGetInteger(
        p: *mut FcPattern,
        object: *const c_char,
        id: c_int,
        i: *mut c_int,
    ) -> FcResult;
}
