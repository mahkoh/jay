use {
    crate::utils::{
        bitflags::BitflagsExt, clonecell::UnsafeCellCloneSafe, ptr_ext::PtrExt, stack::Stack,
    },
    bstr::{BString, ByteSlice},
    std::{
        fmt::{Debug, Formatter},
        rc::Rc,
    },
    thiserror::Error,
};

#[derive(Copy, Clone, Debug)]
pub enum ColorBitDepth {
    Undefined,
    Bits6,
    Bits8,
    Bits10,
    Bits12,
    Bits14,
    Bits16,
    Reserved,
}

#[derive(Copy, Clone, Debug)]
pub enum DigitalVideoInterfaceStandard {
    Undefined,
    Dvi,
    HdmiA,
    HdmiB,
    MDDI,
    DisplayPort,
    #[expect(dead_code)]
    Unknown(u8),
}

#[derive(Copy, Clone)]
pub struct SignalLevelStandard(u8);

impl Debug for SignalLevelStandard {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self.0 {
            0 => "+0.7/−0.3 V",
            1 => "+0.714/−0.286 V",
            2 => "+1.0/−0.4 V",
            _ => "+0.7/0 V",
        };
        Debug::fmt(s, f)
    }
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub enum VideoInputDefinition {
    Analog {
        signal_level_standard: SignalLevelStandard,
        blank_to_black_setup_or_pedestal: bool,
        separate_h_v_sync_supported: bool,
        composite_sync_on_horizontal_supported: bool,
        composite_sync_on_green_supported: bool,
        serration_on_vertical_sync_supported: bool,
    },
    Digital {
        bit_depth: ColorBitDepth,
        video_interface: DigitalVideoInterfaceStandard,
    },
}

#[derive(Copy, Clone, Debug)]
pub struct ScreenDimensions {
    pub horizontal_screen_size_cm: Option<u8>,
    pub vertical_screen_size_cm: Option<u8>,
    pub landscape_aspect_ration: Option<f64>,
    pub portrait_aspect_ration: Option<f64>,
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub struct ChromaticityCoordinates {
    pub red_x: u16,
    pub red_y: u16,
    pub green_x: u16,
    pub green_y: u16,
    pub blue_x: u16,
    pub blue_y: u16,
    pub white_x: u16,
    pub white_y: u16,
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub struct EstablishedTimings {
    pub s_720x400_70: bool,
    pub s_720x400_88: bool,
    pub s_640x480_60: bool,
    pub s_640x480_67: bool,
    pub s_640x480_72: bool,
    pub s_640x480_75: bool,
    pub s_800x600_56: bool,
    pub s_800x600_60: bool,
    pub s_800x600_72: bool,
    pub s_800x600_75: bool,
    pub s_832x624_75: bool,
    pub s_1024x768_87: bool,
    pub s_1024x768_60: bool,
    pub s_1024x768_70: bool,
    pub s_1024x768_75: bool,
    pub s_1280x1024_75: bool,
    pub s_1152x870_75: bool,
}

#[derive(Copy, Clone, Debug)]
pub enum AspectRatio {
    A1_1,
    A16_10,
    A4_3,
    A5_4,
    A16_9,
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub struct StandardTiming {
    pub x_resolution: u16,
    pub aspect_ratio: AspectRatio,
    pub vertical_frequency: u8,
}

#[derive(Copy, Clone, Debug)]
pub enum AnalogSyncType {
    AnalogComposite,
    BipolarAnalogComposite,
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub enum SyncSignal {
    Analog {
        ty: AnalogSyncType,
        with_serrations: bool,
        sync_on_all_signals: bool,
    },
    DigitalComposite {
        with_serration: bool,
        horizontal_sync_is_positive: bool,
    },
    DigitalSeparate {
        vertical_sync_is_positive: bool,
        horizontal_sync_is_positive: bool,
    },
}

#[derive(Copy, Clone)]
pub enum StereoViewingSupport {
    None,
    FieldSequentialRightDuringStereoSync,
    FieldSequentialLeftDuringStereoSync,
    TwoWayInterleavedRightImageOnEvenLines,
    TwoWayInterleavedLeftImageOnEvenLines,
    FourWayInterleaved,
    SideBySideInterleaved,
}

impl Debug for StereoViewingSupport {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let msg = match *self {
            StereoViewingSupport::None => "none",
            StereoViewingSupport::FieldSequentialRightDuringStereoSync => {
                "field sequential, right during stereo sync"
            }
            StereoViewingSupport::FieldSequentialLeftDuringStereoSync => {
                "field sequential, left during stereo sync"
            }
            StereoViewingSupport::TwoWayInterleavedRightImageOnEvenLines => {
                "2-way interleaved, right image on even lines"
            }
            StereoViewingSupport::TwoWayInterleavedLeftImageOnEvenLines => {
                "2-way interleaved, left image on even lines"
            }
            StereoViewingSupport::FourWayInterleaved => "4-way interleaved",
            StereoViewingSupport::SideBySideInterleaved => "side-by-side interleaved",
        };
        write!(f, "\"{}\"", msg)
    }
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub struct DisplayRangeLimitsAndAdditionalTiming {
    pub vertical_field_rate_min: u16,
    pub vertical_field_rate_max: u16,
    pub horizontal_field_rate_min: u16,
    pub horizontal_field_rate_max: u16,
    pub maximum_pixel_clock_mhz: u16,
    pub extended_timing_information: ExtendedTimingInformation,
}

#[derive(Copy, Clone, Debug)]
pub enum AspectRatioPreference {
    A4_3,
    A16_9,
    A16_10,
    A5_4,
    A15_9,
    #[expect(dead_code)]
    Unknown(u8),
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub enum ExtendedTimingInformation {
    DefaultGtf,
    NoTimingInformation,
    SecondaryGtf {
        start_frequency: u16,
        c_value: u16,
        m_value: u16,
        k_value: u8,
        j_value: u16,
    },
    Cvt {
        cvt_major_version: u8,
        cvt_minor_version: u8,
        additional_clock_precision: u8,
        maximum_active_pixels_per_line: Option<u16>,
        ar_4_3: bool,
        ar_16_9: bool,
        ar_16_10: bool,
        ar_5_4: bool,
        ar_15_9: bool,
        ar_preference: AspectRatioPreference,
        cvt_rb_reduced_blanking_preferred: bool,
        cvt_standard_blanking: bool,
        scaling_support_horizontal_shrink: bool,
        scaling_support_horizontal_stretch: bool,
        scaling_support_vertical_shrink: bool,
        scaling_support_vertical_stretch: bool,
        preferred_vertical_refresh_rate_hz: u8,
    },
    Unknown(u8),
}

#[derive(Copy, Clone, Debug, Default)]
#[expect(dead_code)]
pub struct ColorPoint {
    pub white_point_index: u8,
    pub white_point_x: u16,
    pub white_point_y: u16,
    pub gamma: Option<f64>,
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub struct EstablishedTimings3 {
    pub s640x350_85: bool,
    pub s640x400_85: bool,
    pub s720x400_85: bool,
    pub s640x480_85: bool,
    pub s848x480_60: bool,
    pub s800x600_85: bool,
    pub s1024x768_85: bool,
    pub s1152x864_75: bool,
    pub s1280x768_60_rb: bool,
    pub s1280x768_60: bool,
    pub s1280x768_75: bool,
    pub s1280x768_85: bool,
    pub s1280x960_60: bool,
    pub s1280x960_85: bool,
    pub s1280x1024_60: bool,
    pub s1280x1024_85: bool,
    pub s1360x768_60: bool,
    pub s1440x900_60_rb: bool,
    pub s1440x900_60: bool,
    pub s1440x900_75: bool,
    pub s1440x900_85: bool,
    pub s1400x1050_60_rb: bool,
    pub s1400x1050_60: bool,
    pub s1400x1050_75: bool,
    pub s1400x1050_85: bool,
    pub s1680x1050_60_rb: bool,
    pub s1680x1050_60: bool,
    pub s1680x1050_75: bool,
    pub s1680x1050_85: bool,
    pub s1600x1200_60: bool,
    pub s1600x1200_65: bool,
    pub s1600x1200_70: bool,
    pub s1600x1200_75: bool,
    pub s1600x1200_85: bool,
    pub s1792x1344_60: bool,
    pub s1792x1344_75: bool,
    pub s1856x1392_60: bool,
    pub s1856x1392_75: bool,
    pub s1920x1200_60_rb: bool,
    pub s1920x1200_60: bool,
    pub s1920x1200_75: bool,
    pub s1920x1200_85: bool,
    pub s1920x1440_60: bool,
    pub s1920x1440_75: bool,
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub struct ColorManagementData {
    pub red_a3: u16,
    pub red_a2: u16,
    pub green_a3: u16,
    pub green_a2: u16,
    pub blue_a3: u16,
    pub blue_a2: u16,
}

#[derive(Copy, Clone, Debug)]
pub enum CvtAspectRatio {
    A4_3,
    A16_9,
    A16_10,
    A15_9,
}

#[derive(Copy, Clone, Debug)]
pub enum CvtPreferredVerticalRate {
    R50,
    R60,
    R75,
    R85,
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub struct Cvt3ByteCode {
    pub addressable_lines_per_field: u16,
    pub aspect_ration: CvtAspectRatio,
    pub preferred_vertical_rate: CvtPreferredVerticalRate,
    pub r50: bool,
    pub r60: bool,
    pub r75: bool,
    pub r85: bool,
    pub r60_reduced_blanking: bool,
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub struct DetailedTimingDescriptor {
    pub pixel_clock_khz: u32,
    pub horizontal_addressable_pixels: u16,
    pub horizontal_blanking_pixels: u16,
    pub vertical_addressable_lines: u16,
    pub vertical_blanking_lines: u16,
    pub horizontal_front_porch_pixels: u16,
    pub horizontal_sync_pulse_pixels: u16,
    pub vertical_front_porch_lines: u8,
    pub vertical_sync_pulse_lines: u8,
    pub horizontal_addressable_mm: u16,
    pub vertical_addressable_mm: u16,
    pub horizontal_left_border_pixels: u8,
    pub vertical_top_border_pixels: u8,
    pub interlaced: bool,
    pub stereo_viewing_support: StereoViewingSupport,
    pub sync: SyncSignal,
}

#[derive(Clone, Debug)]
#[expect(dead_code)]
pub enum Descriptor {
    Unknown(u8),
    DetailedTimingDescriptor(DetailedTimingDescriptor),
    DisplayProductSerialNumber(String),
    AlphanumericDataString(String),
    DisplayProductName(String),
    DisplayRangeLimitsAndAdditionalTiming(DisplayRangeLimitsAndAdditionalTiming),
    EstablishedTimings3(EstablishedTimings3),
    ColorManagementData(ColorManagementData),
    StandardTimingIdentifier([Option<StandardTiming>; 6]),
    ColorPoint(ColorPoint, Option<ColorPoint>),
    Cvt3ByteCode([Cvt3ByteCode; 4]),
}

type EdidContext = (usize, EdidParseContext);

struct EdidParser<'a> {
    data: &'a [u8],
    pos: usize,
    context: Rc<Stack<EdidContext>>,
    saved_ctx: Vec<EdidContext>,
    errors: Vec<(EdidError, Vec<EdidContext>)>,
}

macro_rules! bail {
    ($slf:expr, $err:expr) => {{
        $slf.saved_ctx = $slf.context.to_vec();
        return Err($err);
    }};
}

#[derive(Clone, Debug)]
pub enum EdidParseContext {
    #[expect(dead_code)]
    ReadingBytes(usize),
    BaseBlock,
    Descriptors,
    Descriptor,
    ChromaticityCoordinates,
    EstablishedTimings,
    StandardTimings,
    ScreenDimensions,
    Gamma,
    FeatureSupport,
    Magic,
    Extension,
    IdManufacturerName,
    VideoInputDefinition,
}

unsafe impl UnsafeCellCloneSafe for EdidParseContext {}

struct EdidPushedContext {
    stack: Rc<Stack<(usize, EdidParseContext)>>,
}

impl Drop for EdidPushedContext {
    fn drop(&mut self) {
        self.stack.pop();
    }
}

impl<'a> EdidParser<'a> {
    fn push_ctx(&self, pc: EdidParseContext) -> EdidPushedContext {
        self.context.push((self.pos, pc));
        EdidPushedContext {
            stack: self.context.clone(),
        }
    }

    fn nest(&self, data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            context: self.context.clone(),
            saved_ctx: vec![],
            errors: vec![],
        }
    }

    fn store_error(&mut self, error: EdidError) {
        self.errors.push((error, self.saved_ctx.clone()));
    }

    fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn read_n<const N: usize>(&mut self) -> Result<&'a [u8; N], EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::ReadingBytes(N));
        if self.data.len() - self.pos < N {
            bail!(self, EdidError::UnexpectedEof);
        }
        let v = unsafe { self.data[self.pos..].as_ptr().cast::<[u8; N]>().deref() };
        self.pos += N;
        Ok(v)
    }

    fn read_var_n(&mut self, n: usize) -> Result<&'a [u8], EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::ReadingBytes(n));
        if self.data.len() - self.pos < n {
            bail!(self, EdidError::UnexpectedEof);
        }
        let v = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(v)
    }

    fn read_u8(&mut self) -> Result<u8, EdidError> {
        let &[a] = self.read_n()?;
        Ok(a)
    }

    fn read_u16(&mut self) -> Result<u16, EdidError> {
        let &[lo, hi] = self.read_n()?;
        Ok(((hi as u16) << 8) + lo as u16)
    }

    fn read_u32(&mut self) -> Result<u32, EdidError> {
        let &[a, b, c, d] = self.read_n()?;
        Ok(((d as u32) << 24) + ((c as u32) << 16) + ((b as u32) << 8) + a as u32)
    }

    fn parse_magic(&mut self) -> Result<(), EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::Magic);
        let magic = self.read_n::<8>()?;
        if magic != &[0, 255, 255, 255, 255, 255, 255, 0] {
            bail!(self, EdidError::InvalidMagic(magic.as_bstr().to_owned()));
        }
        Ok(())
    }

    fn parse_id_manufacturer_name(&mut self) -> Result<BString, EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::IdManufacturerName);
        let name = self.read_n::<2>()?;
        let a = (name[0] >> 2) & 0b11111;
        let b = ((name[0] & 0b11) << 3) | (name[1] >> 5);
        let c = name[1] & 0b11111;
        let name = [a + b'@', b + b'@', c + b'@'].as_bstr().to_owned();
        Ok(name)
    }

    fn parse_video_input_definition(&mut self) -> Result<VideoInputDefinition, EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::VideoInputDefinition);
        let val = self.read_u8()?;
        let res = if val.contains(0x80) {
            VideoInputDefinition::Digital {
                bit_depth: match (val >> 4) & 0b111 {
                    0b000 => ColorBitDepth::Undefined,
                    0b001 => ColorBitDepth::Bits6,
                    0b010 => ColorBitDepth::Bits8,
                    0b011 => ColorBitDepth::Bits10,
                    0b100 => ColorBitDepth::Bits12,
                    0b101 => ColorBitDepth::Bits14,
                    0b110 => ColorBitDepth::Bits16,
                    _ => ColorBitDepth::Reserved,
                },
                video_interface: match val & 0b1111 {
                    0b0000 => DigitalVideoInterfaceStandard::Undefined,
                    0b0001 => DigitalVideoInterfaceStandard::Dvi,
                    0b0010 => DigitalVideoInterfaceStandard::HdmiA,
                    0b0011 => DigitalVideoInterfaceStandard::HdmiB,
                    0b0100 => DigitalVideoInterfaceStandard::MDDI,
                    0b0101 => DigitalVideoInterfaceStandard::DisplayPort,
                    n => DigitalVideoInterfaceStandard::Unknown(n),
                },
            }
        } else {
            VideoInputDefinition::Analog {
                signal_level_standard: SignalLevelStandard((val >> 5) & 0b11),
                blank_to_black_setup_or_pedestal: (val >> 4).contains(1),
                separate_h_v_sync_supported: (val >> 3).contains(1),
                composite_sync_on_horizontal_supported: (val >> 2).contains(1),
                composite_sync_on_green_supported: (val >> 1).contains(1),
                serration_on_vertical_sync_supported: (val >> 0).contains(1),
            }
        };
        Ok(res)
    }

    fn parse_screen_dimensions(&mut self) -> Result<ScreenDimensions, EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::ScreenDimensions);
        let &[hor, vert] = self.read_n()?;
        let mut res = ScreenDimensions {
            horizontal_screen_size_cm: None,
            vertical_screen_size_cm: None,
            landscape_aspect_ration: None,
            portrait_aspect_ration: None,
        };
        if hor != 0 && vert != 0 {
            res.horizontal_screen_size_cm = Some(hor);
            res.vertical_screen_size_cm = Some(vert);
        } else if vert != 0 {
            res.portrait_aspect_ration = Some(100.0 / (vert as f64 + 99.0));
        } else if hor != 0 {
            res.landscape_aspect_ration = Some((hor as f64 + 99.0) / 100.0);
        }
        Ok(res)
    }

    fn parse_gamma(&mut self) -> Result<Option<f64>, EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::Gamma);
        let val = self.read_u8()?;
        if val == 0xff {
            Ok(None)
        } else {
            Ok(Some((val as f64 + 100.0) / 100.0))
        }
    }

    fn parse_feature_support(&mut self, digital: bool) -> Result<FeatureSupport, EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::FeatureSupport);
        let val = self.read_u8()?;
        Ok(FeatureSupport {
            standby_supported: val.contains(0x80),
            suspend_supported: val.contains(0x40),
            active_off_supported: val.contains(0x20),
            features: if digital {
                FeatureSupport2::Digital {
                    rgb444_supported: true,
                    ycrcb422_supported: val.contains(0x10),
                    ycrcb444_supported: val.contains(0x08),
                }
            } else {
                FeatureSupport2::Analog {
                    display_color_type: match (val >> 3) & 0b11 {
                        0b00 => DisplayColorType::Monochrome,
                        0b01 => DisplayColorType::Rgb,
                        0b10 => DisplayColorType::NonRgb,
                        _ => DisplayColorType::Undefined,
                    },
                }
            },
            srgb_is_default_color_space: val.contains(0x04),
            preferred_mode_is_native: val.contains(0x02),
            display_is_continuous_frequency: val.contains(0x01),
        })
    }

    fn parse_chromaticity_coordinates(&mut self) -> Result<ChromaticityCoordinates, EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::ChromaticityCoordinates);
        let b = self.read_n::<10>()?;
        let rx = ((b[0] as u16 >> 6) & 0b11) + ((b[2] as u16) << 2);
        let ry = ((b[0] as u16 >> 4) & 0b11) + ((b[3] as u16) << 2);
        let gx = ((b[0] as u16 >> 2) & 0b11) + ((b[4] as u16) << 2);
        let gy = ((b[0] as u16 >> 0) & 0b11) + ((b[5] as u16) << 2);
        let bx = ((b[1] as u16 >> 6) & 0b11) + ((b[6] as u16) << 2);
        let by = ((b[1] as u16 >> 4) & 0b11) + ((b[7] as u16) << 2);
        let wx = ((b[1] as u16 >> 2) & 0b11) + ((b[8] as u16) << 2);
        let wy = ((b[1] as u16 >> 0) & 0b11) + ((b[9] as u16) << 2);
        Ok(ChromaticityCoordinates {
            red_x: rx,
            red_y: ry,
            green_x: gx,
            green_y: gy,
            blue_x: bx,
            blue_y: by,
            white_x: wx,
            white_y: wy,
        })
    }

    fn parse_established_timings(&mut self) -> Result<EstablishedTimings, EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::EstablishedTimings);
        let b = self.read_n::<3>()?;
        Ok(EstablishedTimings {
            s_720x400_70: b[0].contains(0x80),
            s_720x400_88: b[0].contains(0x40),
            s_640x480_60: b[0].contains(0x20),
            s_640x480_67: b[0].contains(0x10),
            s_640x480_72: b[0].contains(0x08),
            s_640x480_75: b[0].contains(0x04),
            s_800x600_56: b[0].contains(0x02),
            s_800x600_60: b[0].contains(0x01),
            s_800x600_72: b[0].contains(0x80),
            s_800x600_75: b[0].contains(0x40),
            s_832x624_75: b[0].contains(0x20),
            s_1024x768_87: b[0].contains(0x10),
            s_1024x768_60: b[0].contains(0x08),
            s_1024x768_70: b[0].contains(0x04),
            s_1024x768_75: b[0].contains(0x02),
            s_1280x1024_75: b[0].contains(0x01),
            s_1152x870_75: b[0].contains(0x80),
        })
    }

    fn parse_standard_timing(&mut self, revision: u8, a: u8, b: u8) -> Option<StandardTiming> {
        if a == 0 {
            return None;
        }
        Some(StandardTiming {
            x_resolution: (a as u16 + 31) * 8,
            aspect_ratio: match b >> 6 {
                0b00 if revision < 3 => AspectRatio::A1_1,
                0b00 => AspectRatio::A16_10,
                0b01 => AspectRatio::A4_3,
                0b10 => AspectRatio::A5_4,
                _ => AspectRatio::A16_9,
            },
            vertical_frequency: 60 + (b & 0b111111),
        })
    }

    fn parse_standard_timings2(
        &mut self,
        revision: u8,
        b: &[u8; 18],
    ) -> [Option<StandardTiming>; 6] {
        let mut res = [None; 6];
        for i in 0..6 {
            let x = b[5 + 2 * i];
            let y = b[5 + 2 * i + 1];
            res[i] = self.parse_standard_timing(revision, x, y);
        }
        res
    }

    fn parse_color_point(&mut self, b: &[u8; 18]) -> (ColorPoint, Option<ColorPoint>) {
        let mut res = [Default::default(); 2];
        for n in 0..2 {
            let b = &b[5 * (n + 1)..];
            res[n] = ColorPoint {
                white_point_index: b[0],
                white_point_x: ((b[2] as u16) << 2) | ((b[1] as u16) >> 2),
                white_point_y: ((b[3] as u16) << 2) | ((b[1] as u16) & 0b11),
                gamma: if b[4] == 0xff {
                    None
                } else {
                    Some((b[5] as f64 + 100.0) / 100.0)
                },
            };
        }
        let second = if res[1].white_point_index != 0 {
            Some(res[1])
        } else {
            None
        };
        (res[0], second)
    }

    fn parse_standard_timings(
        &mut self,
        revision: u8,
    ) -> Result<[Option<StandardTiming>; 8], EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::StandardTimings);
        let bytes = self.read_n::<16>()?;
        let mut res = [None; 8];
        for i in 0..8 {
            let a = bytes[2 * i];
            let b = bytes[2 * i + 1];
            if (a, b) != (1, 1) {
                res[i] = self.parse_standard_timing(revision, a, b);
            }
        }
        Ok(res)
    }

    fn parse_detailed_timing_descriptor(&self, b: &[u8; 18]) -> DetailedTimingDescriptor {
        let l = b[17];
        DetailedTimingDescriptor {
            pixel_clock_khz: u16::from_le_bytes([b[0], b[1]]) as u32 * 10_000,
            horizontal_addressable_pixels: u16::from_le_bytes([b[2], b[4] >> 4]),
            horizontal_blanking_pixels: u16::from_le_bytes([b[3], b[4] & 0b1111]),
            vertical_addressable_lines: u16::from_le_bytes([b[5], b[7] >> 4]),
            vertical_blanking_lines: u16::from_le_bytes([b[6], b[7] & 0b1111]),
            horizontal_front_porch_pixels: u16::from_le_bytes([b[8], b[11] >> 6]),
            horizontal_sync_pulse_pixels: u16::from_le_bytes([b[9], (b[11] >> 4) & 0b11]),
            vertical_front_porch_lines: (b[10] >> 4) | ((b[11] & 0b1100) << 2),
            vertical_sync_pulse_lines: (b[10] & 0b1111) | ((b[11] & 0b11) << 4),
            horizontal_addressable_mm: u16::from_le_bytes([b[12], b[14] >> 4]),
            vertical_addressable_mm: u16::from_le_bytes([b[13], b[14] & 0b1111]),
            horizontal_left_border_pixels: b[15],
            vertical_top_border_pixels: b[16],
            interlaced: l.contains(0x80),
            stereo_viewing_support: match ((l >> 4) & 0b110) | (l & 0b1) {
                0b010 => StereoViewingSupport::FieldSequentialRightDuringStereoSync,
                0b100 => StereoViewingSupport::FieldSequentialLeftDuringStereoSync,
                0b011 => StereoViewingSupport::TwoWayInterleavedRightImageOnEvenLines,
                0b101 => StereoViewingSupport::TwoWayInterleavedLeftImageOnEvenLines,
                0b110 => StereoViewingSupport::FourWayInterleaved,
                0b111 => StereoViewingSupport::SideBySideInterleaved,
                _ => StereoViewingSupport::None,
            },
            sync: if l.contains(0b10000) {
                if l.contains(0b01000) {
                    SyncSignal::DigitalSeparate {
                        vertical_sync_is_positive: l.contains(0b100),
                        horizontal_sync_is_positive: l.contains(0b10),
                    }
                } else {
                    SyncSignal::DigitalComposite {
                        with_serration: l.contains(0b100),
                        horizontal_sync_is_positive: l.contains(0b10),
                    }
                }
            } else {
                SyncSignal::Analog {
                    ty: if l.contains(0b1000) {
                        AnalogSyncType::BipolarAnalogComposite
                    } else {
                        AnalogSyncType::AnalogComposite
                    },
                    with_serrations: l.contains(0b100),
                    sync_on_all_signals: l.contains(0b10),
                }
            },
        }
    }

    fn parse_display_range_limits_and_additional_timing(
        &self,
        b: &[u8; 18],
    ) -> DisplayRangeLimitsAndAdditionalTiming {
        let min_vert_off = b[4].contains(0b0001);
        let max_vert_off = min_vert_off || b[4].contains(0b0010);
        let min_horz_off = b[4].contains(0b0100);
        let max_horz_off = min_horz_off || b[4].contains(0b1000);
        DisplayRangeLimitsAndAdditionalTiming {
            vertical_field_rate_min: b[5] as u16 + if min_vert_off { 255 } else { 0 },
            vertical_field_rate_max: b[6] as u16 + if max_vert_off { 255 } else { 0 },
            horizontal_field_rate_min: b[7] as u16 + if min_horz_off { 255 } else { 0 },
            horizontal_field_rate_max: b[8] as u16 + if max_horz_off { 255 } else { 0 },
            maximum_pixel_clock_mhz: b[9] as u16 * 10,
            extended_timing_information: match b[10] {
                0x0 => ExtendedTimingInformation::DefaultGtf,
                0x1 => ExtendedTimingInformation::NoTimingInformation,
                0x2 => ExtendedTimingInformation::SecondaryGtf {
                    start_frequency: b[12] as u16,
                    c_value: b[13] as u16,
                    m_value: u16::from_le_bytes([b[14], b[15]]),
                    k_value: b[16],
                    j_value: b[17] as u16,
                },
                0x4 => ExtendedTimingInformation::Cvt {
                    cvt_major_version: b[11] >> 4,
                    cvt_minor_version: b[11] & 0b1111,
                    additional_clock_precision: b[12] >> 2,
                    maximum_active_pixels_per_line: if b[13] == 0 {
                        None
                    } else {
                        Some((((b[12] as u16 & 0b11) << 8) | b[13] as u16) * 8)
                    },
                    ar_4_3: b[14].contains(0x80),
                    ar_16_9: b[14].contains(0x40),
                    ar_16_10: b[14].contains(0x20),
                    ar_5_4: b[14].contains(0x10),
                    ar_15_9: b[14].contains(0x08),
                    ar_preference: match b[15] >> 5 {
                        0b000 => AspectRatioPreference::A4_3,
                        0b001 => AspectRatioPreference::A16_9,
                        0b010 => AspectRatioPreference::A16_10,
                        0b011 => AspectRatioPreference::A5_4,
                        0b100 => AspectRatioPreference::A15_9,
                        n => AspectRatioPreference::Unknown(n),
                    },
                    cvt_rb_reduced_blanking_preferred: b[15].contains(0b10000),
                    cvt_standard_blanking: b[15].contains(0b1000),
                    scaling_support_horizontal_shrink: b[16].contains(0x80),
                    scaling_support_horizontal_stretch: b[16].contains(0x40),
                    scaling_support_vertical_shrink: b[16].contains(0x20),
                    scaling_support_vertical_stretch: b[16].contains(0x10),
                    preferred_vertical_refresh_rate_hz: b[17],
                },
                n => ExtendedTimingInformation::Unknown(n),
            },
        }
    }

    fn parse_established_timings3(&self, b: &[u8; 18]) -> EstablishedTimings3 {
        EstablishedTimings3 {
            s640x350_85: b[6].contains(0x80),
            s640x400_85: b[6].contains(0x40),
            s720x400_85: b[6].contains(0x20),
            s640x480_85: b[6].contains(0x10),
            s848x480_60: b[6].contains(0x08),
            s800x600_85: b[6].contains(0x04),
            s1024x768_85: b[6].contains(0x02),
            s1152x864_75: b[6].contains(0x01),
            s1280x768_60_rb: b[7].contains(0x80),
            s1280x768_60: b[7].contains(0x40),
            s1280x768_75: b[7].contains(0x20),
            s1280x768_85: b[7].contains(0x10),
            s1280x960_60: b[7].contains(0x08),
            s1280x960_85: b[7].contains(0x04),
            s1280x1024_60: b[7].contains(0x02),
            s1280x1024_85: b[7].contains(0x01),
            s1360x768_60: b[8].contains(0x80),
            s1440x900_60_rb: b[8].contains(0x40),
            s1440x900_60: b[8].contains(0x20),
            s1440x900_75: b[8].contains(0x10),
            s1440x900_85: b[8].contains(0x08),
            s1400x1050_60_rb: b[8].contains(0x04),
            s1400x1050_60: b[8].contains(0x02),
            s1400x1050_75: b[8].contains(0x01),
            s1400x1050_85: b[9].contains(0x80),
            s1680x1050_60_rb: b[9].contains(0x40),
            s1680x1050_60: b[9].contains(0x20),
            s1680x1050_75: b[9].contains(0x10),
            s1680x1050_85: b[9].contains(0x08),
            s1600x1200_60: b[9].contains(0x04),
            s1600x1200_65: b[9].contains(0x02),
            s1600x1200_70: b[9].contains(0x01),
            s1600x1200_75: b[10].contains(0x80),
            s1600x1200_85: b[10].contains(0x40),
            s1792x1344_60: b[10].contains(0x20),
            s1792x1344_75: b[10].contains(0x10),
            s1856x1392_60: b[10].contains(0x08),
            s1856x1392_75: b[10].contains(0x04),
            s1920x1200_60_rb: b[10].contains(0x02),
            s1920x1200_60: b[10].contains(0x01),
            s1920x1200_75: b[11].contains(0x80),
            s1920x1200_85: b[11].contains(0x40),
            s1920x1440_60: b[11].contains(0x20),
            s1920x1440_75: b[11].contains(0x10),
        }
    }

    fn parse_color_management_data(&self, b: &[u8; 18]) -> ColorManagementData {
        ColorManagementData {
            red_a3: u16::from_le_bytes([b[6], b[7]]),
            red_a2: u16::from_le_bytes([b[8], b[9]]),
            green_a3: u16::from_le_bytes([b[10], b[11]]),
            green_a2: u16::from_le_bytes([b[12], b[13]]),
            blue_a3: u16::from_le_bytes([b[14], b[15]]),
            blue_a2: u16::from_le_bytes([b[16], b[17]]),
        }
    }

    fn parse_cvt3_byte_codes(&self, b: &[u8; 18]) -> [Cvt3ByteCode; 4] {
        let parse = |n: usize| {
            let b = &b[6 + 3 * n..];
            Cvt3ByteCode {
                addressable_lines_per_field: u16::from_le_bytes([b[0], b[1] >> 4]),
                aspect_ration: match (b[1] >> 2) & 0b11 {
                    0 => CvtAspectRatio::A4_3,
                    1 => CvtAspectRatio::A16_9,
                    2 => CvtAspectRatio::A16_10,
                    _ => CvtAspectRatio::A15_9,
                },
                preferred_vertical_rate: match (b[2] >> 5) & 0b11 {
                    0 => CvtPreferredVerticalRate::R50,
                    1 => CvtPreferredVerticalRate::R60,
                    2 => CvtPreferredVerticalRate::R75,
                    _ => CvtPreferredVerticalRate::R85,
                },
                r50: b[2].contains(0b10000),
                r60: b[2].contains(0b01000),
                r75: b[2].contains(0b00100),
                r85: b[2].contains(0b00010),
                r60_reduced_blanking: b[2].contains(0b00001),
            }
        };
        [parse(0), parse(1), parse(2), parse(3)]
    }

    fn parse_descriptor(&mut self, revision: u8) -> Result<Option<Descriptor>, EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::Descriptor);
        let b = self.read_n::<18>()?;
        let str = || {
            let mut s = &b[5..];
            if let Some(n) = s.find_byte(b'\n') {
                s = &s[..n];
            };
            let mut res = String::new();
            for &b in s {
                res.push_str(CP437[b as usize]);
            }
            res
        };
        let res = if (b[0], b[1]) == (0, 0) {
            match b[3] {
                0xff => Descriptor::DisplayProductSerialNumber(str()),
                0xfe => Descriptor::AlphanumericDataString(str()),
                0xfd => Descriptor::DisplayRangeLimitsAndAdditionalTiming(
                    self.parse_display_range_limits_and_additional_timing(b),
                ),
                0xfc => Descriptor::DisplayProductName(str()),
                0xfb => {
                    let (first, second) = self.parse_color_point(b);
                    Descriptor::ColorPoint(first, second)
                }
                0xfa => {
                    Descriptor::StandardTimingIdentifier(self.parse_standard_timings2(revision, b))
                }
                0xf9 => Descriptor::ColorManagementData(self.parse_color_management_data(b)),
                0xf8 => Descriptor::Cvt3ByteCode(self.parse_cvt3_byte_codes(b)),
                0xf7 => Descriptor::EstablishedTimings3(self.parse_established_timings3(b)),
                0x10 => return Ok(None),
                n => Descriptor::Unknown(n),
            }
        } else {
            Descriptor::DetailedTimingDescriptor(self.parse_detailed_timing_descriptor(b))
        };
        Ok(Some(res))
    }

    fn parse_descriptors(&mut self, revision: u8) -> Result<[Option<Descriptor>; 4], EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::Descriptors);
        let mut res = [None, None, None, None];
        for res in &mut res {
            *res = self.parse_descriptor(revision)?;
        }
        Ok(res)
    }

    fn parse_base_block(&mut self) -> Result<EdidBaseBlock, EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::BaseBlock);
        self.parse_magic()?;
        let id_manufacturer_name = self.parse_id_manufacturer_name()?;
        let id_product_code = self.read_u16()?;
        let id_serial_number = self.read_u32()?;
        let mut week_of_manufacture = None;
        let mut model_year = None;
        let mut year_of_manufacture = None;
        {
            let &[a, b] = self.read_n()?;
            if matches!(a, 1..=0x36) {
                week_of_manufacture = Some(a);
            }
            let year = b as u16 + 1990;
            if a == 0xff {
                model_year = Some(year);
            } else {
                year_of_manufacture = Some(year);
            }
        }
        let &[edid_version, edid_revision] = self.read_n()?;
        let video_input_definition = self.parse_video_input_definition()?;
        let is_digital = matches!(video_input_definition, VideoInputDefinition::Digital { .. });
        let screen_dimensions = self.parse_screen_dimensions()?;
        let gamma = self.parse_gamma()?;
        let feature_support = self.parse_feature_support(is_digital)?;
        let chromaticity_coordinates = self.parse_chromaticity_coordinates()?;
        let established_timings = self.parse_established_timings()?;
        let standard_timings = self.parse_standard_timings(edid_revision)?;
        let descriptors = self.parse_descriptors(edid_revision)?;
        let num_extensions = self.read_u8()?;
        let _checksum = self.read_u8()?;
        Ok(EdidBaseBlock {
            id_manufacturer_name,
            id_product_code,
            id_serial_number,
            week_of_manufacture,
            model_year,
            year_of_manufacture,
            edid_version,
            edid_revision,
            video_input_definition,
            screen_dimensions,
            gamma,
            feature_support,
            chromaticity_coordinates,
            established_timings,
            standard_timings,
            descriptors,
            num_extensions,
        })
    }

    fn parse_cta_amd_vendor_data_block(&mut self) -> Result<CtaDataBlock, EdidError> {
        let _ = self.read_n::<2>()?;
        Ok(CtaDataBlock::VendorAmd(CtaAmdVendorDataBlock {
            minimum_refresh_hz: self.read_u8()?,
            maximum_refresh_hz: self.read_u8()?,
        }))
    }

    fn parse_cta_vendor_data_block(&mut self) -> Result<CtaDataBlock, EdidError> {
        match self.read_n::<3>()? {
            [0x1A, 0x00, 0x00] => self.parse_cta_amd_vendor_data_block(),
            _ => Ok(CtaDataBlock::Unknown),
        }
    }

    fn parse_cta_colorimetry_data_block(&mut self) -> Result<CtaDataBlock, EdidError> {
        let [lo, hi] = *self.read_n::<2>()?;
        Ok(CtaDataBlock::Colorimetry(CtaColorimetryDataBlock {
            bt2020_rgb: lo.contains(0x80),
            bt2020_ycc: lo.contains(0x40),
            bt2020_cycc: lo.contains(0x20),
            op_rgb: lo.contains(0x10),
            op_ycc_601601: lo.contains(0x08),
            s_ycc_601: lo.contains(0x04),
            xv_ycc_709: lo.contains(0x02),
            xv_ycc_601: lo.contains(0x01),
            dci_p3: hi.contains(0x80),
        }))
    }

    fn parse_cta_hdr_static_metadata_data_block(&mut self) -> Result<CtaDataBlock, EdidError> {
        let et = self.read_u8()?;
        let _ = self.read_u8()?;
        let mut read_luminance = |min: bool| {
            let v = self.read_u8().unwrap_or_default();
            if v == 0 {
                None
            } else if min {
                Some((v as f64 / 255.0).powi(2) / 100.0)
            } else {
                Some(50.0 * 2.0f64.powf(v as f64 / 32.0))
            }
        };
        Ok(CtaDataBlock::StaticHdrMetadata(
            CtaStaticHdrMetadataDataBlock {
                traditional_gamma_sdr_luminance: et.contains(0x01),
                traditional_gamma_hdr_luminance: et.contains(0x02),
                smpte_st_2084: et.contains(0x04),
                hlg: et.contains(0x08),
                max_luminance: read_luminance(false),
                max_frame_average_luminance: read_luminance(false),
                min_luminance: read_luminance(true),
            },
        ))
    }

    fn parse_cta_extended_data_block(&mut self) -> Result<CtaDataBlock, EdidError> {
        match self.read_u8()? {
            0x5 => self.parse_cta_colorimetry_data_block(),
            0x6 => self.parse_cta_hdr_static_metadata_data_block(),
            _ => Ok(CtaDataBlock::Unknown),
        }
    }

    fn parse_cta_data_block(&mut self, tag: u8) -> Result<CtaDataBlock, EdidError> {
        match tag {
            0x3 => self.parse_cta_vendor_data_block(),
            0x7 => self.parse_cta_extended_data_block(),
            _ => Ok(CtaDataBlock::Unknown),
        }
    }

    fn parse_cta_extension_v3(&mut self) -> Result<EdidExtension, EdidError> {
        let detailed_timing_descriptors_offset = self.read_u8()? as usize;
        let _ = self.read_u8()?;
        let mut data_blocks = vec![];
        while self.pos < detailed_timing_descriptors_offset {
            let b1 = self.read_u8()?;
            let data = self.read_var_n(b1 as usize & 0x1f)?;
            let mut parser = self.nest(data);
            match parser.parse_cta_data_block(b1 >> 5) {
                Ok(d) => data_blocks.push(d),
                Err(e) => {
                    self.saved_ctx = parser.saved_ctx;
                    self.store_error(e);
                }
            }
        }
        Ok(EdidExtension::CtaV3(CtaExtensionV3 { data_blocks }))
    }

    fn parse_cta_extension(&mut self) -> Result<EdidExtension, EdidError> {
        // https://web.archive.org/web/20171201033424/https://standards.cta.tech/kwspub/published_docs/CTA-861-G_FINAL_revised_2017.pdf
        match self.read_u8()? {
            0x3 => self.parse_cta_extension_v3(),
            _ => Ok(EdidExtension::Unknown),
        }
    }

    fn parse_extension_impl(&mut self) -> Result<EdidExtension, EdidError> {
        match self.read_u8()? {
            0x2 => self.parse_cta_extension(),
            _ => Ok(EdidExtension::Unknown),
        }
    }

    fn parse_extension(&mut self) -> Result<EdidExtension, EdidError> {
        let _ctx = self.push_ctx(EdidParseContext::Extension);
        let data = self.read_n::<128>()?;
        let mut parser = self.nest(data);
        let res = parser.parse_extension_impl();
        if res.is_err() {
            self.saved_ctx = parser.saved_ctx;
        }
        res
    }

    fn parse(&mut self) -> Result<EdidFile, EdidError> {
        let bb = self.parse_base_block()?;
        let mut exts = vec![];
        while !self.is_empty() {
            match self.parse_extension() {
                Ok(e) => exts.push(e),
                Err(e) => self.store_error(e),
            }
        }
        Ok(EdidFile {
            base_block: bb,
            extension_blocks: exts,
        })
    }
}

#[derive(Debug)]
pub enum DisplayColorType {
    Monochrome,
    Rgb,
    NonRgb,
    Undefined,
}

#[derive(Debug)]
#[expect(dead_code)]
pub enum FeatureSupport2 {
    Analog {
        display_color_type: DisplayColorType,
    },
    Digital {
        rgb444_supported: bool,
        ycrcb444_supported: bool,
        ycrcb422_supported: bool,
    },
}

#[derive(Debug)]
#[expect(dead_code)]
pub struct FeatureSupport {
    pub standby_supported: bool,
    pub suspend_supported: bool,
    pub active_off_supported: bool,
    pub features: FeatureSupport2,
    pub srgb_is_default_color_space: bool,
    pub preferred_mode_is_native: bool,
    pub display_is_continuous_frequency: bool,
}

#[derive(Debug)]
#[expect(dead_code)]
pub struct EdidBaseBlock {
    pub id_manufacturer_name: BString,
    pub id_product_code: u16,
    pub id_serial_number: u32,
    pub week_of_manufacture: Option<u8>,
    pub model_year: Option<u16>,
    pub year_of_manufacture: Option<u16>,
    pub edid_version: u8,
    pub edid_revision: u8,
    pub video_input_definition: VideoInputDefinition,
    pub screen_dimensions: ScreenDimensions,
    pub gamma: Option<f64>,
    pub feature_support: FeatureSupport,
    pub chromaticity_coordinates: ChromaticityCoordinates,
    pub established_timings: EstablishedTimings,
    pub standard_timings: [Option<StandardTiming>; 8],
    pub descriptors: [Option<Descriptor>; 4],
    pub num_extensions: u8,
}

#[derive(Debug)]
pub enum EdidExtension {
    Unknown,
    CtaV3(CtaExtensionV3),
}

#[derive(Debug)]
pub struct CtaExtensionV3 {
    pub data_blocks: Vec<CtaDataBlock>,
}

#[derive(Debug)]
pub enum CtaDataBlock {
    Unknown,
    VendorAmd(CtaAmdVendorDataBlock),
    Colorimetry(#[expect(dead_code)] CtaColorimetryDataBlock),
    StaticHdrMetadata(#[expect(dead_code)] CtaStaticHdrMetadataDataBlock),
}

#[derive(Debug)]
pub struct CtaAmdVendorDataBlock {
    pub minimum_refresh_hz: u8,
    #[expect(dead_code)]
    pub maximum_refresh_hz: u8,
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub struct CtaColorimetryDataBlock {
    pub bt2020_rgb: bool,
    pub bt2020_ycc: bool,
    pub bt2020_cycc: bool,
    pub op_rgb: bool,
    pub op_ycc_601601: bool,
    pub s_ycc_601: bool,
    pub xv_ycc_709: bool,
    pub xv_ycc_601: bool,
    pub dci_p3: bool,
}

#[derive(Copy, Clone, Debug)]
#[expect(dead_code)]
pub struct CtaStaticHdrMetadataDataBlock {
    pub traditional_gamma_sdr_luminance: bool,
    pub traditional_gamma_hdr_luminance: bool,
    pub smpte_st_2084: bool,
    pub hlg: bool,
    pub max_luminance: Option<f64>,
    pub max_frame_average_luminance: Option<f64>,
    pub min_luminance: Option<f64>,
}

#[derive(Debug)]
pub struct EdidFile {
    pub base_block: EdidBaseBlock,
    pub extension_blocks: Vec<EdidExtension>,
}

#[derive(Debug, Error)]
pub enum EdidError {
    #[error("Unexpected end-of-file")]
    UnexpectedEof,
    #[error("Invalid magic header")]
    InvalidMagic(BString),
}

pub fn parse(data: &[u8]) -> Result<EdidFile, EdidError> {
    let mut parser = EdidParser {
        data,
        pos: 0,
        context: Rc::new(Default::default()),
        saved_ctx: vec![],
        errors: vec![],
    };
    parser.parse()
}

const CP437: &[&str] = &[
    "\u{0}", "☺", "☻", "♥", "♦", "♣", "♠", "•", "◘", "○", "◙", "♂", "♀", "♪", "♫", "☼", "►", "◄",
    "↕", "‼", "¶", "§", "▬", "↨", "↑", "↓", "→", "←", "∟", "↔", "▲", "▼", " ", "!", "\"", "#", "$",
    "%", "&", "'", "(", ")", "*", "+", ",", "-", ".", "/", "0", "1", "2", "3", "4", "5", "6", "7",
    "8", "9", ":", ";", "<", "=", ">", "?", "@", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J",
    "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "[", "\\", "]",
    "^", "_", "`", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p",
    "q", "r", "s", "t", "u", "v", "w", "x", "y", "z", "{", "|", "}", "~", "⌂", "Ç", "ü", "é", "â",
    "ä", "à", "å", "ç", "ê", "ë", "è", "ï", "î", "ì", "Ä", "Å", "É", "æ", "Æ", "ô", "ö", "ò", "û",
    "ù", "ÿ", "Ö", "Ü", "¢", "£", "¥", "₧", "ƒ", "á", "í", "ó", "ú", "ñ", "Ñ", "ª", "º", "¿", "⌐",
    "¬", "½", "¼", "¡", "«", "»", "░", "▒", "▓", "│", "┤", "╡", "╢", "╖", "╕", "╣", "║", "╗", "╝",
    "╜", "╛", "┐", "└", "┴", "┬", "├", "─", "┼", "╞", "╟", "╚", "╔", "╩", "╦", "╠", "═", "╬", "╧",
    "╨", "╤", "╥", "╙", "╘", "╒", "╓", "╫", "╪", "┘", "┌", "█", "▄", "▌", "▐", "▀", "α", "ß", "Γ",
    "π", "Σ", "σ", "µ", "τ", "Φ", "Θ", "Ω", "δ", "∞", "φ", "ε", "∩", "≡", "±", "≥", "≤", "⌠", "⌡",
    "÷", "≈", "°", "∙", "·", "√", "ⁿ", "²", "■", "\u{a0}",
];
