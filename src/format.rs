use {
    crate::{
        gfx_apis::gl::sys::{GL_BGRA_EXT, GL_RGBA, GL_RGBA8, GL_UNSIGNED_BYTE, GLenum, GLint},
        pipewire::pw_pod::{
            SPA_VIDEO_FORMAT_ABGR_210LE, SPA_VIDEO_FORMAT_ARGB_210LE, SPA_VIDEO_FORMAT_BGR,
            SPA_VIDEO_FORMAT_BGR15, SPA_VIDEO_FORMAT_BGR16, SPA_VIDEO_FORMAT_BGRA,
            SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_GRAY8, SPA_VIDEO_FORMAT_RGB,
            SPA_VIDEO_FORMAT_RGB16, SPA_VIDEO_FORMAT_RGBA, SPA_VIDEO_FORMAT_RGBx,
            SPA_VIDEO_FORMAT_UNKNOWN, SPA_VIDEO_FORMAT_xBGR_210LE, SPA_VIDEO_FORMAT_xRGB_210LE,
            SpaVideoFormat,
        },
        utils::debug_fn::debug_fn,
    },
    ahash::AHashMap,
    ash::vk,
    jay_config::video::Format as ConfigFormat,
    once_cell::sync::Lazy,
    std::fmt::{Debug, Write},
};

#[derive(Copy, Clone, Debug)]
pub struct FormatShmInfo {
    pub bpp: u32,
    pub gl_format: GLint,
    pub gl_internal_format: GLenum,
    pub gl_type: GLint,
}

#[derive(Copy, Clone, Debug)]
pub struct Format {
    pub name: &'static str,
    pub vk_format: vk::Format,
    pub drm: u32,
    pub wl_id: Option<u32>,
    pub external_only_guess: bool,
    pub has_alpha: bool,
    pub pipewire: SpaVideoFormat,
    pub opaque: Option<&'static Format>,
    pub shm_info: Option<FormatShmInfo>,
    pub config: ConfigFormat,
}

const fn default(config: ConfigFormat) -> Format {
    Format {
        name: "",
        vk_format: vk::Format::UNDEFINED,
        drm: 0,
        wl_id: None,
        external_only_guess: false,
        has_alpha: false,
        pipewire: SPA_VIDEO_FORMAT_UNKNOWN,
        opaque: None,
        shm_info: None,
        config,
    }
}

impl PartialEq for Format {
    fn eq(&self, other: &Self) -> bool {
        self.drm == other.drm
    }
}

impl Eq for Format {}

static FORMATS_MAP: Lazy<AHashMap<u32, &'static Format>> = Lazy::new(|| {
    let mut map = AHashMap::new();
    for format in FORMATS {
        assert!(map.insert(format.drm, format).is_none());
    }
    map
});

static PW_FORMATS_MAP: Lazy<AHashMap<SpaVideoFormat, &'static Format>> = Lazy::new(|| {
    let mut map = AHashMap::new();
    for format in FORMATS {
        if format.pipewire != SPA_VIDEO_FORMAT_UNKNOWN {
            assert!(map.insert(format.pipewire, format).is_none());
        }
    }
    map
});

static FORMATS_REFS: Lazy<Vec<&'static Format>> = Lazy::new(|| FORMATS.iter().collect());

static FORMATS_NAMES: Lazy<AHashMap<&'static str, &'static Format>> = Lazy::new(|| {
    let mut map = AHashMap::new();
    for format in FORMATS {
        assert!(map.insert(format.name, format).is_none());
    }
    map
});

static FORMATS_CONFIG: Lazy<AHashMap<ConfigFormat, &'static Format>> = Lazy::new(|| {
    let mut map = AHashMap::new();
    for format in FORMATS {
        assert!(map.insert(format.config, format).is_none());
    }
    map
});

#[test]
fn formats_dont_panic() {
    formats();
    pw_formats();
    named_formats();
    config_formats();
}

pub fn formats() -> &'static AHashMap<u32, &'static Format> {
    &FORMATS_MAP
}

pub fn pw_formats() -> &'static AHashMap<SpaVideoFormat, &'static Format> {
    &PW_FORMATS_MAP
}

pub fn ref_formats() -> &'static [&'static Format] {
    &FORMATS_REFS
}

pub fn named_formats() -> &'static AHashMap<&'static str, &'static Format> {
    &FORMATS_NAMES
}

pub fn config_formats() -> &'static AHashMap<ConfigFormat, &'static Format> {
    &FORMATS_CONFIG
}

const fn fourcc_code(a: char, b: char, c: char, d: char) -> u32 {
    (a as u32) | ((b as u32) << 8) | ((c as u32) << 16) | ((d as u32) << 24)
}

#[expect(dead_code)]
pub fn debug(fourcc: u32) -> impl Debug {
    debug_fn(move |fmt| {
        fmt.write_char(fourcc as u8 as char)?;
        fmt.write_char((fourcc >> 8) as u8 as char)?;
        fmt.write_char((fourcc >> 16) as u8 as char)?;
        fmt.write_char((fourcc >> 24) as u8 as char)?;
        Ok(())
    })
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

pub static ARGB8888: &Format = &Format {
    name: "argb8888",
    shm_info: Some(FormatShmInfo {
        bpp: 4,
        gl_format: GL_BGRA_EXT,
        gl_internal_format: GL_RGBA8,
        gl_type: GL_UNSIGNED_BYTE,
    }),
    vk_format: vk::Format::B8G8R8A8_UNORM,
    drm: ARGB8888_DRM,
    wl_id: Some(ARGB8888_ID),
    external_only_guess: false,
    has_alpha: true,
    pipewire: SPA_VIDEO_FORMAT_BGRA,
    opaque: Some(XRGB8888),
    config: ConfigFormat::ARGB8888,
};

pub static XRGB8888: &Format = &Format {
    name: "xrgb8888",
    shm_info: Some(FormatShmInfo {
        bpp: 4,
        gl_format: GL_BGRA_EXT,
        gl_internal_format: GL_RGBA8,
        gl_type: GL_UNSIGNED_BYTE,
    }),
    vk_format: vk::Format::B8G8R8A8_UNORM,
    drm: XRGB8888_DRM,
    wl_id: Some(XRGB8888_ID),
    external_only_guess: false,
    has_alpha: false,
    pipewire: SPA_VIDEO_FORMAT_BGRx,
    opaque: None,
    config: ConfigFormat::XRGB8888,
};

static ABGR8888: &Format = &Format {
    name: "abgr8888",
    shm_info: Some(FormatShmInfo {
        bpp: 4,
        gl_format: GL_RGBA,
        gl_internal_format: GL_RGBA8,
        gl_type: GL_UNSIGNED_BYTE,
    }),
    vk_format: vk::Format::R8G8B8A8_UNORM,
    drm: fourcc_code('A', 'B', '2', '4'),
    wl_id: None,
    external_only_guess: false,
    has_alpha: true,
    pipewire: SPA_VIDEO_FORMAT_RGBA,
    opaque: Some(XBGR8888),
    config: ConfigFormat::ABGR8888,
};

static XBGR8888: &Format = &Format {
    name: "xbgr8888",
    shm_info: Some(FormatShmInfo {
        bpp: 4,
        gl_format: GL_RGBA,
        gl_internal_format: GL_RGBA8,
        gl_type: GL_UNSIGNED_BYTE,
    }),
    vk_format: vk::Format::R8G8B8A8_UNORM,
    drm: fourcc_code('X', 'B', '2', '4'),
    wl_id: None,
    external_only_guess: false,
    has_alpha: false,
    pipewire: SPA_VIDEO_FORMAT_RGBx,
    opaque: None,
    config: ConfigFormat::XBGR8888,
};

static R8: &Format = &Format {
    name: "r8",
    vk_format: vk::Format::R8_UNORM,
    drm: fourcc_code('R', '8', ' ', ' '),
    pipewire: SPA_VIDEO_FORMAT_GRAY8,
    ..default(ConfigFormat::R8)
};

static GR88: &Format = &Format {
    name: "gr88",
    vk_format: vk::Format::R8G8_UNORM,
    drm: fourcc_code('G', 'R', '8', '8'),
    ..default(ConfigFormat::GR88)
};

static RGB888: &Format = &Format {
    name: "rgb888",
    vk_format: vk::Format::B8G8R8_UNORM,
    drm: fourcc_code('R', 'G', '2', '4'),
    pipewire: SPA_VIDEO_FORMAT_BGR,
    ..default(ConfigFormat::RGB888)
};

static BGR888: &Format = &Format {
    name: "bgr888",
    vk_format: vk::Format::R8G8B8_UNORM,
    drm: fourcc_code('B', 'G', '2', '4'),
    pipewire: SPA_VIDEO_FORMAT_RGB,
    ..default(ConfigFormat::BGR888)
};

static RGBA4444: &Format = &Format {
    name: "rgba4444",
    vk_format: vk::Format::R4G4B4A4_UNORM_PACK16,
    drm: fourcc_code('R', 'A', '1', '2'),
    has_alpha: true,
    opaque: Some(RGBX4444),
    ..default(ConfigFormat::RGBA4444)
};

static RGBX4444: &Format = &Format {
    name: "rgbx4444",
    vk_format: vk::Format::R4G4B4A4_UNORM_PACK16,
    drm: fourcc_code('R', 'X', '1', '2'),
    ..default(ConfigFormat::RGBX4444)
};

static BGRA4444: &Format = &Format {
    name: "bgra4444",
    vk_format: vk::Format::B4G4R4A4_UNORM_PACK16,
    drm: fourcc_code('B', 'A', '1', '2'),
    has_alpha: true,
    opaque: Some(BGRX4444),
    ..default(ConfigFormat::BGRA4444)
};

static BGRX4444: &Format = &Format {
    name: "bgrx4444",
    vk_format: vk::Format::B4G4R4A4_UNORM_PACK16,
    drm: fourcc_code('B', 'X', '1', '2'),
    ..default(ConfigFormat::BGRX4444)
};

static RGB565: &Format = &Format {
    name: "rgb565",
    vk_format: vk::Format::R5G6B5_UNORM_PACK16,
    drm: fourcc_code('R', 'G', '1', '6'),
    pipewire: SPA_VIDEO_FORMAT_BGR16,
    ..default(ConfigFormat::RGB565)
};

static BGR565: &Format = &Format {
    name: "bgr565",
    vk_format: vk::Format::B5G6R5_UNORM_PACK16,
    drm: fourcc_code('B', 'G', '1', '6'),
    pipewire: SPA_VIDEO_FORMAT_RGB16,
    ..default(ConfigFormat::BGR565)
};

static RGBA5551: &Format = &Format {
    name: "rgba5551",
    vk_format: vk::Format::R5G5B5A1_UNORM_PACK16,
    drm: fourcc_code('R', 'A', '1', '5'),
    has_alpha: true,
    opaque: Some(RGBX5551),
    ..default(ConfigFormat::RGBA5551)
};

static RGBX5551: &Format = &Format {
    name: "rgbx5551",
    vk_format: vk::Format::R5G5B5A1_UNORM_PACK16,
    drm: fourcc_code('R', 'X', '1', '5'),
    ..default(ConfigFormat::RGBX5551)
};

static BGRA5551: &Format = &Format {
    name: "bgra5551",
    vk_format: vk::Format::B5G5R5A1_UNORM_PACK16,
    drm: fourcc_code('B', 'A', '1', '5'),
    has_alpha: true,
    opaque: Some(BGRX5551),
    ..default(ConfigFormat::BGRA5551)
};

static BGRX5551: &Format = &Format {
    name: "bgrx5551",
    vk_format: vk::Format::B5G5R5A1_UNORM_PACK16,
    drm: fourcc_code('B', 'X', '1', '5'),
    ..default(ConfigFormat::BGRX5551)
};

static ARGB1555: &Format = &Format {
    name: "argb1555",
    vk_format: vk::Format::A1R5G5B5_UNORM_PACK16,
    drm: fourcc_code('A', 'R', '1', '5'),
    has_alpha: true,
    opaque: Some(XRGB1555),
    ..default(ConfigFormat::ARGB1555)
};

static XRGB1555: &Format = &Format {
    name: "xrgb1555",
    vk_format: vk::Format::A1R5G5B5_UNORM_PACK16,
    drm: fourcc_code('X', 'R', '1', '5'),
    pipewire: SPA_VIDEO_FORMAT_BGR15,
    ..default(ConfigFormat::XRGB1555)
};

static ARGB2101010: &Format = &Format {
    name: "argb2101010",
    vk_format: vk::Format::A2R10G10B10_UNORM_PACK32,
    drm: fourcc_code('A', 'R', '3', '0'),
    has_alpha: true,
    opaque: Some(XRGB2101010),
    pipewire: SPA_VIDEO_FORMAT_ARGB_210LE,
    ..default(ConfigFormat::ARGB2101010)
};

static XRGB2101010: &Format = &Format {
    name: "xrgb2101010",
    vk_format: vk::Format::A2R10G10B10_UNORM_PACK32,
    drm: fourcc_code('X', 'R', '3', '0'),
    pipewire: SPA_VIDEO_FORMAT_xRGB_210LE,
    ..default(ConfigFormat::XRGB2101010)
};

static ABGR2101010: &Format = &Format {
    name: "abgr2101010",
    vk_format: vk::Format::A2B10G10R10_UNORM_PACK32,
    drm: fourcc_code('A', 'B', '3', '0'),
    has_alpha: true,
    opaque: Some(XBGR2101010),
    pipewire: SPA_VIDEO_FORMAT_ABGR_210LE,
    ..default(ConfigFormat::ABGR2101010)
};

static XBGR2101010: &Format = &Format {
    name: "xbgr2101010",
    vk_format: vk::Format::A2B10G10R10_UNORM_PACK32,
    drm: fourcc_code('X', 'B', '3', '0'),
    pipewire: SPA_VIDEO_FORMAT_xBGR_210LE,
    ..default(ConfigFormat::XBGR2101010)
};

static ABGR16161616: &Format = &Format {
    name: "abgr16161616",
    vk_format: vk::Format::R16G16B16A16_UNORM,
    drm: fourcc_code('A', 'B', '4', '8'),
    has_alpha: true,
    opaque: Some(XBGR16161616),
    ..default(ConfigFormat::ABGR16161616)
};

static XBGR16161616: &Format = &Format {
    name: "xbgr16161616",
    vk_format: vk::Format::R16G16B16A16_UNORM,
    drm: fourcc_code('X', 'B', '4', '8'),
    ..default(ConfigFormat::XBGR16161616)
};

static ABGR16161616F: &Format = &Format {
    name: "abgr16161616f",
    vk_format: vk::Format::R16G16B16A16_SFLOAT,
    drm: fourcc_code('A', 'B', '4', 'H'),
    has_alpha: true,
    opaque: Some(XBGR16161616F),
    ..default(ConfigFormat::ABGR16161616F)
};

static XBGR16161616F: &Format = &Format {
    name: "xbgr16161616f",
    vk_format: vk::Format::R16G16B16A16_SFLOAT,
    drm: fourcc_code('X', 'B', '4', 'H'),
    ..default(ConfigFormat::XBGR16161616F)
};

pub static FORMATS: &[Format] = &[
    *ARGB8888,
    *XRGB8888,
    *ABGR8888,
    *XBGR8888,
    *R8,
    *GR88,
    *RGB888,
    *BGR888,
    #[cfg(target_endian = "little")]
    *RGBA4444,
    #[cfg(target_endian = "little")]
    *RGBX4444,
    #[cfg(target_endian = "little")]
    *BGRA4444,
    #[cfg(target_endian = "little")]
    *BGRX4444,
    #[cfg(target_endian = "little")]
    *RGB565,
    #[cfg(target_endian = "little")]
    *BGR565,
    #[cfg(target_endian = "little")]
    *RGBA5551,
    #[cfg(target_endian = "little")]
    *RGBX5551,
    #[cfg(target_endian = "little")]
    *BGRA5551,
    #[cfg(target_endian = "little")]
    *BGRX5551,
    #[cfg(target_endian = "little")]
    *ARGB1555,
    #[cfg(target_endian = "little")]
    *XRGB1555,
    #[cfg(target_endian = "little")]
    *ARGB2101010,
    #[cfg(target_endian = "little")]
    *XRGB2101010,
    #[cfg(target_endian = "little")]
    *ABGR2101010,
    #[cfg(target_endian = "little")]
    *XBGR2101010,
    #[cfg(target_endian = "little")]
    *ABGR16161616,
    #[cfg(target_endian = "little")]
    *XBGR16161616,
    #[cfg(target_endian = "little")]
    *ABGR16161616F,
    #[cfg(target_endian = "little")]
    *XBGR16161616F,
];
