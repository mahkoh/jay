use crate::gles2::sys::{GLint, GL_BGRA_EXT, GL_UNSIGNED_BYTE};
use crate::pixman;
use ahash::AHashMap;
use once_cell::sync::Lazy;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Format {
    pub name: &'static str,
    pub bpp: u32,
    pub pixman: Option<pixman::Format>,
    pub gl_format: GLint,
    pub gl_type: GLint,
    pub drm: u32,
    pub wl_id: Option<u32>,
}

static FORMATS_MAP: Lazy<AHashMap<u32, &'static Format>> = Lazy::new(|| {
    let mut map = AHashMap::new();
    for format in FORMATS {
        assert!(map.insert(format.drm, format).is_none());
    }
    map
});

pub fn formats() -> &'static AHashMap<u32, &'static Format> {
    &*FORMATS_MAP
}

#[allow(dead_code)]
const fn fourcc_code(a: char, b: char, c: char, d: char) -> u32 {
    (a as u32) | ((b as u32) << 8) | ((c as u32) << 16) | ((d as u32) << 24)
}

const ARGB8888_ID: u32 = 0;
const ARGB8888_DRM: u32 = fourcc_code('A', 'R', '2', '4');

const XRGB8888_ID: u32 = 1;
const XRGB8888_DRM: u32 = fourcc_code('X', 'R', '2', '4');

pub fn map_wayland_format_id(id: u32) -> u32 {
    match id {
        ARGB8888_ID => ARGB8888_DRM,
        XRGB8888_ID => XRGB8888_DRM,
        _ => id,
    }
}

pub static ARGB8888: &Format = &FORMATS[0];
pub static XRGB8888: &Format = &FORMATS[1];

pub static FORMATS: &[Format] = &[
    Format {
        name: "argb8888",
        bpp: 4,
        pixman: Some(pixman::A8R8G8B8),
        gl_format: GL_BGRA_EXT,
        gl_type: GL_UNSIGNED_BYTE,
        drm: ARGB8888_DRM,
        wl_id: Some(ARGB8888_ID),
    },
    Format {
        name: "xrgb8888",
        bpp: 4,
        pixman: Some(pixman::X8R8G8B8),
        gl_format: GL_BGRA_EXT,
        gl_type: GL_UNSIGNED_BYTE,
        drm: XRGB8888_DRM,
        wl_id: Some(XRGB8888_ID),
    },
    // Format {
    //     id: fourcc_code('C', '8', ' ', ' '),
    //     name: "c8",
    // },
    // Format {
    //     id: fourcc_code('R', '8', ' ', ' '),
    //     name: "r8",
    // },
    // Format {
    //     id: fourcc_code('R', '1', '6', ' '),
    //     name: "r16",
    // },
    // Format {
    //     id: fourcc_code('R', 'G', '8', '8'),
    //     name: "rg88",
    // },
    // Format {
    //     id: fourcc_code('G', 'R', '8', '8'),
    //     name: "gr88",
    // },
    // Format {
    //     id: fourcc_code('R', 'G', '3', '2'),
    //     name: "rg1616",
    // },
    // Format {
    //     id: fourcc_code('G', 'R', '3', '2'),
    //     name: "gr1616",
    // },
    // Format {
    //     id: fourcc_code('R', 'G', 'B', '8'),
    //     name: "rgb332",
    // },
    // Format {
    //     id: fourcc_code('B', 'G', 'R', '8'),
    //     name: "bgr233",
    // },
    // Format {
    //     id: fourcc_code('X', 'R', '1', '2'),
    //     name: "xrgb4444",
    // },
    // Format {
    //     id: fourcc_code('X', 'B', '1', '2'),
    //     name: "xbgr4444",
    // },
    // Format {
    //     id: fourcc_code('R', 'X', '1', '2'),
    //     name: "rgbx4444",
    // },
    // Format {
    //     id: fourcc_code('B', 'X', '1', '2'),
    //     name: "bgrx4444",
    // },
    // Format {
    //     id: fourcc_code('A', 'R', '1', '2'),
    //     name: "argb4444",
    // },
    // Format {
    //     id: fourcc_code('A', 'B', '1', '2'),
    //     name: "abgr4444",
    // },
    // Format {
    //     id: fourcc_code('R', 'A', '1', '2'),
    //     name: "rgba4444",
    // },
    // Format {
    //     id: fourcc_code('B', 'A', '1', '2'),
    //     name: "bgra4444",
    // },
    // Format {
    //     id: fourcc_code('X', 'R', '1', '5'),
    //     name: "xrgb1555",
    // },
    // Format {
    //     id: fourcc_code('X', 'B', '1', '5'),
    //     name: "xbgr1555",
    // },
    // Format {
    //     id: fourcc_code('R', 'X', '1', '5'),
    //     name: "rgbx5551",
    // },
    // Format {
    //     id: fourcc_code('B', 'X', '1', '5'),
    //     name: "bgrx5551",
    // },
    // Format {
    //     id: fourcc_code('A', 'R', '1', '5'),
    //     name: "argb1555",
    // },
    // Format {
    //     id: fourcc_code('A', 'B', '1', '5'),
    //     name: "abgr1555",
    // },
    // Format {
    //     id: fourcc_code('R', 'A', '1', '5'),
    //     name: "rgba5551",
    // },
    // Format {
    //     id: fourcc_code('B', 'A', '1', '5'),
    //     name: "bgra5551",
    // },
    // Format {
    //     id: fourcc_code('R', 'G', '1', '6'),
    //     name: "rgb565",
    // },
    // Format {
    //     id: fourcc_code('B', 'G', '1', '6'),
    //     name: "bgr565",
    // },
    // Format {
    //     id: fourcc_code('R', 'G', '2', '4'),
    //     name: "rgb888",
    // },
    // Format {
    //     id: fourcc_code('B', 'G', '2', '4'),
    //     name: "bgr888",
    // },
    // Format {
    //     id: fourcc_code('X', 'R', '2', '4'),
    //     name: "xrgb8888",
    // },
    // Format {
    //     id: fourcc_code('X', 'B', '2', '4'),
    //     name: "xbgr8888",
    // },
    // Format {
    //     id: fourcc_code('R', 'X', '2', '4'),
    //     name: "rgbx8888",
    // },
    // Format {
    //     id: fourcc_code('B', 'X', '2', '4'),
    //     name: "bgrx8888",
    // },
    // Format {
    //     id: fourcc_code('A', 'R', '2', '4'),
    //     name: "argb8888",
    // },
    // Format {
    //     id: fourcc_code('A', 'B', '2', '4'),
    //     name: "abgr8888",
    // },
    // Format {
    //     id: fourcc_code('R', 'A', '2', '4'),
    //     name: "rgba8888",
    // },
    // Format {
    //     id: fourcc_code('B', 'A', '2', '4'),
    //     name: "bgra8888",
    // },
    // Format {
    //     id: fourcc_code('X', 'R', '3', '0'),
    //     name: "xrgb2101010",
    // },
    // Format {
    //     id: fourcc_code('X', 'B', '3', '0'),
    //     name: "xbgr2101010",
    // },
    // Format {
    //     id: fourcc_code('R', 'X', '3', '0'),
    //     name: "rgbx1010102",
    // },
    // Format {
    //     id: fourcc_code('B', 'X', '3', '0'),
    //     name: "bgrx1010102",
    // },
    // Format {
    //     id: fourcc_code('A', 'R', '3', '0'),
    //     name: "argb2101010",
    // },
    // Format {
    //     id: fourcc_code('A', 'B', '3', '0'),
    //     name: "abgr2101010",
    // },
    // Format {
    //     id: fourcc_code('R', 'A', '3', '0'),
    //     name: "rgba1010102",
    // },
    // Format {
    //     id: fourcc_code('B', 'A', '3', '0'),
    //     name: "bgra1010102",
    // },
    // Format {
    //     id: fourcc_code('X', 'R', '4', '8'),
    //     name: "xrgb16161616",
    // },
    // Format {
    //     id: fourcc_code('X', 'B', '4', '8'),
    //     name: "xbgr16161616",
    // },
    // Format {
    //     id: fourcc_code('A', 'R', '4', '8'),
    //     name: "argb16161616",
    // },
    // Format {
    //     id: fourcc_code('A', 'B', '4', '8'),
    //     name: "abgr16161616",
    // },
    // Format {
    //     id: fourcc_code('X', 'R', '4', 'H'),
    //     name: "xrgb16161616f",
    // },
    // Format {
    //     id: fourcc_code('X', 'B', '4', 'H'),
    //     name: "xbgr16161616f",
    // },
    // Format {
    //     id: fourcc_code('A', 'R', '4', 'H'),
    //     name: "argb16161616f",
    // },
    // Format {
    //     id: fourcc_code('A', 'B', '4', 'H'),
    //     name: "abgr16161616f",
    // },
    // Format {
    //     id: fourcc_code('A', 'B', '1', '0'),
    //     name: "axbxgxrx106106106106",
    // },
    // Format {
    //     id: fourcc_code('Y', 'U', 'Y', 'V'),
    //     name: "yuyv",
    // },
    // Format {
    //     id: fourcc_code('Y', 'V', 'Y', 'U'),
    //     name: "yvyu",
    // },
    // Format {
    //     id: fourcc_code('U', 'Y', 'V', 'Y'),
    //     name: "uyvy",
    // },
    // Format {
    //     id: fourcc_code('V', 'Y', 'U', 'Y'),
    //     name: "vyuy",
    // },
    // Format {
    //     id: fourcc_code('A', 'Y', 'U', 'V'),
    //     name: "ayuv",
    // },
    // Format {
    //     id: fourcc_code('X', 'Y', 'U', 'V'),
    //     name: "xyuv8888",
    // },
    // Format {
    //     id: fourcc_code('V', 'U', '2', '4'),
    //     name: "vuy888",
    // },
    // Format {
    //     id: fourcc_code('V', 'U', '3', '0'),
    //     name: "vuy101010",
    // },
    // Format {
    //     id: fourcc_code('Y', '2', '1', '0'),
    //     name: "y210",
    // },
    // Format {
    //     id: fourcc_code('Y', '2', '1', '2'),
    //     name: "y212",
    // },
    // Format {
    //     id: fourcc_code('Y', '2', '1', '6'),
    //     name: "y216",
    // },
    // Format {
    //     id: fourcc_code('Y', '4', '1', '0'),
    //     name: "y410",
    // },
    // Format {
    //     id: fourcc_code('Y', '4', '1', '2'),
    //     name: "y412",
    // },
    // Format {
    //     id: fourcc_code('Y', '4', '1', '6'),
    //     name: "y416",
    // },
    // Format {
    //     id: fourcc_code('X', 'V', '3', '0'),
    //     name: "xvyu2101010",
    // },
    // Format {
    //     id: fourcc_code('X', 'V', '3', '6'),
    //     name: "xvyu12_16161616",
    // },
    // Format {
    //     id: fourcc_code('X', 'V', '4', '8'),
    //     name: "xvyu16161616",
    // },
    // Format {
    //     id: fourcc_code('Y', '0', 'L', '0'),
    //     name: "y0l0",
    // },
    // Format {
    //     id: fourcc_code('X', '0', 'L', '0'),
    //     name: "x0l0",
    // },
    // Format {
    //     id: fourcc_code('Y', '0', 'L', '2'),
    //     name: "y0l2",
    // },
    // Format {
    //     id: fourcc_code('X', '0', 'L', '2'),
    //     name: "x0l2",
    // },
    // Format {
    //     id: fourcc_code('Y', 'U', '0', '8'),
    //     name: "yuv420_8bit",
    // },
    // Format {
    //     id: fourcc_code('Y', 'U', '1', '0'),
    //     name: "yuv420_10bit",
    // },
    // Format {
    //     id: fourcc_code('X', 'R', 'A', '8'),
    //     name: "xrgb8888_a8",
    // },
    // Format {
    //     id: fourcc_code('X', 'B', 'A', '8'),
    //     name: "xbgr8888_a8",
    // },
    // Format {
    //     id: fourcc_code('R', 'X', 'A', '8'),
    //     name: "rgbx8888_a8",
    // },
    // Format {
    //     id: fourcc_code('B', 'X', 'A', '8'),
    //     name: "bgrx8888_a8",
    // },
    // Format {
    //     id: fourcc_code('R', '8', 'A', '8'),
    //     name: "rgb888_a8",
    // },
    // Format {
    //     id: fourcc_code('B', '8', 'A', '8'),
    //     name: "bgr888_a8",
    // },
    // Format {
    //     id: fourcc_code('R', '5', 'A', '8'),
    //     name: "rgb565_a8",
    // },
    // Format {
    //     id: fourcc_code('B', '5', 'A', '8'),
    //     name: "bgr565_a8",
    // },
    // Format {
    //     id: fourcc_code('N', 'V', '1', '2'),
    //     name: "nv12",
    // },
    // Format {
    //     id: fourcc_code('N', 'V', '2', '1'),
    //     name: "nv21",
    // },
    // Format {
    //     id: fourcc_code('N', 'V', '1', '6'),
    //     name: "nv16",
    // },
    // Format {
    //     id: fourcc_code('N', 'V', '6', '1'),
    //     name: "nv61",
    // },
    // Format {
    //     id: fourcc_code('N', 'V', '2', '4'),
    //     name: "nv24",
    // },
    // Format {
    //     id: fourcc_code('N', 'V', '4', '2'),
    //     name: "nv42",
    // },
    // Format {
    //     id: fourcc_code('N', 'V', '1', '5'),
    //     name: "nv15",
    // },
    // Format {
    //     id: fourcc_code('P', '2', '1', '0'),
    //     name: "p210",
    // },
    // Format {
    //     id: fourcc_code('P', '0', '1', '0'),
    //     name: "p010",
    // },
    // Format {
    //     id: fourcc_code('P', '0', '1', '2'),
    //     name: "p012",
    // },
    // Format {
    //     id: fourcc_code('P', '0', '1', '6'),
    //     name: "p016",
    // },
    // Format {
    //     id: fourcc_code('Q', '4', '1', '0'),
    //     name: "q410",
    // },
    // Format {
    //     id: fourcc_code('Q', '4', '0', '1'),
    //     name: "q401",
    // },
    // Format {
    //     id: fourcc_code('Y', 'U', 'V', '9'),
    //     name: "yuv410",
    // },
    // Format {
    //     id: fourcc_code('Y', 'V', 'U', '9'),
    //     name: "yvu410",
    // },
    // Format {
    //     id: fourcc_code('Y', 'U', '1', '1'),
    //     name: "yuv411",
    // },
    // Format {
    //     id: fourcc_code('Y', 'V', '1', '1'),
    //     name: "yvu411",
    // },
    // Format {
    //     id: fourcc_code('Y', 'U', '1', '2'),
    //     name: "yuv420",
    // },
    // Format {
    //     id: fourcc_code('Y', 'V', '1', '2'),
    //     name: "yvu420",
    // },
    // Format {
    //     id: fourcc_code('Y', 'U', '1', '6'),
    //     name: "yuv422",
    // },
    // Format {
    //     id: fourcc_code('Y', 'V', '1', '6'),
    //     name: "yvu422",
    // },
    // Format {
    //     id: fourcc_code('Y', 'U', '2', '4'),
    //     name: "yuv444",
    // },
    // Format {
    //     id: fourcc_code('Y', 'V', '2', '4'),
    //     name: "yvu444",
    // },
];
