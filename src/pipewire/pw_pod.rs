#![allow(non_upper_case_globals, non_camel_case_types)]

mod pw_debug;

use {
    crate::pipewire::pw_parser::{PwParser, PwParserError},
    bstr::BStr,
    std::{
        fmt::{Debug, Formatter},
        sync::atomic::AtomicU32,
    },
    uapi::{c, Pod},
};

macro_rules! ty {
    ($name:ident; $($id:ident = $val:expr,)*) => {
        #[derive(Copy, Clone, Eq, PartialEq, Hash)]
        #[repr(transparent)]
        pub struct $name(pub u32);

        $(
            pub const $id: $name = $name($val);
        )*

        impl $name {
            pub fn name(self) -> Option<&'static str> {
                let res = match self {
                    $(
                        $id => stringify!($id),
                    )*
                    _ => return None,
                };
                Some(res)
            }
        }

        impl Debug for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                match self.name() {
                    Some(n) => write!(f, "{}", n),
                    _ => write!(f, "{}({})", stringify!($name), self.0),
                }
            }
        }
    }
}

ty! {
    PwPodType;

    PW_TYPE_None      = 0x01,
    PW_TYPE_Bool      = 0x02,
    PW_TYPE_Id        = 0x03,
    PW_TYPE_Int       = 0x04,
    PW_TYPE_Long      = 0x05,
    PW_TYPE_Float     = 0x06,
    PW_TYPE_Double    = 0x07,
    PW_TYPE_String    = 0x08,
    PW_TYPE_Bytes     = 0x09,
    PW_TYPE_Rectangle = 0x0A,
    PW_TYPE_Fraction  = 0x0B,
    PW_TYPE_Bitmap    = 0x0C,
    PW_TYPE_Array     = 0x0D,
    PW_TYPE_Struct    = 0x0E,
    PW_TYPE_Object    = 0x0F,
    PW_TYPE_Sequence  = 0x10,
    PW_TYPE_Pointer   = 0x11,
    PW_TYPE_Fd        = 0x12,
    PW_TYPE_Choice    = 0x13,
    PW_TYPE_Pod       = 0x14,
}

ty! {
    PwPodObjectType;

    PW_COMMAND_Device = 0x30001,
    PW_COMMAND_Node = 0x30002,

    PW_OBJECT_PropInfo             = 0x40001,
    PW_OBJECT_Props                = 0x40002,
    PW_OBJECT_Format               = 0x40003,
    PW_OBJECT_ParamBuffers         = 0x40004,
    PW_OBJECT_ParamMeta            = 0x40005,
    PW_OBJECT_ParamIO              = 0x40006,
    PW_OBJECT_ParamProfile         = 0x40007,
    PW_OBJECT_ParamPortConfig      = 0x40008,
    PW_OBJECT_ParamRoute           = 0x40009,
    PW_OBJECT_Profiler             = 0x4000A,
    PW_OBJECT_ParamLatency         = 0x4000B,
    PW_OBJECT_ParamProcessLatency  = 0x4000C,
}

ty! {
    SpaParamType;

    SPA_PARAM_Invalid         = 0,
    SPA_PARAM_PropInfo        = 1,
    SPA_PARAM_Props           = 2,
    SPA_PARAM_EnumFormat      = 3,
    SPA_PARAM_Format          = 4,
    SPA_PARAM_Buffers         = 5,
    SPA_PARAM_Meta            = 6,
    SPA_PARAM_IO              = 7,
    SPA_PARAM_EnumProfile     = 8,
    SPA_PARAM_Profile         = 9,
    SPA_PARAM_EnumPortConfig  = 10,
    SPA_PARAM_PortConfig      = 11,
    SPA_PARAM_EnumRoute       = 12,
    SPA_PARAM_Route           = 13,
    SPA_PARAM_Control         = 14,
    SPA_PARAM_Latency         = 15,
    SPA_PARAM_ProcessLatency  = 16,
}

ty! {
    SpaFormat;

    SPA_FORMAT_START                   = 0x00000,

    SPA_FORMAT_mediaType               = 0x00001,
    SPA_FORMAT_mediaSubtype            = 0x00002,

    SPA_FORMAT_START_Audio             = 0x10000,
    SPA_FORMAT_AUDIO_format            = 0x10001,
    SPA_FORMAT_AUDIO_flags             = 0x10002,
    SPA_FORMAT_AUDIO_rate              = 0x10003,
    SPA_FORMAT_AUDIO_channels          = 0x10004,
    SPA_FORMAT_AUDIO_position          = 0x10005,
    SPA_FORMAT_AUDIO_iec958Codec       = 0x10006,
    SPA_FORMAT_AUDIO_bitorder          = 0x10007,
    SPA_FORMAT_AUDIO_interleave        = 0x10008,

    SPA_FORMAT_START_Video             = 0x20000,
    SPA_FORMAT_VIDEO_format            = 0x20001,
    SPA_FORMAT_VIDEO_modifier          = 0x20002,
    SPA_FORMAT_VIDEO_size              = 0x20003,
    SPA_FORMAT_VIDEO_framerate         = 0x20004,
    SPA_FORMAT_VIDEO_maxFramerate      = 0x20005,
    SPA_FORMAT_VIDEO_views             = 0x20006,
    SPA_FORMAT_VIDEO_interlaceMode     = 0x20007,
    SPA_FORMAT_VIDEO_pixelAspectRatio  = 0x20008,
    SPA_FORMAT_VIDEO_multiviewMode     = 0x20009,
    SPA_FORMAT_VIDEO_multiviewFlags    = 0x2000A,
    SPA_FORMAT_VIDEO_chromaSite        = 0x2000B,
    SPA_FORMAT_VIDEO_colorRange        = 0x2000C,
    SPA_FORMAT_VIDEO_colorMatrix       = 0x2000D,
    SPA_FORMAT_VIDEO_transferFunction  = 0x2000E,
    SPA_FORMAT_VIDEO_colorPrimaries    = 0x2000F,
    SPA_FORMAT_VIDEO_profile           = 0x20010,
    SPA_FORMAT_VIDEO_level             = 0x20011,
    SPA_FORMAT_VIDEO_H264_streamFormat = 0x20012,
    SPA_FORMAT_VIDEO_H264_alignment    = 0x20013,

    SPA_FORMAT_START_Image             = 0x30000,
    SPA_FORMAT_START_Binary            = 0x40000,
    SPA_FORMAT_START_Stream            = 0x50000,
    SPA_FORMAT_START_Application       = 0x60000,
}

bitflags! {
    SPA_PARAM_INFO: u32;

    SPA_PARAM_INFO_SERIAL = 1<<0,
    SPA_PARAM_INFO_READ   = 1<<1,
    SPA_PARAM_INFO_WRITE  = 1<<2,
}

ty! {
    PwControlType;

    PW_CONTROL_PropInfo = 1,
    PW_CONTROL_Props = 2,
    PW_CONTROL_Format = 3,
}

ty! {
    PwPointerType;

    PW_POINTER_Buffer = 0x10001,
    PW_POINTER_Meta = 0x10002,
    PW_POINTER_Dict = 0x10003,
}

ty! {
    PwChoiceType;

    PW_CHOICE_None = 0,
    PW_CHOICE_Range = 1,
    PW_CHOICE_Step = 2,
    PW_CHOICE_Enum = 3,
    PW_CHOICE_Flags = 4,
}

ty! {
    PwIoType;

    PW_IO_Buffers = 1,
    PW_IO_Range = 2,
    PW_IO_Clock = 3,
    PW_IO_Latency = 4,
    PW_IO_Control = 5,
    PW_IO_Notify = 6,
    PW_IO_Position = 7,
    PW_IO_RateMatch = 8,
    PW_IO_Memory = 9,
}

bitflags! {
    PwPropFlag: u32;

    PW_PROP_READONLY = 1 << 0,
    PW_PROP_HARDWARE = 1 << 1,
    PW_PROP_HINT_DICT = 1 << 2,
    PW_PROP_MANDATORY = 1 << 3,
    PW_PROP_DONT_FIXATE = 1 << 4,
}

ty! {
    SpaMediaType;

    SPA_MEDIA_TYPE_unknown = 0,
    SPA_MEDIA_TYPE_audio = 1,
    SPA_MEDIA_TYPE_video = 2,
    SPA_MEDIA_TYPE_image = 3,
    SPA_MEDIA_TYPE_binary = 4,
    SPA_MEDIA_TYPE_stream = 5,
    SPA_MEDIA_TYPE_application = 6,
}

ty! {
    SpaMediaSubtype;

    SPA_MEDIA_SUBTYPE_unknown           = 0x00000,
    SPA_MEDIA_SUBTYPE_raw               = 0x00001,
    SPA_MEDIA_SUBTYPE_dsp               = 0x00002,
    SPA_MEDIA_SUBTYPE_iec958            = 0x00003,
    SPA_MEDIA_SUBTYPE_dsd               = 0x00004,

    SPA_MEDIA_SUBTYPE_START_Audio       = 0x10000,
    SPA_MEDIA_SUBTYPE_mp3               = 0x10001,
    SPA_MEDIA_SUBTYPE_aac               = 0x10002,
    SPA_MEDIA_SUBTYPE_vorbis            = 0x10003,
    SPA_MEDIA_SUBTYPE_wma               = 0x10004,
    SPA_MEDIA_SUBTYPE_ra                = 0x10005,
    SPA_MEDIA_SUBTYPE_sbc               = 0x10006,
    SPA_MEDIA_SUBTYPE_adpcm             = 0x10007,
    SPA_MEDIA_SUBTYPE_g723              = 0x10008,
    SPA_MEDIA_SUBTYPE_g726              = 0x10009,
    SPA_MEDIA_SUBTYPE_g729              = 0x1000A,
    SPA_MEDIA_SUBTYPE_amr               = 0x1000B,
    SPA_MEDIA_SUBTYPE_gsm               = 0x1000C,

    SPA_MEDIA_SUBTYPE_START_Video       = 0x20000,
    SPA_MEDIA_SUBTYPE_h264              = 0x20001,
    SPA_MEDIA_SUBTYPE_mjpg              = 0x20002,
    SPA_MEDIA_SUBTYPE_dv                = 0x20003,
    SPA_MEDIA_SUBTYPE_mpegts            = 0x20004,
    SPA_MEDIA_SUBTYPE_h263              = 0x20005,
    SPA_MEDIA_SUBTYPE_mpeg1             = 0x20006,
    SPA_MEDIA_SUBTYPE_mpeg2             = 0x20007,
    SPA_MEDIA_SUBTYPE_mpeg4             = 0x20008,
    SPA_MEDIA_SUBTYPE_xvid              = 0x20009,
    SPA_MEDIA_SUBTYPE_vc1               = 0x2000A,
    SPA_MEDIA_SUBTYPE_vp8               = 0x2000B,
    SPA_MEDIA_SUBTYPE_vp9               = 0x2000C,
    SPA_MEDIA_SUBTYPE_bayer             = 0x2000D,

    SPA_MEDIA_SUBTYPE_START_Image       = 0x30000,
    SPA_MEDIA_SUBTYPE_jpeg              = 0x30001,

    SPA_MEDIA_SUBTYPE_START_Binary      = 0x40000,

    SPA_MEDIA_SUBTYPE_START_Stream      = 0x50000,
    SPA_MEDIA_SUBTYPE_midi              = 0x50001,

    SPA_MEDIA_SUBTYPE_START_Application = 0x60000,
    SPA_MEDIA_SUBTYPE_control           = 0x60001,
}

ty! {
    SpaAudioFormat;

    SPA_AUDIO_FORMAT_UNKNOWN           = 0x000,
    SPA_AUDIO_FORMAT_ENCODED           = 0x001,

    SPA_AUDIO_FORMAT_START_Interleaved = 0x100,
    SPA_AUDIO_FORMAT_S8                = 0x101,
    SPA_AUDIO_FORMAT_U8                = 0x102,
    SPA_AUDIO_FORMAT_S16_LE            = 0x103,
    SPA_AUDIO_FORMAT_S16_BE            = 0x104,
    SPA_AUDIO_FORMAT_U16_LE            = 0x105,
    SPA_AUDIO_FORMAT_U16_BE            = 0x106,
    SPA_AUDIO_FORMAT_S24_32_LE         = 0x107,
    SPA_AUDIO_FORMAT_S24_32_BE         = 0x108,
    SPA_AUDIO_FORMAT_U24_32_LE         = 0x109,
    SPA_AUDIO_FORMAT_U24_32_BE         = 0x10A,
    SPA_AUDIO_FORMAT_S32_LE            = 0x10B,
    SPA_AUDIO_FORMAT_S32_BE            = 0x10C,
    SPA_AUDIO_FORMAT_U32_LE            = 0x10D,
    SPA_AUDIO_FORMAT_U32_BE            = 0x10E,
    SPA_AUDIO_FORMAT_S24_LE            = 0x10F,
    SPA_AUDIO_FORMAT_S24_BE            = 0x110,
    SPA_AUDIO_FORMAT_U24_LE            = 0x111,
    SPA_AUDIO_FORMAT_U24_BE            = 0x112,
    SPA_AUDIO_FORMAT_S20_LE            = 0x113,
    SPA_AUDIO_FORMAT_S20_BE            = 0x114,
    SPA_AUDIO_FORMAT_U20_LE            = 0x115,
    SPA_AUDIO_FORMAT_U20_BE            = 0x116,
    SPA_AUDIO_FORMAT_S18_LE            = 0x117,
    SPA_AUDIO_FORMAT_S18_BE            = 0x118,
    SPA_AUDIO_FORMAT_U18_LE            = 0x119,
    SPA_AUDIO_FORMAT_U18_BE            = 0x11A,
    SPA_AUDIO_FORMAT_F32_LE            = 0x11B,
    SPA_AUDIO_FORMAT_F32_BE            = 0x11C,
    SPA_AUDIO_FORMAT_F64_LE            = 0x11D,
    SPA_AUDIO_FORMAT_F64_BE            = 0x11E,

    SPA_AUDIO_FORMAT_ULAW              = 0x11F,
    SPA_AUDIO_FORMAT_ALAW              = 0x120,

    SPA_AUDIO_FORMAT_START_Planar      = 0x200,
    SPA_AUDIO_FORMAT_U8P               = 0x201,
    SPA_AUDIO_FORMAT_S16P              = 0x202,
    SPA_AUDIO_FORMAT_S24_32P           = 0x203,
    SPA_AUDIO_FORMAT_S32P              = 0x204,
    SPA_AUDIO_FORMAT_S24P              = 0x205,
    SPA_AUDIO_FORMAT_F32P              = 0x206,
    SPA_AUDIO_FORMAT_F64P              = 0x207,
    SPA_AUDIO_FORMAT_S8P               = 0x208,

    SPA_AUDIO_FORMAT_START_Other       = 0x400,
}

ty! {
    SpaVideoFormat;

    SPA_VIDEO_FORMAT_UNKNOWN    = 000,
    SPA_VIDEO_FORMAT_ENCODED    = 001,
    SPA_VIDEO_FORMAT_I420       = 002,
    SPA_VIDEO_FORMAT_YV12       = 003,
    SPA_VIDEO_FORMAT_YUY2       = 004,
    SPA_VIDEO_FORMAT_UYVY       = 005,
    SPA_VIDEO_FORMAT_AYUV       = 006,
    SPA_VIDEO_FORMAT_RGBx       = 007,
    SPA_VIDEO_FORMAT_BGRx       = 008,
    SPA_VIDEO_FORMAT_xRGB       = 009,
    SPA_VIDEO_FORMAT_xBGR       = 010,
    SPA_VIDEO_FORMAT_RGBA       = 011,
    SPA_VIDEO_FORMAT_BGRA       = 012,
    SPA_VIDEO_FORMAT_ARGB       = 013,
    SPA_VIDEO_FORMAT_ABGR       = 014,
    SPA_VIDEO_FORMAT_RGB        = 015,
    SPA_VIDEO_FORMAT_BGR        = 016,
    SPA_VIDEO_FORMAT_Y41B       = 017,
    SPA_VIDEO_FORMAT_Y42B       = 018,
    SPA_VIDEO_FORMAT_YVYU       = 019,
    SPA_VIDEO_FORMAT_Y444       = 020,
    SPA_VIDEO_FORMAT_v210       = 021,
    SPA_VIDEO_FORMAT_v216       = 022,
    SPA_VIDEO_FORMAT_NV12       = 023,
    SPA_VIDEO_FORMAT_NV21       = 024,
    SPA_VIDEO_FORMAT_GRAY8      = 025,
    SPA_VIDEO_FORMAT_GRAY16_BE  = 026,
    SPA_VIDEO_FORMAT_GRAY16_LE  = 027,
    SPA_VIDEO_FORMAT_v308       = 028,
    SPA_VIDEO_FORMAT_RGB16      = 029,
    SPA_VIDEO_FORMAT_BGR16      = 030,
    SPA_VIDEO_FORMAT_RGB15      = 031,
    SPA_VIDEO_FORMAT_BGR15      = 032,
    SPA_VIDEO_FORMAT_UYVP       = 033,
    SPA_VIDEO_FORMAT_A420       = 034,
    SPA_VIDEO_FORMAT_RGB8P      = 035,
    SPA_VIDEO_FORMAT_YUV9       = 036,
    SPA_VIDEO_FORMAT_YVU9       = 037,
    SPA_VIDEO_FORMAT_IYU1       = 038,
    SPA_VIDEO_FORMAT_ARGB64     = 039,
    SPA_VIDEO_FORMAT_AYUV64     = 040,
    SPA_VIDEO_FORMAT_r210       = 041,
    SPA_VIDEO_FORMAT_I420_10BE  = 042,
    SPA_VIDEO_FORMAT_I420_10LE  = 043,
    SPA_VIDEO_FORMAT_I422_10BE  = 044,
    SPA_VIDEO_FORMAT_I422_10LE  = 045,
    SPA_VIDEO_FORMAT_Y444_10BE  = 046,
    SPA_VIDEO_FORMAT_Y444_10LE  = 047,
    SPA_VIDEO_FORMAT_GBR        = 048,
    SPA_VIDEO_FORMAT_GBR_10BE   = 049,
    SPA_VIDEO_FORMAT_GBR_10LE   = 050,
    SPA_VIDEO_FORMAT_NV16       = 051,
    SPA_VIDEO_FORMAT_NV24       = 052,
    SPA_VIDEO_FORMAT_NV12_64Z32 = 053,
    SPA_VIDEO_FORMAT_A420_10BE  = 054,
    SPA_VIDEO_FORMAT_A420_10LE  = 055,
    SPA_VIDEO_FORMAT_A422_10BE  = 056,
    SPA_VIDEO_FORMAT_A422_10LE  = 057,
    SPA_VIDEO_FORMAT_A444_10BE  = 058,
    SPA_VIDEO_FORMAT_A444_10LE  = 059,
    SPA_VIDEO_FORMAT_NV61       = 060,
    SPA_VIDEO_FORMAT_P010_10BE  = 061,
    SPA_VIDEO_FORMAT_P010_10LE  = 062,
    SPA_VIDEO_FORMAT_IYU2       = 063,
    SPA_VIDEO_FORMAT_VYUY       = 064,
    SPA_VIDEO_FORMAT_GBRA       = 065,
    SPA_VIDEO_FORMAT_GBRA_10BE  = 066,
    SPA_VIDEO_FORMAT_GBRA_10LE  = 067,
    SPA_VIDEO_FORMAT_GBR_12BE   = 068,
    SPA_VIDEO_FORMAT_GBR_12LE   = 069,
    SPA_VIDEO_FORMAT_GBRA_12BE  = 070,
    SPA_VIDEO_FORMAT_GBRA_12LE  = 071,
    SPA_VIDEO_FORMAT_I420_12BE  = 072,
    SPA_VIDEO_FORMAT_I420_12LE  = 073,
    SPA_VIDEO_FORMAT_I422_12BE  = 074,
    SPA_VIDEO_FORMAT_I422_12LE  = 075,
    SPA_VIDEO_FORMAT_Y444_12BE  = 076,
    SPA_VIDEO_FORMAT_Y444_12LE  = 077,
    SPA_VIDEO_FORMAT_RGBA_F16   = 078,
    SPA_VIDEO_FORMAT_RGBA_F32   = 079,
    SPA_VIDEO_FORMAT_xRGB_210LE = 080,
    SPA_VIDEO_FORMAT_xBGR_210LE = 081,
    SPA_VIDEO_FORMAT_RGBx_102LE = 082,
    SPA_VIDEO_FORMAT_BGRx_102LE = 083,
    SPA_VIDEO_FORMAT_ARGB_210LE = 084,
    SPA_VIDEO_FORMAT_ABGR_210LE = 085,
    SPA_VIDEO_FORMAT_RGBA_102LE = 086,
    SPA_VIDEO_FORMAT_BGRA_102LE = 087,
}

ty! {
    SpaVideoInterlaceMode;

    SPA_VIDEO_INTERLACE_MODE_PROGRESSIVE = 0,
    SPA_VIDEO_INTERLACE_MODE_INTERLEAVED = 1,
    SPA_VIDEO_INTERLACE_MODE_MIXED = 2,
    SPA_VIDEO_INTERLACE_MODE_FIELDS = 3,
}

ty! {
    SpaVideoMultiviewMode;

    SPA_VIDEO_MULTIVIEW_MODE_NONE = !0,
    SPA_VIDEO_MULTIVIEW_MODE_MONO = 0,

    SPA_VIDEO_MULTIVIEW_MODE_LEFT = 1,
    SPA_VIDEO_MULTIVIEW_MODE_RIGHT = 2,

    SPA_VIDEO_MULTIVIEW_MODE_SIDE_BY_SIDE = 3,
    SPA_VIDEO_MULTIVIEW_MODE_SIDE_BY_SIDE_QUINCUNX = 4,
    SPA_VIDEO_MULTIVIEW_MODE_COLUMN_INTERLEAVED = 5,
    SPA_VIDEO_MULTIVIEW_MODE_ROW_INTERLEAVED = 6,
    SPA_VIDEO_MULTIVIEW_MODE_TOP_BOTTOM = 7,
    SPA_VIDEO_MULTIVIEW_MODE_CHECKERBOARD = 8,

    SPA_VIDEO_MULTIVIEW_MODE_FRAME_BY_FRAME = 32,
    SPA_VIDEO_MULTIVIEW_MODE_MULTIVIEW_FRAME_BY_FRAME = 33,
    SPA_VIDEO_MULTIVIEW_MODE_SEPARATED = 34,
}

bitflags! {
    SpaVideoMultiviewFlags: u32;

    SPA_VIDEO_MULTIVIEW_FLAGS_NONE             = 0,
    SPA_VIDEO_MULTIVIEW_FLAGS_RIGHT_VIEW_FIRST = 1 << 0,
    SPA_VIDEO_MULTIVIEW_FLAGS_LEFT_FLIPPED     = 1 << 1,
    SPA_VIDEO_MULTIVIEW_FLAGS_LEFT_FLOPPED     = 1 << 2,
    SPA_VIDEO_MULTIVIEW_FLAGS_RIGHT_FLIPPED    = 1 << 3,
    SPA_VIDEO_MULTIVIEW_FLAGS_RIGHT_FLOPPED    = 1 << 4,
    SPA_VIDEO_MULTIVIEW_FLAGS_HALF_ASPECT      = 1 << 14,
    SPA_VIDEO_MULTIVIEW_FLAGS_MIXED_MONO       = 1 << 15,
}

bitflags! {
    SpaVideoChromaSite: u32;

    SPA_VIDEO_CHROMA_SITE_UNKNOWN   = 0,
    SPA_VIDEO_CHROMA_SITE_NONE      = 1 << 0,
    SPA_VIDEO_CHROMA_SITE_H_COSITED = 1 << 1,
    SPA_VIDEO_CHROMA_SITE_V_COSITED = 1 << 2,
    SPA_VIDEO_CHROMA_SITE_ALT_LINE  = 1 << 3,
}

ty! {
    SpaVideoColorRange;

    SPA_VIDEO_COLOR_RANGE_UNKNOWN = 0,
    SPA_VIDEO_COLOR_RANGE_0_255 = 1,
    SPA_VIDEO_COLOR_RANGE_16_235 = 2,
}

ty! {
    SpaVideoColorMatrix;

    SPA_VIDEO_COLOR_MATRIX_UNKNOWN   = 0,
    SPA_VIDEO_COLOR_MATRIX_RGB       = 1,
    SPA_VIDEO_COLOR_MATRIX_FCC       = 2,
    SPA_VIDEO_COLOR_MATRIX_BT709     = 3,
    SPA_VIDEO_COLOR_MATRIX_BT601     = 4,
    SPA_VIDEO_COLOR_MATRIX_SMPTE240M = 5,
    SPA_VIDEO_COLOR_MATRIX_BT2020    = 6,
}

ty! {
    SpaVideoTransferFunction;

    SPA_VIDEO_TRANSFER_UNKNOWN   = 0,
    SPA_VIDEO_TRANSFER_GAMMA10   = 1,
    SPA_VIDEO_TRANSFER_GAMMA18   = 2,
    SPA_VIDEO_TRANSFER_GAMMA20   = 3,
    SPA_VIDEO_TRANSFER_GAMMA22   = 4,
    SPA_VIDEO_TRANSFER_BT709     = 5,
    SPA_VIDEO_TRANSFER_SMPTE240M = 6,
    SPA_VIDEO_TRANSFER_SRGB      = 7,
    SPA_VIDEO_TRANSFER_GAMMA28   = 8,
    SPA_VIDEO_TRANSFER_LOG100    = 9,
    SPA_VIDEO_TRANSFER_LOG316    = 10,
    SPA_VIDEO_TRANSFER_BT2020_12 = 11,
    SPA_VIDEO_TRANSFER_ADOBERGB  = 12,
}

ty! {
    SpaVideoColorPrimaries;

    SPA_VIDEO_COLOR_PRIMARIES_UNKNOWN = 0,
    SPA_VIDEO_COLOR_PRIMARIES_BT709 = 1,
    SPA_VIDEO_COLOR_PRIMARIES_BT470M = 2,
    SPA_VIDEO_COLOR_PRIMARIES_BT470BG = 3,
    SPA_VIDEO_COLOR_PRIMARIES_SMPTE170M = 4,
    SPA_VIDEO_COLOR_PRIMARIES_SMPTE240M = 5,
    SPA_VIDEO_COLOR_PRIMARIES_FILM = 6,
    SPA_VIDEO_COLOR_PRIMARIES_BT2020 = 7,
    SPA_VIDEO_COLOR_PRIMARIES_ADOBERGB = 8,
}

ty! {
    SpaH264StreamFormat;

    SPA_H264_STREAM_FORMAT_UNKNOWN = 0,
    SPA_H264_STREAM_FORMAT_AVC = 1,
    SPA_H264_STREAM_FORMAT_AVC3 = 2,
    SPA_H264_STREAM_FORMAT_BYTESTREAM = 3,
}

ty! {
    SpaH264Alignment;

    SPA_H264_ALIGNMENT_UNKNOWN = 0,
    SPA_H264_ALIGNMENT_AU = 1,
    SPA_H264_ALIGNMENT_NAL = 2,
}

ty! {
    SpaParamBuffers;

    SPA_PARAM_BUFFERS_START    = 0,
    SPA_PARAM_BUFFERS_buffers  = 1,
    SPA_PARAM_BUFFERS_blocks   = 2,
    SPA_PARAM_BUFFERS_size     = 3,
    SPA_PARAM_BUFFERS_stride   = 4,
    SPA_PARAM_BUFFERS_align    = 5,
    SPA_PARAM_BUFFERS_dataType = 6,
}

ty! {
    SpaDataType;

    SPA_DATA_Invalid = 0,
    SPA_DATA_MemPtr = 1,
    SPA_DATA_MemFd = 2,
    SPA_DATA_DmaBuf = 3,
    SPA_DATA_MemId = 4,
}

impl Default for SpaDataType {
    fn default() -> Self {
        SPA_DATA_Invalid
    }
}

bitflags! {
    SpaNodeBuffersFlags: u32;

    SPA_NODE_BUFFERS_FLAG_ALLOC = 1 << 0,
}

bitflags! {
    SpaDataFlags: u32;

    SPA_DATA_FLAG_READABLE = 1 << 0,
    SPA_DATA_FLAG_WRITABLE = 1 << 1,
    SPA_DATA_FLAG_DYNAMIC = 1 << 2,
}

bitflags! {
    SpaDataTypes: u32;

    SPA_DATA_MASK_Invalid = 1,
    SPA_DATA_MASK_MemPtr = 2,
    SPA_DATA_MASK_MemFd = 4,
    SPA_DATA_MASK_DmaBuf = 8,
    SPA_DATA_MASK_MemId = 16,
}

ty! {
    SpaParamMeta;

    SPA_PARAM_META_START = 0,
    SPA_PARAM_META_type = 1,
    SPA_PARAM_META_size = 2,
}

ty! {
    SpaParamIo;

    SPA_PARAM_IO_START = 0,
    SPA_PARAM_IO_id = 1,
    SPA_PARAM_IO_size = 2,
}

ty! {
    SpaIoType;

    SPA_IO_Invalid = 0,
    SPA_IO_Buffers = 1,
    SPA_IO_Range = 2,
    SPA_IO_Clock = 3,
    SPA_IO_Latency = 4,
    SPA_IO_Control = 5,
    SPA_IO_Notify = 6,
    SPA_IO_Position = 7,
    SPA_IO_RateMatch = 8,
    SPA_IO_Memory = 9,
}

ty! {
    SpaParamProfile;

    SPA_PARAM_PROFILE_START = 0,
    SPA_PARAM_PROFILE_index = 1,
    SPA_PARAM_PROFILE_name = 2,
    SPA_PARAM_PROFILE_description = 3,
    SPA_PARAM_PROFILE_priority = 4,
    SPA_PARAM_PROFILE_available = 5,
    SPA_PARAM_PROFILE_info = 6,
    SPA_PARAM_PROFILE_classes = 7,
    SPA_PARAM_PROFILE_save = 8,
}

ty! {
    SpaParamAvailability;

    SPA_PARAM_AVAILABILITY_unknown = 0,
    SPA_PARAM_AVAILABILITY_no = 1,
    SPA_PARAM_AVAILABILITY_yes = 2,
}

ty! {
    SpaParamPortConfig;

    SPA_PARAM_PORT_CONFIG_START     = 0,
    SPA_PARAM_PORT_CONFIG_direction = 1,
    SPA_PARAM_PORT_CONFIG_mode      = 2,
    SPA_PARAM_PORT_CONFIG_monitor   = 3,
    SPA_PARAM_PORT_CONFIG_control   = 4,
    SPA_PARAM_PORT_CONFIG_format    = 5,
}

ty! {
    SpaDirection;

    SPA_DIRECTION_INPUT = 0,
    SPA_DIRECTION_OUTPUT = 1,
}

ty! {
    SpaParamRoute;

    SPA_PARAM_ROUTE_START       = 0,
    SPA_PARAM_ROUTE_index       = 1,
    SPA_PARAM_ROUTE_direction   = 2,
    SPA_PARAM_ROUTE_device      = 3,
    SPA_PARAM_ROUTE_name        = 4,
    SPA_PARAM_ROUTE_description = 5,
    SPA_PARAM_ROUTE_priority    = 6,
    SPA_PARAM_ROUTE_available   = 7,
    SPA_PARAM_ROUTE_info        = 8,
    SPA_PARAM_ROUTE_profiles    = 9,
    SPA_PARAM_ROUTE_props       = 10,
    SPA_PARAM_ROUTE_devices     = 11,
    SPA_PARAM_ROUTE_profile     = 12,
    SPA_PARAM_ROUTE_save        = 13,
}

ty! {
    SpaProfiler;

    SPA_PROFILER_START          = 0x0000000,

    SPA_PROFILER_START_Driver   = 0x0010000,
    SPA_PROFILER_info           = 0x0010001,
    SPA_PROFILER_clock          = 0x0010002,
    SPA_PROFILER_driverBlock    = 0x0010003,
    SPA_PROFILER_START_Follower = 0x0020000,
    SPA_PROFILER_followerBlock  = 0x0020001,
    SPA_PROFILER_START_CUSTOM   = 0x1000000,
}

ty! {
    SpaParamLatency;

    SPA_PARAM_LATENCY_START      = 0,
    SPA_PARAM_LATENCY_direction  = 1,
    SPA_PARAM_LATENCY_minQuantum = 2,
    SPA_PARAM_LATENCY_maxQuantum = 3,
    SPA_PARAM_LATENCY_minRate    = 4,
    SPA_PARAM_LATENCY_maxRate    = 5,
    SPA_PARAM_LATENCY_minNs      = 6,
    SPA_PARAM_LATENCY_maxNs      = 7,
}

ty! {
    SpaParamProcessLatency;

    SPA_PARAM_PROCESS_LATENCY_START   = 0,
    SPA_PARAM_PROCESS_LATENCY_quantum = 1,
    SPA_PARAM_PROCESS_LATENCY_rate    = 2,
    SPA_PARAM_PROCESS_LATENCY_ns      = 3,
}

ty! {
    SpaParamPortConfigMode;

    SPA_PARAM_PORT_CONFIG_MODE_none        = 0,
    SPA_PARAM_PORT_CONFIG_MODE_passthrough = 1,
    SPA_PARAM_PORT_CONFIG_MODE_convert     = 2,
    SPA_PARAM_PORT_CONFIG_MODE_dsp         = 3,
}

bitflags! {
    SpaMetaHeaderFlags: u32;

    SPA_META_HEADER_FLAG_DISCONT    = 1 << 0,
    SPA_META_HEADER_FLAG_CORRUPTED  = 1 << 1,
    SPA_META_HEADER_FLAG_MARKER     = 1 << 2,
    SPA_META_HEADER_FLAG_HEADER     = 1 << 3,
    SPA_META_HEADER_FLAG_GAP        = 1 << 4,
    SPA_META_HEADER_FLAG_DELTA_UNIT = 1 << 5,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct spa_meta_header {
    pub flags: SpaMetaHeaderFlags,
    pub offset: u32,
    pub pts: i64,
    pub dts_offset: i64,
    pub seq: u64,
}

unsafe impl Pod for spa_meta_header {}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct spa_point {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct spa_region {
    pub position: spa_point,
    pub size: spa_rectangle,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct spa_meta_region {
    pub region: spa_region,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct spa_meta_bitmap {
    pub format: SpaVideoFormat,
    pub size: spa_rectangle,
    pub stride: i32,
    pub offset: u32,
}

unsafe impl Pod for spa_meta_bitmap {}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct spa_meta_cursor {
    pub id: u32,
    pub flags: u32,
    pub position: spa_point,
    pub hotspot: spa_point,
    pub bitmap_offset: u32,
}

unsafe impl Pod for spa_meta_cursor {}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct spa_meta_busy {
    pub flags: u32,
    pub count: u32,
}

unsafe impl Pod for spa_meta_busy {}

unsafe impl Pod for spa_meta_region {}

ty! {
    SpaMetaType;

    SPA_META_Invalid = 0,
    SPA_META_Header = 1,
    SPA_META_VideoCrop = 2,
    SPA_META_VideoDamage = 3,
    SPA_META_Bitmap = 4,
    SPA_META_Cursor = 5,
    SPA_META_Control = 6,
    SPA_META_Busy = 7,
}

ty! {
    SpaPropInfo;

    SPA_PROP_INFO_START = 0,
    SPA_PROP_INFO_id = 1,
    SPA_PROP_INFO_name = 2,
    SPA_PROP_INFO_type = 3,
    SPA_PROP_INFO_labels = 4,
    SPA_PROP_INFO_container = 5,
    SPA_PROP_INFO_params = 6,
    SPA_PROP_INFO_description = 7,
}

ty! {
    SpaProp;

    SPA_PROP_START               = 0x0000000,
    SPA_PROP_unknown             = 0x0000001,
    SPA_PROP_START_Device        = 0x0000100,
    SPA_PROP_device              = 0x0000101,
    SPA_PROP_deviceName          = 0x0000102,
    SPA_PROP_deviceFd            = 0x0000103,
    SPA_PROP_card                = 0x0000104,
    SPA_PROP_cardName            = 0x0000105,
    SPA_PROP_minLatency          = 0x0000106,
    SPA_PROP_maxLatency          = 0x0000107,
    SPA_PROP_periods             = 0x0000108,
    SPA_PROP_periodSize          = 0x0000109,
    SPA_PROP_periodEvent         = 0x000010A,
    SPA_PROP_live                = 0x000010B,
    SPA_PROP_rate                = 0x000010C,
    SPA_PROP_quality             = 0x000010D,
    SPA_PROP_bluetoothAudioCodec = 0x000010E,
    SPA_PROP_START_Audio         = 0x0010000,
    SPA_PROP_waveType            = 0x0010001,
    SPA_PROP_frequency           = 0x0010002,
    SPA_PROP_volume              = 0x0010003,
    SPA_PROP_mute                = 0x0010004,
    SPA_PROP_patternType         = 0x0010005,
    SPA_PROP_ditherType          = 0x0010006,
    SPA_PROP_truncate            = 0x0010007,
    SPA_PROP_channelVolumes      = 0x0010008,
    SPA_PROP_volumeBase          = 0x0010009,
    SPA_PROP_volumeStep          = 0x001000A,
    SPA_PROP_channelMap          = 0x001000B,
    SPA_PROP_monitorMute         = 0x001000C,
    SPA_PROP_monitorVolumes      = 0x001000D,
    SPA_PROP_latencyOffsetNsec   = 0x001000E,
    SPA_PROP_softMute            = 0x001000F,
    SPA_PROP_softVolumes         = 0x0010010,
    SPA_PROP_iec958Codecs        = 0x0010011,
    SPA_PROP_START_Video         = 0x0020000,
    SPA_PROP_brightness          = 0x0020001,
    SPA_PROP_contrast            = 0x0020002,
    SPA_PROP_saturation          = 0x0020003,
    SPA_PROP_hue                 = 0x0020004,
    SPA_PROP_gamma               = 0x0020005,
    SPA_PROP_exposure            = 0x0020006,
    SPA_PROP_gain                = 0x0020007,
    SPA_PROP_sharpness           = 0x0020008,
    SPA_PROP_START_Other         = 0x0080000,
    SPA_PROP_params              = 0x0080001,
    SPA_PROP_START_CUSTOM        = 0x1000000,
}

ty! {
    SpaAudioChannel;

    SPA_AUDIO_CHANNEL_UNKNOWN      = 0x00000,
    SPA_AUDIO_CHANNEL_NA           = 0x00001,

    SPA_AUDIO_CHANNEL_MONO         = 0x00002,

    SPA_AUDIO_CHANNEL_FL           = 0x00003,
    SPA_AUDIO_CHANNEL_FR           = 0x00004,
    SPA_AUDIO_CHANNEL_FC           = 0x00005,
    SPA_AUDIO_CHANNEL_LFE          = 0x00006,
    SPA_AUDIO_CHANNEL_SL           = 0x00007,
    SPA_AUDIO_CHANNEL_SR           = 0x00008,
    SPA_AUDIO_CHANNEL_FLC          = 0x00009,
    SPA_AUDIO_CHANNEL_FRC          = 0x0000A,
    SPA_AUDIO_CHANNEL_RC           = 0x0000B,
    SPA_AUDIO_CHANNEL_RL           = 0x0000C,
    SPA_AUDIO_CHANNEL_RR           = 0x0000D,
    SPA_AUDIO_CHANNEL_TC           = 0x0000E,
    SPA_AUDIO_CHANNEL_TFL          = 0x0000F,
    SPA_AUDIO_CHANNEL_TFC          = 0x00010,
    SPA_AUDIO_CHANNEL_TFR          = 0x00011,
    SPA_AUDIO_CHANNEL_TRL          = 0x00012,
    SPA_AUDIO_CHANNEL_TRC          = 0x00013,
    SPA_AUDIO_CHANNEL_TRR          = 0x00014,
    SPA_AUDIO_CHANNEL_RLC          = 0x00015,
    SPA_AUDIO_CHANNEL_RRC          = 0x00016,
    SPA_AUDIO_CHANNEL_FLW          = 0x00017,
    SPA_AUDIO_CHANNEL_FRW          = 0x00018,
    SPA_AUDIO_CHANNEL_LFE2         = 0x00019,
    SPA_AUDIO_CHANNEL_FLH          = 0x0001A,
    SPA_AUDIO_CHANNEL_FCH          = 0x0001B,
    SPA_AUDIO_CHANNEL_FRH          = 0x0001C,
    SPA_AUDIO_CHANNEL_TFLC         = 0x0001D,
    SPA_AUDIO_CHANNEL_TFRC         = 0x0001E,
    SPA_AUDIO_CHANNEL_TSL          = 0x0001F,
    SPA_AUDIO_CHANNEL_TSR          = 0x00020,
    SPA_AUDIO_CHANNEL_LLFE         = 0x00021,
    SPA_AUDIO_CHANNEL_RLFE         = 0x00022,
    SPA_AUDIO_CHANNEL_BC           = 0x00023,
    SPA_AUDIO_CHANNEL_BLC          = 0x00024,
    SPA_AUDIO_CHANNEL_BRC          = 0x00025,

    SPA_AUDIO_CHANNEL_AUX0         = 0x01000,
    SPA_AUDIO_CHANNEL_AUX1         = 0x01001,
    SPA_AUDIO_CHANNEL_AUX2         = 0x01002,
    SPA_AUDIO_CHANNEL_AUX3         = 0x01003,
    SPA_AUDIO_CHANNEL_AUX4         = 0x01004,
    SPA_AUDIO_CHANNEL_AUX5         = 0x01005,
    SPA_AUDIO_CHANNEL_AUX6         = 0x01006,
    SPA_AUDIO_CHANNEL_AUX7         = 0x01007,
    SPA_AUDIO_CHANNEL_AUX8         = 0x01008,
    SPA_AUDIO_CHANNEL_AUX9         = 0x01009,
    SPA_AUDIO_CHANNEL_AUX10        = 0x0100A,
    SPA_AUDIO_CHANNEL_AUX11        = 0x0100B,
    SPA_AUDIO_CHANNEL_AUX12        = 0x0100C,
    SPA_AUDIO_CHANNEL_AUX13        = 0x0100D,
    SPA_AUDIO_CHANNEL_AUX14        = 0x0100E,
    SPA_AUDIO_CHANNEL_AUX15        = 0x0100F,
    SPA_AUDIO_CHANNEL_AUX16        = 0x01010,
    SPA_AUDIO_CHANNEL_AUX17        = 0x01011,
    SPA_AUDIO_CHANNEL_AUX18        = 0x01012,
    SPA_AUDIO_CHANNEL_AUX19        = 0x01013,
    SPA_AUDIO_CHANNEL_AUX20        = 0x01014,
    SPA_AUDIO_CHANNEL_AUX21        = 0x01015,
    SPA_AUDIO_CHANNEL_AUX22        = 0x01016,
    SPA_AUDIO_CHANNEL_AUX23        = 0x01017,
    SPA_AUDIO_CHANNEL_AUX24        = 0x01018,
    SPA_AUDIO_CHANNEL_AUX25        = 0x01019,
    SPA_AUDIO_CHANNEL_AUX26        = 0x0101A,
    SPA_AUDIO_CHANNEL_AUX27        = 0x0101B,
    SPA_AUDIO_CHANNEL_AUX28        = 0x0101C,
    SPA_AUDIO_CHANNEL_AUX29        = 0x0101D,
    SPA_AUDIO_CHANNEL_AUX30        = 0x0101E,
    SPA_AUDIO_CHANNEL_AUX31        = 0x0101F,
    SPA_AUDIO_CHANNEL_AUX32        = 0x01020,
    SPA_AUDIO_CHANNEL_AUX33        = 0x01021,
    SPA_AUDIO_CHANNEL_AUX34        = 0x01022,
    SPA_AUDIO_CHANNEL_AUX35        = 0x01023,
    SPA_AUDIO_CHANNEL_AUX36        = 0x01024,
    SPA_AUDIO_CHANNEL_AUX37        = 0x01025,
    SPA_AUDIO_CHANNEL_AUX38        = 0x01026,
    SPA_AUDIO_CHANNEL_AUX39        = 0x01027,
    SPA_AUDIO_CHANNEL_AUX40        = 0x01028,
    SPA_AUDIO_CHANNEL_AUX41        = 0x01029,
    SPA_AUDIO_CHANNEL_AUX42        = 0x0102A,
    SPA_AUDIO_CHANNEL_AUX43        = 0x0102B,
    SPA_AUDIO_CHANNEL_AUX44        = 0x0102C,
    SPA_AUDIO_CHANNEL_AUX45        = 0x0102D,
    SPA_AUDIO_CHANNEL_AUX46        = 0x0102E,
    SPA_AUDIO_CHANNEL_AUX47        = 0x0102F,
    SPA_AUDIO_CHANNEL_AUX48        = 0x01030,
    SPA_AUDIO_CHANNEL_AUX49        = 0x01031,
    SPA_AUDIO_CHANNEL_AUX50        = 0x01032,
    SPA_AUDIO_CHANNEL_AUX51        = 0x01033,
    SPA_AUDIO_CHANNEL_AUX52        = 0x01034,
    SPA_AUDIO_CHANNEL_AUX53        = 0x01035,
    SPA_AUDIO_CHANNEL_AUX54        = 0x01036,
    SPA_AUDIO_CHANNEL_AUX55        = 0x01037,
    SPA_AUDIO_CHANNEL_AUX56        = 0x01038,
    SPA_AUDIO_CHANNEL_AUX57        = 0x01039,
    SPA_AUDIO_CHANNEL_AUX58        = 0x0103A,
    SPA_AUDIO_CHANNEL_AUX59        = 0x0103B,
    SPA_AUDIO_CHANNEL_AUX60        = 0x0103C,
    SPA_AUDIO_CHANNEL_AUX61        = 0x0103D,
    SPA_AUDIO_CHANNEL_AUX62        = 0x0103E,
    SPA_AUDIO_CHANNEL_AUX63        = 0x0103F,

    SPA_AUDIO_CHANNEL_LAST_Aux     = 0x01fff,

    SPA_AUDIO_CHANNEL_START_Custom = 0x10000,
}

ty! {
    SpaAudioIec958Codec;

    SPA_AUDIO_IEC958_CODEC_UNKNOWN   = 0,

    SPA_AUDIO_IEC958_CODEC_PCM       = 1,
    SPA_AUDIO_IEC958_CODEC_DTS       = 2,
    SPA_AUDIO_IEC958_CODEC_AC3       = 3,
    SPA_AUDIO_IEC958_CODEC_MPEG      = 4,
    SPA_AUDIO_IEC958_CODEC_MPEG2_AAC = 5,

    SPA_AUDIO_IEC958_CODEC_EAC3      = 6,

    SPA_AUDIO_IEC958_CODEC_TRUEHD    = 7,
    SPA_AUDIO_IEC958_CODEC_DTSHD     = 8,
}

ty! {
    SpaParamBitorder;

    SPA_PARAM_BITORDER_unknown = 0,
    SPA_PARAM_BITORDER_msb = 1,
    SPA_PARAM_BITORDER_lsb = 2,
}
ty! {
    SpaNodeCommand;

    SPA_NODE_COMMAND_Suspend        = 0,
    SPA_NODE_COMMAND_Pause          = 1,
    SPA_NODE_COMMAND_Start          = 2,
    SPA_NODE_COMMAND_Enable         = 3,
    SPA_NODE_COMMAND_Disable        = 4,
    SPA_NODE_COMMAND_Flush          = 5,
    SPA_NODE_COMMAND_Drain          = 6,
    SPA_NODE_COMMAND_Marker         = 7,
    SPA_NODE_COMMAND_ParamBegin     = 8,
    SPA_NODE_COMMAND_ParamEnd       = 9,
    SPA_NODE_COMMAND_RequestProcess = 10,
}

#[derive(Copy, Clone)]
pub enum PwPod<'a> {
    None,
    Bool(bool),
    Id(u32),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(&'a BStr),
    Bytes(&'a [u8]),
    Rectangle(PwPodRectangle),
    Fraction(PwPodFraction),
    Bitmap(&'a [u8]),
    Array(PwPodArray<'a>),
    Struct(PwPodStruct<'a>),
    Object(PwPodObject<'a>),
    Sequence(PwPodSequence<'a>),
    Pointer(PwPodPointer),
    Fd(u64),
    Choice(PwPodChoice<'a>),
}

#[derive(Copy, Clone, Debug)]
pub struct PwPodRectangle {
    pub width: u32,
    pub height: u32,
}

#[derive(Copy, Clone, Debug)]
pub struct PwPodFraction {
    pub num: u32,
    pub denom: u32,
}

#[derive(Copy, Clone)]
pub struct PwPodArray<'a> {
    pub ty: PwPodType,
    pub child_len: usize,
    pub n_elements: usize,
    pub elements: PwParser<'a>,
}

#[derive(Copy, Clone)]
pub struct PwPodStruct<'a> {
    pub fields: PwParser<'a>,
}

#[derive(Copy, Clone)]
pub struct PwPodObject<'a> {
    pub ty: PwPodObjectType,
    pub id: u32,
    pub probs: PwParser<'a>,
}

impl<'a> PwPodObject<'a> {
    pub fn get_param(&mut self, key: u32) -> Result<Option<PwProp<'_>>, PwParserError> {
        let start = self.probs.pos();
        loop {
            if self.probs.len() == 0 {
                self.probs.reset();
            } else {
                let prob = self.probs.read_prop()?;
                if prob.key == key {
                    return Ok(Some(prob));
                }
            }
            if self.probs.pos() == start {
                return Ok(None);
            }
        }
    }
}

#[derive(Copy, Clone)]
pub struct PwPodSequence<'a> {
    pub unit: u32,
    pub controls: PwParser<'a>,
}

#[derive(Copy, Clone, Debug)]
pub struct PwPodControl<'a> {
    pub _offset: u32,
    pub _ty: PwControlType,
    pub _value: PwPod<'a>,
}

#[derive(Copy, Clone, Debug)]
pub struct PwPodPointer {
    pub _ty: PwPointerType,
    pub _value: usize,
}

#[derive(Copy, Clone, Debug)]
pub struct PwPodChoice<'a> {
    pub ty: PwChoiceType,
    pub flags: u32,
    pub elements: PwPodArray<'a>,
}

#[derive(Copy, Clone, Debug)]
pub struct PwProp<'a> {
    pub key: u32,
    pub flags: PwPropFlag,
    pub pod: PwPod<'a>,
}

impl<'a> PwPod<'a> {
    pub fn ty(&self) -> PwPodType {
        match self {
            PwPod::None => PW_TYPE_None,
            PwPod::Bool(_) => PW_TYPE_Bool,
            PwPod::Id(_) => PW_TYPE_Id,
            PwPod::Int(_) => PW_TYPE_Int,
            PwPod::Long(_) => PW_TYPE_Long,
            PwPod::Float(_) => PW_TYPE_Float,
            PwPod::Double(_) => PW_TYPE_Double,
            PwPod::String(_) => PW_TYPE_String,
            PwPod::Bytes(_) => PW_TYPE_Bytes,
            PwPod::Rectangle(_) => PW_TYPE_Rectangle,
            PwPod::Fraction(_) => PW_TYPE_Fraction,
            PwPod::Bitmap(_) => PW_TYPE_Bitmap,
            PwPod::Array(_) => PW_TYPE_Array,
            PwPod::Struct(_) => PW_TYPE_Struct,
            PwPod::Object(_) => PW_TYPE_Object,
            PwPod::Sequence(_) => PW_TYPE_Sequence,
            PwPod::Pointer(_) => PW_TYPE_Pointer,
            PwPod::Fd(_) => PW_TYPE_Fd,
            PwPod::Choice(_) => PW_TYPE_Choice,
        }
    }

    pub fn get_fraction(&self) -> Result<PwPodFraction, PwParserError> {
        match self.get_value()? {
            PwPod::Fraction(i) => Ok(i),
            _ => Err(PwParserError::UnexpectedPodType(
                PW_TYPE_Fraction,
                self.ty(),
            )),
        }
    }

    pub fn get_rectangle(&self) -> Result<PwPodRectangle, PwParserError> {
        match self.get_value()? {
            PwPod::Rectangle(i) => Ok(i),
            _ => Err(PwParserError::UnexpectedPodType(
                PW_TYPE_Rectangle,
                self.ty(),
            )),
        }
    }

    pub fn get_id(&self) -> Result<u32, PwParserError> {
        match self.get_value()? {
            PwPod::Id(i) => Ok(i),
            _ => Err(PwParserError::UnexpectedPodType(PW_TYPE_Id, self.ty())),
        }
    }

    pub fn get_value(mut self) -> Result<PwPod<'a>, PwParserError> {
        if let PwPod::Choice(v) = &mut self {
            if v.ty == PW_CHOICE_None && v.elements.n_elements > 0 {
                return v
                    .elements
                    .elements
                    .read_pod_body_packed(v.elements.ty, v.elements.child_len);
            }
        }
        Ok(self)
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct spa_fraction {
    pub num: u32,
    pub denom: u32,
}

bitflags! {
    SPA_IO_SEGMENT_VIDEO_FLAG: u32;

    SPA_IO_SEGMENT_VIDEO_FLAG_VALID      = 1<<0,
    SPA_IO_SEGMENT_VIDEO_FLAG_DROP_FRAME = 1<<1,
    SPA_IO_SEGMENT_VIDEO_FLAG_PULL_DOWN  = 1<<2,
    SPA_IO_SEGMENT_VIDEO_FLAG_INTERLACED = 1<<3,
}

#[repr(C)]
#[derive(Debug)]
pub struct spa_io_segment_video {
    pub flags: SPA_IO_SEGMENT_VIDEO_FLAG,
    pub offset: u32,
    pub framerate: spa_fraction,
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub frames: u32,
    pub field_count: u32,
    pub padding: [u32; 11],
}

bitflags! {
    SPA_IO_SEGMENT_BAR_FLAG: u32;

    SPA_IO_SEGMENT_BAR_FLAG_VALID = 1<<0,
}

#[repr(C)]
#[derive(Debug)]
pub struct spa_io_segment_bar {
    pub flags: SPA_IO_SEGMENT_BAR_FLAG,
    pub offset: u32,
    pub signature_num: f32,
    pub signature_denom: f32,
    pub bpm: f64,
    pub beat: f64,
    pub padding: [u32; 8],
}

bitflags! {
    SPA_IO_SEGMENT_FLAG: u32;

    SPA_IO_SEGMENT_FLAG_LOOPING     = 1<<0,
    SPA_IO_SEGMENT_FLAG_NO_POSITION = 1<<1,
}

#[repr(C)]
#[derive(Debug)]
pub struct spa_io_segment {
    pub version: u32,
    pub flags: SPA_IO_SEGMENT_FLAG,
    pub start: u64,
    pub duration: u64,
    pub rate: f64,
    pub position: u64,
    pub bar: spa_io_segment_bar,
    pub video: spa_io_segment_video,
}

bitflags! {
    SPA_IO_CLOCK_FLAG: u32;

    SPA_IO_CLOCK_FLAG_FREEWHEEL = 1<<0,
}

#[repr(C)]
#[derive(Debug)]
pub struct spa_io_clock {
    pub flags: SPA_IO_CLOCK_FLAG,
    pub id: u32,
    pub name: [u8; 64],
    pub nsec: u64,
    pub rate: spa_fraction,
    pub position: u64,
    pub duration: u64,
    pub delay: i64,
    pub rate_diff: f64,
    pub next_nsec: u64,
    pub padding: [u32; 8],
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct spa_rectangle {
    pub width: u32,
    pub height: u32,
}

bitflags! {
    SPA_IO_VIDEO_SIZE: u32;

    SPA_IO_VIDEO_SIZE_VALID = 1<<0,
}

#[repr(C)]
#[derive(Debug)]
pub struct spa_io_video_size {
    pub flags: SPA_IO_VIDEO_SIZE,
    pub stride: u32,
    pub size: spa_rectangle,
    pub framerate: spa_fraction,
    pub padding: [u32; 4],
}

pub const SPA_IO_POSITION_MAX_SEGMENTS: usize = 8;

#[repr(C)]
#[derive(Debug)]
pub struct spa_io_position {
    pub clock: spa_io_clock,
    pub video: spa_io_video_size,
    pub offset: i64,
    pub state: u32,
    pub n_segments: u32,
    pub segments: [spa_io_segment; SPA_IO_POSITION_MAX_SEGMENTS],
}

#[repr(C)]
#[derive(Debug)]
pub struct pw_node_activation_state {
    pub status: c::c_int,
    pub required: i32,
    pub pending: i32,
}

ty! {
    PW_NODE_ACTIVATION;

    PW_NODE_ACTIVATION_NOT_TRIGGERED	= 0,
    PW_NODE_ACTIVATION_TRIGGERED		= 1,
    PW_NODE_ACTIVATION_AWAKE		= 2,
    PW_NODE_ACTIVATION_FINISHED		= 3,
}

ty! {
    PW_NODE_ACTIVATION_COMMAND;

    PW_NODE_ACTIVATION_COMMAND_NONE		= 0,
    PW_NODE_ACTIVATION_COMMAND_START	= 1,
    PW_NODE_ACTIVATION_COMMAND_STOP		= 2,
}

#[repr(C)]
#[derive(Debug)]
pub struct pw_node_activation {
    pub status: PW_NODE_ACTIVATION,

    pub flags: c::c_uint,

    pub state: [pw_node_activation_state; 2],

    pub signal_time: u64,
    pub awake_time: u64,
    pub finish_time: u64,
    pub prev_signal_time: u64,

    pub reposition: spa_io_segment,
    pub segment: spa_io_segment,

    pub segment_owner: [u32; 32],
    pub position: spa_io_position,

    pub sync_timeout: u64,
    pub sync_left: u64,

    pub cpu_load: [f32; 3],
    pub xrun_count: u32,
    pub xrun_time: u64,
    pub xrun_delay: u64,
    pub max_delay: u64,

    pub command: PW_NODE_ACTIVATION_COMMAND,
    pub reposition_owner: u32,
}

unsafe impl Pod for pw_node_activation {}

bitflags! {
    SPA_PORT_FLAG: u64;

    SPA_PORT_FLAG_REMOVABLE         = 1<<0,
    SPA_PORT_FLAG_OPTIONAL          = 1<<1,
    SPA_PORT_FLAG_CAN_ALLOC_BUFFERS = 1<<2,
    SPA_PORT_FLAG_IN_PLACE          = 1<<3,
    SPA_PORT_FLAG_NO_REF            = 1<<4,
    SPA_PORT_FLAG_LIVE              = 1<<5,
    SPA_PORT_FLAG_PHYSICAL          = 1<<6,
    SPA_PORT_FLAG_TERMINAL          = 1<<7,
    SPA_PORT_FLAG_DYNAMIC_DATA      = 1<<8,
}

bitflags! {
    SpaStatus: u32;

    SPA_STATUS_NEED_DATA = 1 << 0,
    SPA_STATUS_HAVE_DATA = 1 << 1,
    SPA_STATUS_STOPPED   = 1 << 2,
    SPA_STATUS_DRAINED   = 1 << 3,
}

#[repr(C)]
#[derive(Debug)]
pub struct spa_io_buffers {
    pub status: AtomicU32,
    pub buffer_id: AtomicU32,
}

unsafe impl Pod for spa_io_buffers {}

bitflags! {
    SpaChunkFlags: u32;

    SPA_CHUNK_FLAG_CORRUPTED = 1,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct spa_chunk {
    pub offset: u32,
    pub size: u32,
    pub stride: u32,
    pub flags: SpaChunkFlags,
}

unsafe impl Pod for spa_chunk {}
