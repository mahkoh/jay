#![allow(dead_code)]

pub const fn format(bpp: u32, ty: u32, a: u32, r: u32, g: u32, b: u32) -> i32 {
    ((bpp << 24) | (ty << 16) | (a << 12) | (r << 8) | (g << 4) | b) as i32
}

pub const fn format_byte(bpp: u32, ty: u32, a: u32, r: u32, g: u32, b: u32) -> i32 {
    (((bpp >> 3) << 24)
        | (3 << 22)
        | (ty << 16)
        | ((a >> 3) << 12)
        | ((r >> 3) << 8)
        | ((g >> 3) << 4)
        | (b >> 3)) as i32
}

pub const fn format_reshift(val: u32, ofs: u32, num: u32) -> u32 {
    ((val >> (ofs)) & ((1 << (num)) - 1)) << ((val >> 22) & 3)
}

pub const fn format_bpp(f: u32) -> u32 {
    format_reshift(f, 24, 8)
}

pub const fn format_shift(f: u32) -> u32 {
    (f >> 22) & 3
}

pub const fn format_type(f: u32) -> u32 {
    (f >> 16) & 0x3f
}

pub const fn format_a(f: u32) -> u32 {
    format_reshift(f, 12, 4)
}

pub const fn format_r(f: u32) -> u32 {
    format_reshift(f, 8, 4)
}

pub const fn format_g(f: u32) -> u32 {
    format_reshift(f, 4, 4)
}

pub const fn format_b(f: u32) -> u32 {
    format_reshift(f, 0, 4)
}

pub const fn format_rgb(f: u32) -> u32 {
    f & 0xfff
}

pub const fn format_vis(f: u32) -> u32 {
    f & 0xffff
}

pub const fn format_depth(f: u32) -> u32 {
    format_a(f) + format_r(f) + format_g(f) + format_b(f)
}

pub const TYPE_OTHER: u32 = 0;
pub const TYPE_A: u32 = 1;
pub const TYPE_ARGB: u32 = 2;
pub const TYPE_ABGR: u32 = 3;
pub const TYPE_COLOR: u32 = 4;
pub const TYPE_GRAY: u32 = 5;
pub const TYPE_YUY2: u32 = 6;
pub const TYPE_YV12: u32 = 7;
pub const TYPE_BGRA: u32 = 8;
pub const TYPE_RGBA: u32 = 9;
pub const TYPE_ARGB_SRGB: u32 = 10;
pub const TYPE_RGBA_FLOAT: u32 = 11;

pub const fn format_color(f: u32) -> bool {
    format_type(f) == TYPE_ARGB
        || format_type(f) == TYPE_ABGR
        || format_type(f) == TYPE_BGRA
        || format_type(f) == TYPE_RGBA
        || format_type(f) == TYPE_RGBA_FLOAT
}

cenum! {
    Format, FORMATS;

    RGBA_FLOAT    = format_byte(128, TYPE_RGBA_FLOAT, 32, 32, 32, 32),
    RGB_FLOAT     = format_byte(96,  TYPE_RGBA_FLOAT, 0,  32, 32, 32),
    A8R8G8B8      = format(32,       TYPE_ARGB,       8,  8,  8,  8),
    X8R8G8B8      = format(32,       TYPE_ARGB,       0,  8,  8,  8),
    A8B8G8R8      = format(32,       TYPE_ABGR,       8,  8,  8,  8),
    X8B8G8R8      = format(32,       TYPE_ABGR,       0,  8,  8,  8),
    B8G8R8A8      = format(32,       TYPE_BGRA,       8,  8,  8,  8),
    B8G8R8X8      = format(32,       TYPE_BGRA,       0,  8,  8,  8),
    R8G8B8A8      = format(32,       TYPE_RGBA,       8,  8,  8,  8),
    R8G8B8X8      = format(32,       TYPE_RGBA,       0,  8,  8,  8),
    X14R6G6B6     = format(32,       TYPE_ARGB,       0,  6,  6,  6),
    X2R10G10B10   = format(32,       TYPE_ARGB,       0,  10, 10, 10),
    A2R10G10B10   = format(32,       TYPE_ARGB,       2,  10, 10, 10),
    X2B10G10R10   = format(32,       TYPE_ABGR,       0,  10, 10, 10),
    A2B10G10R10   = format(32,       TYPE_ABGR,       2,  10, 10, 10),
    A8R8G8B8_SRGB = format(32,       TYPE_ARGB_SRGB,  8,  8,  8,  8),
    R8G8B8        = format(24,       TYPE_ARGB,       0,  8,  8,  8),
    B8G8R8        = format(24,       TYPE_ABGR,       0,  8,  8,  8),
    R5G6B5        = format(16,       TYPE_ARGB,       0,  5,  6,  5),
    B5G6R5        = format(16,       TYPE_ABGR,       0,  5,  6,  5),
    A1R5G5B5      = format(16,       TYPE_ARGB,       1,  5,  5,  5),
    X1R5G5B5      = format(16,       TYPE_ARGB,       0,  5,  5,  5),
    A1B5G5R5      = format(16,       TYPE_ABGR,       1,  5,  5,  5),
    X1B5G5R5      = format(16,       TYPE_ABGR,       0,  5,  5,  5),
    A4R4G4B4      = format(16,       TYPE_ARGB,       4,  4,  4,  4),
    X4R4G4B4      = format(16,       TYPE_ARGB,       0,  4,  4,  4),
    A4B4G4R4      = format(16,       TYPE_ABGR,       4,  4,  4,  4),
    X4B4G4R4      = format(16,       TYPE_ABGR,       0,  4,  4,  4),
    A8            = format(8,        TYPE_A,          8,  0,  0,  0),
    R3G3B2        = format(8,        TYPE_ARGB,       0,  3,  3,  2),
    B2G3R3        = format(8,        TYPE_ABGR,       0,  3,  3,  2),
    A2R2G2B2      = format(8,        TYPE_ARGB,       2,  2,  2,  2),
    A2B2G2R2      = format(8,        TYPE_ABGR,       2,  2,  2,  2),
    C8            = format(8,        TYPE_COLOR,      0,  0,  0,  0),
    G8            = format(8,        TYPE_GRAY,       0,  0,  0,  0),
    X4A4          = format(8,        TYPE_A,          4,  0,  0,  0),
    X4C4          = format(8,        TYPE_COLOR,      0,  0,  0,  0),
    X4G4          = format(8,        TYPE_GRAY,       0,  0,  0,  0),
    A4            = format(4,        TYPE_A,          4,  0,  0,  0),
    R1G2B1        = format(4,        TYPE_ARGB,       0,  1,  2,  1),
    B1G2R1        = format(4,        TYPE_ABGR,       0,  1,  2,  1),
    A1R1G1B1      = format(4,        TYPE_ARGB,       1,  1,  1,  1),
    A1B1G1R1      = format(4,        TYPE_ABGR,       1,  1,  1,  1),
    C4            = format(4,        TYPE_COLOR,      0,  0,  0,  0),
    G4            = format(4,        TYPE_GRAY,       0,  0,  0,  0),
    A1            = format(1,        TYPE_A,          1,  0,  0,  0),
    G1            = format(1,        TYPE_GRAY,       0,  0,  0,  0),
    YUY2          = format(16,       TYPE_YUY2,       0,  0,  0,  0),
    YV12          = format(12,       TYPE_YV12,       0,  0,  0,  0),
}

cenum! {
    Op, OPS;

    OP_CLEAR                 = 0x00,
    OP_SRC                   = 0x01,
    OP_DST                   = 0x02,
    OP_OVER                  = 0x03,
    OP_OVER_REVERSE          = 0x04,
    OP_IN                    = 0x05,
    OP_IN_REVERSE            = 0x06,
    OP_OUT                   = 0x07,
    OP_OUT_REVERSE           = 0x08,
    OP_ATOP                  = 0x09,
    OP_ATOP_REVERSE          = 0x0a,
    OP_XOR                   = 0x0b,
    OP_ADD                   = 0x0c,
    OP_SATURATE              = 0x0d,
    OP_DISJOINT_CLEAR        = 0x10,
    OP_DISJOINT_SRC          = 0x11,
    OP_DISJOINT_DST          = 0x12,
    OP_DISJOINT_OVER         = 0x13,
    OP_DISJOINT_OVER_REVERSE = 0x14,
    OP_DISJOINT_IN           = 0x15,
    OP_DISJOINT_IN_REVERSE   = 0x16,
    OP_DISJOINT_OUT          = 0x17,
    OP_DISJOINT_OUT_REVERSE  = 0x18,
    OP_DISJOINT_ATOP         = 0x19,
    OP_DISJOINT_ATOP_REVERSE = 0x1a,
    OP_DISJOINT_XOR          = 0x1b,
    OP_CONJOINT_CLEAR        = 0x20,
    OP_CONJOINT_SRC          = 0x21,
    OP_CONJOINT_DST          = 0x22,
    OP_CONJOINT_OVER         = 0x23,
    OP_CONJOINT_OVER_REVERSE = 0x24,
    OP_CONJOINT_IN           = 0x25,
    OP_CONJOINT_IN_REVERSE   = 0x26,
    OP_CONJOINT_OUT          = 0x27,
    OP_CONJOINT_OUT_REVERSE  = 0x28,
    OP_CONJOINT_ATOP         = 0x29,
    OP_CONJOINT_ATOP_REVERSE = 0x2a,
    OP_CONJOINT_XOR          = 0x2b,
    OP_MULTIPLY              = 0x30,
    OP_SCREEN                = 0x31,
    OP_OVERLAY               = 0x32,
    OP_DARKEN                = 0x33,
    OP_LIGHTEN               = 0x34,
    OP_COLOR_DODGE           = 0x35,
    OP_COLOR_BURN            = 0x36,
    OP_HARD_LIGHT            = 0x37,
    OP_SOFT_LIGHT            = 0x38,
    OP_DIFFERENCE            = 0x39,
    OP_EXCLUSION             = 0x3a,
    OP_HSL_HUE               = 0x3b,
    OP_HSL_SATURATION        = 0x3c,
    OP_HSL_COLOR             = 0x3d,
    OP_HSL_LUMINOSITY        = 0x3e,
}
