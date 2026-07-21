use crate::backend::BackendGammaLut;
use crate::backend::BackendGammaLutId;
use crate::backends::metal::video::metal_cm::BlobRegistryKey;
use crate::backends::metal::video::metal_cm::CrtcColorOpProgramming;
use crate::backends::metal::video::metal_cm::CrtcColorOpProps;
use crate::backends::metal::video::metal_cm::MIN_LUT_SIZE;
use crate::backends::metal::video::metal_cm::Shared;
use crate::backends::metal::video::metal_cm::dst_lut_out_scale;
use crate::backends::metal::video::metal_cm::metal_cm_crtc::ColorOp;
use crate::backends::metal::video::metal_cm::metal_cm_crtc::ColorOpType;
use crate::backends::metal::video::metal_cm::metal_cm_crtc::ColorPipeline;
use crate::backends::metal::video::metal_cm::metal_cm_crtc::CrtcColorPipelines;
use crate::backends::metal::video::metal_cm::metal_cm_lut::CurveConfig;
use crate::backends::metal::video::metal_cm::metal_cm_lut::LutConfig;
use crate::backends::metal::video::metal_cm::metal_cm_lut::create_lut;
use crate::backends::metal::video::metal_cm::metal_cm_lut::create_lut_curve;
use crate::backends::metal::video::metal_cm::metal_cm_paths::Criteria;
use crate::backends::metal::video::metal_cm::metal_cm_paths::CrtcOpKind;
use crate::backends::metal::video::metal_cm::metal_cm_paths::LutEotf;
use crate::backends::metal::video::metal_cm::metal_cm_paths::MAX_CRTC_PATH_LEN;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PlaneOpTarget::Dst;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PlaneOpTarget::Src;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PlaneOpTarget::{self};
use crate::cmm::cmm_description::ColorDescription;
use crate::cmm::cmm_description::ColorDescriptionId;
use crate::utils::bool_ext::BoolExt;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::numcell::NumCell;
use crate::utils::obj_and_id::ObjWithId;
use crate::video::drm::DrmBlob;
use crate::video::drm::DrmCrtc;
use crate::video::drm::DrmMaster;
use crate::video::drm::DrmObject;
use crate::video::drm::DrmProperty;
use crate::video::drm::DrmPropertyValue;
use jay_proc::jay_hash;
use linearize::Linearized;
use linearize::StaticCopyMap;
use log::Level;
use log::log_enabled;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Default)]
pub struct CrtcMatcherCache {
    partials: Cell<Vec<Partial>>,
    serials: NumCell<u64>,
}

#[derive(Copy, Clone, Debug)]
struct ColorOpMatch {
    crtc: DrmCrtc,
    ty: ColorOpMatchType,
}

#[derive(Copy, Clone, Debug)]
enum ColorOpMatchType {
    Bypass {
        gamma_lut: DrmProperty,
    },
    GammaLut {
        gamma_lut: DrmProperty,
        eotf: Option<CurveConfig>,
        size: usize,
        input_is_scaled_for_dst_lut: bool,
    },
}

#[derive(Clone, Default)]
struct Partial {
    serial: u64,
    op: Option<ColorOpMatch>,
}

pub struct CrtcColorPipelineMatch {
    matches: Vec<ColorOpMatch>,
}

#[jay_hash]
#[derive(Copy, Clone, Debug)]
pub struct GammaLutKey {
    dst: Option<ColorDescriptionId>,
    eotf: Option<CurveConfig>,
    size: usize,
    input_is_scaled_for_dst_lut: bool,
    client_gamma_lut: Option<BackendGammaLutId>,
}

impl CrtcColorPipelines {
    pub fn find_match(
        &self,
        shared: &Shared,
        descriptions: StaticCopyMap<PlaneOpTarget, &Rc<ColorDescription>>,
        criteria: Linearized<Criteria>,
        path_idx: usize,
        path: &[CrtcOpKind],
    ) -> Option<CrtcColorPipelineMatch> {
        for pipeline in &self.pipelines {
            let Some(matches) = pipeline.find_match(shared, descriptions, criteria, path_idx, path)
            else {
                continue;
            };
            return Some(CrtcColorPipelineMatch { matches });
        }
        None
    }
}

const PARTIALS_ROW_LENGTH: usize = MAX_CRTC_PATH_LEN + 1;

impl ColorPipeline {
    fn find_match(
        &self,
        shared: &Shared,
        descriptions: StaticCopyMap<PlaneOpTarget, &Rc<ColorDescription>>,
        criteria: Linearized<Criteria>,
        path_idx: usize,
        path: &[CrtcOpKind],
    ) -> Option<Vec<ColorOpMatch>> {
        if !self.filter.contains(criteria, path_idx) {
            return None;
        }
        let cache = &shared.crtc_matcher_cache;
        let mut partials = cache.partials.take();
        let partials_len = self.ops.len() * PARTIALS_ROW_LENGTH;
        if partials.len() < partials_len {
            partials.resize(partials_len, Partial::default());
        }
        let mut matcher = Matcher {
            crtc: self.crtc,
            ops: &self.ops,
            serial: cache.serials.add_fetch(1),
            partials: &mut partials,
            path,
            descriptions,
        };
        let mut matches = Vec::new();
        let ok = matcher.match_at(0, 0);
        if ok {
            let mut j = 0;
            for i in 0..self.ops.len() {
                let linear_idx = i * PARTIALS_ROW_LENGTH + j;
                let partial = &matcher.partials[linear_idx];
                if let Some(op) = partial.op {
                    if not_matches!(op.ty, ColorOpMatchType::Bypass { .. }) {
                        j += 1;
                    }
                    matches.push(op);
                }
            }
            debug_assert_eq!(j, path.len());
        }
        cache.partials.set(partials);
        ok.then_some(matches)
    }
}

impl CrtcColorPipelineMatch {
    pub fn log(&self) {
        let level = Level::Debug;
        if !log_enabled!(level) {
            return;
        }
        log::log!(level, "programming:");
        for m in &self.matches {
            log::log!(level, "  - crtc: {:?}", m.crtc);
            match &m.ty {
                ColorOpMatchType::Bypass { .. } => {
                    log::log!(level, "    bypass");
                }
                ColorOpMatchType::GammaLut {
                    eotf,
                    size,
                    input_is_scaled_for_dst_lut,
                    ..
                } => {
                    log::log!(level, "    type: GammaLut");
                    log::log!(level, "    eotf: {eotf:?}");
                    log::log!(level, "    size: {size:?}");
                    log::log!(
                        level,
                        "    input_is_scaled_for_dst_lut: {input_is_scaled_for_dst_lut:?}"
                    );
                }
            }
        }
    }

    pub fn compute_programming(
        self,
        master: &Rc<DrmMaster>,
        shared: &Shared,
        dst: &Rc<ColorDescription>,
        client_gamma_lut: Option<&Rc<BackendGammaLut>>,
    ) -> Option<Vec<CrtcColorOpProgramming>> {
        macro_rules! create_blob {
            ($v:expr) => {
                match master.create_blob($v) {
                    Ok(b) => b,
                    Err(e) => {
                        log::error!("Could not create blob: {}", ErrorFmt(e));
                        return None;
                    }
                }
            };
        }
        let mut res = vec![];
        for m in self.matches {
            let p = match m.ty {
                ColorOpMatchType::Bypass { gamma_lut } => CrtcColorOpProgramming {
                    id: m.crtc,
                    props: CrtcColorOpProps {
                        gamma_lut: Some(DrmPropertyValue {
                            id: gamma_lut,
                            value: DrmBlob::NONE,
                        }),
                    },
                    ..Default::default()
                },
                ColorOpMatchType::GammaLut {
                    gamma_lut,
                    eotf,
                    size,
                    input_is_scaled_for_dst_lut,
                } => {
                    let key = GammaLutKey {
                        dst: input_is_scaled_for_dst_lut.then_some(dst.id),
                        eotf,
                        size,
                        input_is_scaled_for_dst_lut,
                        client_gamma_lut: client_gamma_lut.id(),
                    };
                    let blob_key = BlobRegistryKey::GammaLut(key);
                    let blob = match shared.blob_registry.get(&blob_key) {
                        Some(b) => b,
                        None => {
                            let dst_lut_scale = || dst_lut_out_scale(dst).map(|v| v as f32);
                            let lut_obj;
                            let lut = if let Some(eotf) = key.eotf {
                                let curve = create_lut_curve(
                                    eotf,
                                    key.size,
                                    key.input_is_scaled_for_dst_lut
                                        .and_then(dst_lut_scale)
                                        .map(|v| 1.0 / v),
                                    None,
                                );
                                lut_obj = create_lut::<u16>(
                                    LutConfig::Curve(&curve),
                                    client_gamma_lut,
                                    None,
                                );
                                &*lut_obj
                            } else if let Some(gamma_lut) = client_gamma_lut
                                && gamma_lut.gamma_lut.len() == key.size
                            {
                                &*gamma_lut.gamma_lut
                            } else {
                                lut_obj = create_lut::<u16>(
                                    LutConfig::Size(key.size),
                                    client_gamma_lut,
                                    None,
                                );
                                &*lut_obj
                            };
                            let blob = create_blob!(lut);
                            shared.blob_registry.insert(blob_key, blob)
                        }
                    };
                    CrtcColorOpProgramming {
                        id: m.crtc,
                        props: CrtcColorOpProps {
                            gamma_lut: Some(DrmPropertyValue {
                                id: gamma_lut,
                                value: blob.id(),
                            }),
                        },
                        _gamma_lut_blob: Some(blob),
                    }
                }
            };
            res.push(p);
        }
        Some(res)
    }
}

struct Matcher<'a, 'b, 'c> {
    crtc: DrmCrtc,
    ops: &'a [Rc<ColorOp>],
    serial: u64,
    partials: &'b mut [Partial],
    path: &'a [CrtcOpKind],
    descriptions: StaticCopyMap<PlaneOpTarget, &'c Rc<ColorDescription>>,
}

impl Matcher<'_, '_, '_> {
    fn match_at(&mut self, i: usize, j: usize) -> bool {
        if i == self.ops.len() {
            return j == self.path.len();
        }
        let linear_idx = i * PARTIALS_ROW_LENGTH + j;
        let partial = &self.partials[linear_idx];
        if partial.serial == self.serial {
            return partial.op.is_some();
        }
        let res = self.match_at_uncached(i, j);
        let partial = &mut self.partials[linear_idx];
        partial.serial = self.serial;
        partial.op = res.map(|ty| ColorOpMatch {
            crtc: self.crtc,
            ty,
        });
        partial.op.is_some()
    }

    fn match_at_uncached(&mut self, i: usize, j: usize) -> Option<ColorOpMatchType> {
        let op = &self.ops[i];
        if j < self.path.len() {
            let ty = match (&op.ty, self.path[j]) {
                (
                    ColorOpType::GammaLut(l),
                    CrtcOpKind::GammaLut {
                        eotf,
                        input_is_scaled_for_dst_lut,
                    },
                ) if l.size >= MIN_LUT_SIZE || eotf.is_none() => {
                    let src = self.descriptions[Src].eotf;
                    let dst = self.descriptions[Dst].eotf;
                    let eotf = eotf.map(|eotf| match eotf {
                        LutEotf::Eotf => CurveConfig::Eotf(src),
                        LutEotf::InvEotf => CurveConfig::InvEotf(dst),
                        LutEotf::Both => CurveConfig::Both(src, dst),
                    });
                    let ty = ColorOpMatchType::GammaLut {
                        gamma_lut: l.gamma_lut,
                        eotf,
                        size: l.size,
                        input_is_scaled_for_dst_lut,
                    };
                    Some(ty)
                }
                _ => None,
            };
            if ty.is_some() && self.match_at(i + 1, j + 1) {
                return ty;
            }
        }
        let ok = self.match_at(i + 1, j);
        ok.then(|| match &op.ty {
            ColorOpType::GammaLut(l) => ColorOpMatchType::Bypass {
                gamma_lut: l.gamma_lut,
            },
        })
    }
}
