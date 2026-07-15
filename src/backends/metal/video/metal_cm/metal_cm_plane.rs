use crate::backends::metal::video::CollectedProperties;
use crate::backends::metal::video::MetalDrmVendor;
use crate::backends::metal::video::collect_properties;
use crate::backends::metal::video::metal_cm::MIN_LUT_SIZE;
use crate::backends::metal::video::metal_cm::metal_cm_paths::Filter;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PLANE_MATCHERS;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PlaneOpName;
use crate::backends::metal::video::metal_cm::metal_cm_paths::create_filter;
use crate::env::JAY_MCM_AMD_USE_FIRST_LUT;
use crate::env::JAY_MCM_NVIDIA_USE_LUTS;
use crate::utils::errorfmt::ErrorFmt;
use crate::video::drm::DrmColorop;
use crate::video::drm::DrmError;
use crate::video::drm::DrmMaster;
use crate::video::drm::DrmPlane;
use crate::video::drm::DrmProperty;
use crate::video::drm::DrmPropertyType;
use bstr::BStr;
use bstr::BString;
use bstr::ByteSlice;
use linearize::Linearize;
use linearize::StaticCopyMap;
use linearize::StaticMap;
use std::fmt::Debug;
use std::mem;
use std::rc::Rc;
use thiserror::Error;

pub mod metal_cm_plane_matcher;

#[derive(Debug)]
pub struct PlaneColorPipelines {
    pub plane: DrmPlane,
    pub prop: Option<DrmProperty>,
    pub pipelines: Vec<ColorPipeline>,
    pub filter: Filter,
    pub supported_curves: StaticCopyMap<ColorOpCurve1dType, bool>,
}

#[derive(Debug)]
pub struct ColorPipeline {
    id: DrmColorop,
    ops: Vec<Rc<ColorOp>>,
    filter: Filter,
}

#[derive(Debug)]
struct ColorOp {
    op: DrmColorop,
    bypass: Option<DrmProperty>,
    next_non_bypass: Option<usize>,
    ty: ColorOpType,
}

#[derive(Debug)]
enum ColorOpType {
    Curve1d(ColorOpCurve1d),
    Lut1d(ColorOpLut1d),
    Lut1dToSmall,
    Lut1dLowPrecision,
    Matrix3x4(ColorOpMatrix3x4),
    Multiplier(ColorOpMultiplier),
    Lut3d(ColorOpLut3d),
    Unknown(#[expect(dead_code)] Option<BString>),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Linearize)]
pub enum ColorOpCurve1dType {
    SrgbEotf,
    SrgbInverseEotf,
    Pq125Eotf,
    Pq125InverseEotf,
    Bt2020InverseOetf,
    Bt2020Oetf,
    Gamma22,
    Gamma22Inverse,
}

#[derive(Debug)]
struct ColorOpCurve1d {
    types: StaticMap<ColorOpCurve1dType, Option<(u64, &'static str)>>,
    ty: DrmProperty,
}

#[derive(Debug, Linearize)]
enum ColorOpLut1dInterpolationType {
    Linear,
}

#[derive(Debug)]
struct ColorOpLut1d {
    interpolations: StaticMap<ColorOpLut1dInterpolationType, Option<u64>>,
    interpolation: DrmProperty,
    size: usize,
    data: DrmProperty,
}

#[derive(Debug)]
struct ColorOpMatrix3x4 {
    // [u64; 12]
    // s31.32
    // |  0,  1,  2,  3 |
    // |  4,  5,  6,  7 |
    // |  8,  9, 10, 11 |
    data: DrmProperty,
}

#[derive(Debug)]
struct ColorOpMultiplier {
    // u64
    // s31.32
    multiplier: DrmProperty,
}

#[derive(Debug, Linearize)]
enum ColorOpLut3dInterpolationType {
    Tetrahedral,
}

#[derive(Debug)]
struct ColorOpLut3d {
    interpolations: StaticMap<ColorOpLut3dInterpolationType, Option<u64>>,
    interpolation: DrmProperty,
    size: usize,
    data: DrmProperty,
}

impl PlaneColorPipelines {
    pub(super) fn new(
        master: &Rc<DrmMaster>,
        vendor: &MetalDrmVendor,
        plane: DrmPlane,
        props: &CollectedProperties,
    ) -> Self {
        parse_color_pipelines(master, vendor, plane, props)
    }
}

fn to_s3132(v: f64) -> u64 {
    let mul = 2.0f64.powf(32.0);
    let v = (v * mul) as i64;
    v.unsigned_abs() | ((v < 0) as i64 * i64::MIN) as u64
}

pub(super) fn parse_color_pipelines(
    master: &Rc<DrmMaster>,
    vendor: &MetalDrmVendor,
    plane: DrmPlane,
    props: &CollectedProperties,
) -> PlaneColorPipelines {
    let mut res = PlaneColorPipelines {
        plane,
        prop: Default::default(),
        pipelines: Default::default(),
        filter: Default::default(),
        supported_curves: Default::default(),
    };
    let Some((d, _)) = props.props.get(b"COLOR_PIPELINE".as_bstr()) else {
        return res;
    };
    let DrmPropertyType::Enum { values, .. } = &d.ty else {
        return res;
    };
    res.prop = Some(d.id);
    for v in values {
        if v.value == 0 {
            continue;
        }
        if let Err(e) = parse_color_pipeline(&mut res, master, vendor, v.value) {
            log::warn!("Could not parse color pipeline: {}", ErrorFmt(e));
        }
    }
    res
}

#[derive(Debug, Error)]
enum ColorPipelineError {
    #[error(transparent)]
    DrmError(#[from] DrmError),
    #[error("ColorOp has no NEXT property")]
    NoNext,
    #[error("ColorOp has no TYPE property")]
    NoType,
    #[error("ColorOp has the unknown type {0} and cannot be bypassed")]
    UnknownType(BString),
    #[error("ColorOp has type {0} but no {1} property")]
    MissingProperty(&'static BStr, &'static str),
    #[error("ColorOp has type {0} but the {1} property is not an enum")]
    PropertyNotEnum(&'static BStr, &'static str),
}

fn parse_color_pipeline(
    res: &mut PlaneColorPipelines,
    master: &Rc<DrmMaster>,
    vendor: &MetalDrmVendor,
    root: u64,
) -> Result<(), ColorPipelineError> {
    let mut next = root;
    let id = DrmColorop(root as u32);
    let mut ops = vec![];
    let mut first_lut = true;
    while next != 0 {
        let op = DrmColorop(next as u32);
        let props = collect_properties(master, op)?;
        let Some((_, n)) = props.props.get(b"NEXT".as_bstr()) else {
            return Err(ColorPipelineError::NoNext);
        };
        next = *n;
        let mut bypass = None;
        if let Some((def, _)) = props.props.get(b"BYPASS".as_bstr())
            && let DrmPropertyType::Range { min, max } = &def.ty
            && *min == 0
            && *max > 0
        {
            bypass = Some(def.id);
        }
        macro_rules! no_type {
            () => {
                if bypass.is_some() {
                    log::warn!("Color op {op:?} has no type");
                    ops.push(ColorOp {
                        op,
                        bypass,
                        ty: ColorOpType::Unknown(None),
                        next_non_bypass: Default::default(),
                    });
                    continue;
                } else {
                    return Err(ColorPipelineError::NoType);
                }
            };
        }
        let Some((ty_def, ty)) = props.props.get(b"TYPE".as_bstr()) else {
            no_type!();
        };
        let DrmPropertyType::Enum { values, .. } = &ty_def.ty else {
            no_type!();
        };
        let Some(v) = values.iter().find(|v| v.value == *ty) else {
            no_type!();
        };
        let ty = match parse_color_op_type(res, vendor, &props, &v.name, &mut first_lut) {
            Ok(ty) => ty,
            Err(e) if bypass.is_some() => {
                log::warn!("Could not parse color op type: {}", ErrorFmt(e));
                ColorOpType::Unknown(Some(v.name.clone()))
            }
            Err(e) => return Err(e),
        };
        ops.push(ColorOp {
            op,
            bypass,
            next_non_bypass: Default::default(),
            ty,
        });
    }
    let mut next_non_bypass = None;
    for (idx, op) in ops.iter_mut().enumerate().rev() {
        op.next_non_bypass = next_non_bypass;
        if op.bypass.is_none() {
            next_non_bypass = Some(idx);
        }
    }
    let names: Vec<_> = ops
        .iter()
        .map(|o| {
            let name = match o.ty {
                ColorOpType::Curve1d(_) => PlaneOpName::Curve1d,
                ColorOpType::Lut1d(_) => PlaneOpName::Lut1d,
                ColorOpType::Matrix3x4(_) => PlaneOpName::Matrix3x4,
                ColorOpType::Multiplier(_) => PlaneOpName::Multiply,
                ColorOpType::Lut3d(_) => PlaneOpName::Lut3d,
                ColorOpType::Unknown(_) => PlaneOpName::Other,
                ColorOpType::Lut1dToSmall => PlaneOpName::Other,
                ColorOpType::Lut1dLowPrecision => PlaneOpName::Other,
            };
            (name, o.bypass.is_some())
        })
        .collect();
    let filter = create_filter(&mut res.filter, &names, &PLANE_MATCHERS);
    let ops = ops.into_iter().map(Rc::new).collect();
    res.pipelines.push(ColorPipeline { id, ops, filter });
    Ok(())
}

const CURVE_1D: &[u8] = b"1D Curve";
const LUT_1D: &[u8] = b"1D LUT";
const MATRIX_3X4: &[u8] = b"3x4 Matrix";
const MULTIPLIER: &[u8] = b"Multiplier";
const LUT_3D: &[u8] = b"3D LUT";

fn parse_color_op_type(
    res: &mut PlaneColorPipelines,
    vendor: &MetalDrmVendor,
    props: &CollectedProperties,
    ty: &[u8],
    first_lut: &mut bool,
) -> Result<ColorOpType, ColorPipelineError> {
    let get_prop = |ty: &'static [u8], name: &'static str| {
        props
            .props
            .get(name.as_bytes().as_bstr())
            .ok_or(ColorPipelineError::MissingProperty(ty.as_bstr(), name))
    };
    let get_enum = |ty: &'static [u8], name: &'static str| {
        let (def, v) = get_prop(ty, name)?;
        let DrmPropertyType::Enum { values, .. } = &def.ty else {
            return Err(ColorPipelineError::PropertyNotEnum(ty.as_bstr(), name));
        };
        Ok((def, v, values))
    };
    let ty = match ty {
        CURVE_1D => {
            let (def, _, values) = get_enum(CURVE_1D, "CURVE_1D_TYPE")?;
            let mut types = StaticMap::default();
            'outer: for v in values {
                use ColorOpCurve1dType::*;
                let pairs = [
                    ("sRGB EOTF", SrgbEotf),
                    ("sRGB Inverse EOTF", SrgbInverseEotf),
                    ("PQ 125 EOTF", Pq125Eotf),
                    ("PQ 125 Inverse EOTF", Pq125InverseEotf),
                    ("BT.2020 Inverse OETF", Bt2020InverseOetf),
                    ("BT.2020 OETF", Bt2020Oetf),
                    ("Gamma 2.2", Gamma22),
                    ("Gamma 2.2 Inverse", Gamma22Inverse),
                ];
                let (name, ty) = 'ty: {
                    for ty @ (name, _) in pairs {
                        if v.name.as_slice() == name.as_bytes() {
                            break 'ty ty;
                        }
                    }
                    continue 'outer;
                };
                res.supported_curves[ty] = true;
                types[ty] = Some((v.value, name));
            }
            ColorOpType::Curve1d(ColorOpCurve1d { types, ty: def.id })
        }
        LUT_1D => {
            let first_lut = mem::take(first_lut);
            if vendor.is_amd && first_lut && !*JAY_MCM_AMD_USE_FIRST_LUT {
                return Ok(ColorOpType::Lut1dLowPrecision);
            }
            if vendor.is_nvidia && !*JAY_MCM_NVIDIA_USE_LUTS {
                return Ok(ColorOpType::Lut1dLowPrecision);
            }
            let (def, _, values) = get_enum(LUT_1D, "LUT1D_INTERPOLATION")?;
            let mut interpolations = StaticMap::default();
            for v in values {
                use ColorOpLut1dInterpolationType::*;
                let ty = match v.name.as_slice() {
                    b"Linear" => Linear,
                    _ => continue,
                };
                interpolations[ty] = Some(v.value);
            }
            let interpolation = def.id;
            let &(_, size) = get_prop(LUT_1D, "SIZE")?;
            let size = size as usize;
            let (def, _) = get_prop(LUT_1D, "DATA")?;
            let data = def.id;
            if size < MIN_LUT_SIZE {
                return Ok(ColorOpType::Lut1dToSmall);
            }
            ColorOpType::Lut1d(ColorOpLut1d {
                interpolations,
                interpolation,
                size,
                data,
            })
        }
        MATRIX_3X4 => {
            let (def, _) = get_prop(MATRIX_3X4, "DATA")?;
            ColorOpType::Matrix3x4(ColorOpMatrix3x4 { data: def.id })
        }
        MULTIPLIER => {
            let (def, _) = get_prop(MULTIPLIER, "MULTIPLIER")?;
            ColorOpType::Multiplier(ColorOpMultiplier { multiplier: def.id })
        }
        LUT_3D => {
            let (def, _, values) = get_enum(LUT_3D, "LUT3D_INTERPOLATION")?;
            let mut interpolations = StaticMap::default();
            for v in values {
                use ColorOpLut3dInterpolationType::*;
                let ty = match v.name.as_slice() {
                    b"Tetrahedral" => Tetrahedral,
                    _ => continue,
                };
                interpolations[ty] = Some(v.value);
            }
            let interpolation = def.id;
            let (_, size) = get_prop(LUT_3D, "SIZE")?;
            let (def, _) = get_prop(LUT_3D, "DATA")?;
            let data = def.id;
            ColorOpType::Lut3d(ColorOpLut3d {
                interpolations,
                interpolation,
                size: *size as usize,
                data,
            })
        }
        _ => {
            return Err(ColorPipelineError::UnknownType(ty.to_vec().into()));
        }
    };
    Ok(ty)
}
