use crate::backends::metal::video::CollectedProperties;
use crate::backends::metal::video::metal_cm::metal_cm_paths::CRTC_MATCHERS;
use crate::backends::metal::video::metal_cm::metal_cm_paths::CrtcOpName;
use crate::backends::metal::video::metal_cm::metal_cm_paths::Filter;
use crate::backends::metal::video::metal_cm::metal_cm_paths::create_filter;
use crate::video::drm::DrmBlob;
use crate::video::drm::DrmCrtc;
use crate::video::drm::DrmProperty;
use std::fmt::Debug;
use std::rc::Rc;

pub mod metal_cm_crtc_matcher;

#[derive(Debug)]
pub struct CrtcColorPipelines {
    pipelines: Vec<Rc<ColorPipeline>>,
    pub filter: Filter,
}

#[derive(Debug)]
struct ColorPipeline {
    crtc: DrmCrtc,
    ops: Vec<Rc<ColorOp>>,
    filter: Filter,
}

#[derive(Debug)]
struct ColorOp {
    ty: ColorOpType,
}

#[derive(Debug)]
enum ColorOpType {
    GammaLut(ColorOpGammaLut),
}

#[derive(Debug)]
struct ColorOpGammaLut {
    size: usize,
    gamma_lut: DrmProperty,
}

impl CrtcColorPipelines {
    pub(super) fn new(crtc: DrmCrtc, props: &CollectedProperties) -> Self {
        parse_color_pipelines(crtc, props)
    }
}

pub(super) fn parse_color_pipelines(
    crtc: DrmCrtc,
    props: &CollectedProperties,
) -> CrtcColorPipelines {
    let gamma_lut = props
        .get("GAMMA_LUT")
        .ok()
        .map(|v| v.map(|v| DrmBlob(v as u32)));
    let mut size = None;
    if gamma_lut.is_some() {
        size = props.get("GAMMA_LUT_SIZE").ok().map(|v| v.value as usize);
    }
    let mut ops = vec![];
    if let Some(gamma_lut) = gamma_lut
        && let Some(size) = size
    {
        let op = Rc::new(ColorOp {
            ty: ColorOpType::GammaLut(ColorOpGammaLut {
                size,
                gamma_lut: gamma_lut.id,
            }),
        });
        ops.push(op);
    }
    let names: Vec<_> = ops
        .iter()
        .map(|o| {
            let name = match o.ty {
                ColorOpType::GammaLut(_) => CrtcOpName::GammaLut,
            };
            (name, true)
        })
        .collect();
    let mut filter = Filter::default();
    let pipeline = Rc::new(ColorPipeline {
        crtc,
        ops,
        filter: create_filter(&mut filter, &names, &CRTC_MATCHERS),
    });
    let pipelines = vec![pipeline];
    CrtcColorPipelines { pipelines, filter }
}
