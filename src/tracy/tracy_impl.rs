use {
    ahash::AHashMap,
    parking_lot::Mutex,
    std::{
        ffi::{CStr, CString},
        ptr,
        sync::{
            atomic::{AtomicBool, Ordering::Relaxed},
            LazyLock,
        },
    },
    tracy_client_sys::{
        ___tracy_c_zone_context, ___tracy_emit_frame_mark_end, ___tracy_emit_frame_mark_start,
        ___tracy_emit_zone_end, ___tracy_source_location_data, ___tracy_startup_profiler,
    },
};

#[derive(Copy, Clone)]
pub struct ZoneName {
    data: &'static ZoneNameData,
}

struct ZoneNameData {
    _name: CString,
    loc: ___tracy_source_location_data,
}

unsafe impl Sync for ZoneNameData {}
unsafe impl Send for ZoneNameData {}

static CACHE: LazyLock<Mutex<AHashMap<String, ZoneName>>> = LazyLock::new(|| Default::default());

impl ZoneName {
    pub fn __get(name: &str) -> Self {
        let mut cache = CACHE.lock();
        if let Some(span) = cache.get(name) {
            return *span;
        }
        let cname = CString::new(name).unwrap();
        let span = ZoneName {
            data: Box::leak(Box::new(ZoneNameData {
                loc: ___tracy_source_location_data {
                    name: cname.as_ptr(),
                    function: ptr::null(),
                    file: ptr::null(),
                    line: 0,
                    color: 0,
                },
                _name: cname,
            })),
        };
        cache.insert(name.to_string(), span);
        span
    }

    #[inline(always)]
    pub fn __enter(self) -> RunningZone {
        if enabled() {
            unsafe {
                let zone = tracy_client_sys::___tracy_emit_zone_begin(&self.data.loc, 1);
                RunningZone(Some(zone))
            }
        } else {
            RunningZone(None)
        }
    }
}

macro_rules! create_zone_name {
    ($($tt:tt)*) => {
        crate::tracy::ZoneName::__get(&format!($($tt)*))
    };
}

pub struct RunningZone(Option<___tracy_c_zone_context>);

impl Drop for RunningZone {
    #[inline(always)]
    fn drop(&mut self) {
        if let Some(zone) = self.0 {
            unsafe {
                ___tracy_emit_zone_end(zone);
            }
        }
    }
}

macro_rules! dynamic_raii_zone {
    ($name:expr) => {{
        let name: ZoneName = $name;
        name.__enter()
    }};
}

macro_rules! dynamic_zone {
    ($name:expr) => {
        let _zone = dynamic_raii_zone!($name);
    };
}

macro_rules! raii_zone {
    ($($tt:tt)*) => {
        {
            static CACHE: std::sync::LazyLock<crate::tracy::ZoneName> = std::sync::LazyLock::new(|| {
                create_zone_name!($($tt)*)
            });
            CACHE.__enter()
        }
    };
}

macro_rules! zone {
    ($($tt:tt)*) => {
        let _zone = raii_zone!($($tt)*);
    };
}

#[derive(Copy, Clone)]
pub struct FrameName {
    name: &'static CString,
}

static FRAME_CACHE: LazyLock<Mutex<AHashMap<String, FrameName>>> =
    LazyLock::new(|| Default::default());

impl FrameName {
    pub fn get(name: &str) -> Self {
        let mut cache = FRAME_CACHE.lock();
        if let Some(frame_name) = cache.get(name) {
            return *frame_name;
        }
        let cname = CString::new(name).unwrap();
        let span = Self {
            name: Box::leak(Box::new(cname)),
        };
        cache.insert(name.to_string(), span);
        span
    }

    #[inline(always)]
    pub fn __start(self) -> RenderingFrame {
        if enabled() {
            unsafe {
                ___tracy_emit_frame_mark_start(self.name.as_ptr());
            }
        }
        RenderingFrame { name: self.name }
    }
}

macro_rules! raii_frame {
    ($name:expr) => {{
        let name: FrameName = $name;
        name.__start()
    }};
}

macro_rules! frame {
    ($name:expr) => {
        let _frame = raii_frame!($name);
    };
}

pub struct RenderingFrame {
    name: &'static CString,
}

impl Drop for RenderingFrame {
    #[inline(always)]
    fn drop(&mut self) {
        if enabled() {
            unsafe {
                ___tracy_emit_frame_mark_end(self.name.as_ptr());
            }
        }
    }
}

#[no_mangle]
#[allow(static_mut_refs)]
unsafe extern "C" fn ___tracy_demangle(
    mangled: *const std::ffi::c_char,
) -> *const std::ffi::c_char {
    use std::io::Write;
    if mangled.is_null() {
        return ptr::null();
    }
    let Ok(mangled) = CStr::from_ptr(mangled).to_str() else {
        return ptr::null();
    };
    let demangled = rustc_demangle::demangle(mangled);
    static mut BUF: Vec<u8> = Vec::new();
    BUF.clear();
    if let Err(_) = write!(BUF, "{demangled:#}\0") {
        return ptr::null();
    }
    BUF.as_ptr().cast()
}

static ENABLED: AtomicBool = AtomicBool::new(false);

#[inline(always)]
fn enabled() -> bool {
    ENABLED.load(Relaxed)
}

pub fn enable_profiler() {
    unsafe {
        ___tracy_startup_profiler();
    }
    ENABLED.store(true, Relaxed);
}
