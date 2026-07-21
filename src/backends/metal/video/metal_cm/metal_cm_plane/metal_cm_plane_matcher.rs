use crate::backends::metal::video::metal_cm::BlobRegistryKey;
use crate::backends::metal::video::metal_cm::PlaneColorOpProgramming;
use crate::backends::metal::video::metal_cm::PlaneColorOpProps;
use crate::backends::metal::video::metal_cm::PlaneProgramming;
use crate::backends::metal::video::metal_cm::PlaneProps;
use crate::backends::metal::video::metal_cm::Shared;
use crate::backends::metal::video::metal_cm::dst_lut_out_scale;
use crate::backends::metal::video::metal_cm::metal_cm_lut::CurveConfig;
use crate::backends::metal::video::metal_cm::metal_cm_lut::LutConfig;
use crate::backends::metal::video::metal_cm::metal_cm_lut::create_lut;
use crate::backends::metal::video::metal_cm::metal_cm_lut::create_lut_curve;
use crate::backends::metal::video::metal_cm::metal_cm_paths::Criteria;
use crate::backends::metal::video::metal_cm::metal_cm_paths::LutEotf;
use crate::backends::metal::video::metal_cm::metal_cm_paths::MAX_PLANE_PATH_LEN;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PlaneOpKind;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PlaneOpTarget::Dst;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PlaneOpTarget::Src;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PlaneOpTarget::{self};
use crate::backends::metal::video::metal_cm::metal_cm_plane::ColorOp;
use crate::backends::metal::video::metal_cm::metal_cm_plane::ColorOpCurve1dType;
use crate::backends::metal::video::metal_cm::metal_cm_plane::ColorOpLut1dInterpolationType;
use crate::backends::metal::video::metal_cm::metal_cm_plane::ColorOpLut3dInterpolationType;
use crate::backends::metal::video::metal_cm::metal_cm_plane::ColorOpType;
use crate::backends::metal::video::metal_cm::metal_cm_plane::ColorPipeline;
use crate::backends::metal::video::metal_cm::metal_cm_plane::PlaneColorPipelines;
use crate::backends::metal::video::metal_cm::metal_cm_plane::to_s3132;
use crate::cmm::cmm_description::ColorDescription;
use crate::cmm::cmm_description::ColorDescriptionId;
use crate::cmm::cmm_transform::ColorMatrix;
use crate::cmm::cmm_transform::format_matrix;
use crate::utils::bool_ext::BoolExt;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::numcell::NumCell;
use crate::utils::obj_and_id::ObjWithId;
use crate::video::drm::DrmColorop;
use crate::video::drm::DrmMaster;
use crate::video::drm::DrmObject;
use crate::video::drm::DrmPlane;
use crate::video::drm::DrmProperty;
use crate::video::drm::DrmPropertyValue;
use jay_algorithms::lut::fill_lut_3d;
use jay_proc::jay_hash;
use linearize::Linearized;
use linearize::StaticCopyMap;
use log::Level;
use log::log_enabled;
use std::cell::Cell;
use std::ops::RangeTo;
use std::rc::Rc;

#[derive(Default)]
pub struct PlaneMatcherCache {
    partials: Cell<Vec<Partial>>,
    serials: NumCell<u64>,
}

#[derive(Copy, Clone, Debug, Default)]
struct ColorOpMatch {
    id: DrmColorop,
    bypass: Option<DrmPropertyValue>,
    ty: ColorOpMatchType,
}

#[derive(Copy, Clone, Debug, Default)]
enum ColorOpMatchType {
    #[default]
    Bypass,
    Curve1d {
        ty: DrmPropertyValue,
        name: &'static str,
    },
    Lut1d {
        interpolation: DrmPropertyValue,
        blob_id: DrmProperty,
        eotf: CurveConfig,
        size: usize,
        scale_for_dst_lut: bool,
        input_is_scaled_for_dst_lut: bool,
    },
    Lut3d {
        interpolation: DrmPropertyValue,
        blob_id: DrmProperty,
        matrix: ColorMatrix,
        size: usize,
    },
    Matrix3x4 {
        blob_id: DrmProperty,
        matrix: ColorMatrix,
        factor_before: Option<f64>,
        factor_after: Option<f64>,
    },
    Multiply {
        multiplier_id: DrmProperty,
        multiplier: f64,
    },
}

#[derive(Clone, Default)]
struct Partial {
    next_i: usize,
    serial: u64,
    ok: bool,
    op: ColorOpMatch,
}

pub struct PlaneColorPipelineMatch {
    plane: DrmPlane,
    pipeline: Option<DrmPropertyValue<DrmColorop>>,
    matches: Vec<ColorOpMatch>,
}

#[jay_hash]
#[derive(Copy, Clone, Debug)]
pub struct MatrixKey {
    matrix: [u64; 12],
}

#[jay_hash]
#[derive(Copy, Clone, Debug)]
pub struct Lut1dKey {
    dst: ColorDescriptionId,
    eotf: CurveConfig,
    size: usize,
    scale_for_dst_lut: bool,
    input_is_scaled_for_dst_lut: bool,
}

#[jay_hash]
#[derive(Copy, Clone, Debug)]
pub struct Lut3dKey {
    matrix: ColorMatrix,
    size: usize,
}

impl PlaneColorPipelines {
    pub fn find_match(
        &self,
        shared: &Shared,
        descriptions: StaticCopyMap<PlaneOpTarget, &Rc<ColorDescription>>,
        curve_type: StaticCopyMap<PlaneOpTarget, Option<ColorOpCurve1dType>>,
        multiply: StaticCopyMap<PlaneOpTarget, f64>,
        matrix: ColorMatrix,
        criteria: Linearized<Criteria>,
        path_idx: usize,
        path: &[PlaneOpKind],
    ) -> Option<PlaneColorPipelineMatch> {
        if path.is_empty() {
            return Some(PlaneColorPipelineMatch {
                plane: self.plane,
                pipeline: self.prop.map(|id| DrmPropertyValue {
                    id,
                    value: DrmColorop::NONE,
                }),
                matches: Default::default(),
            });
        }
        let id = self.prop?;
        for pipeline in &self.pipelines {
            let Some(matches) = pipeline.find_match(
                shared,
                descriptions,
                curve_type,
                multiply,
                matrix,
                criteria,
                path_idx,
                path,
            ) else {
                continue;
            };
            return Some(PlaneColorPipelineMatch {
                plane: self.plane,
                pipeline: Some(DrmPropertyValue {
                    id,
                    value: pipeline.id,
                }),
                matches,
            });
        }
        None
    }
}

const PARTIALS_ROW_LENGTH: usize = MAX_PLANE_PATH_LEN + 1;

impl ColorPipeline {
    fn find_match(
        &self,
        shared: &Shared,
        descriptions: StaticCopyMap<PlaneOpTarget, &Rc<ColorDescription>>,
        curve_type: StaticCopyMap<PlaneOpTarget, Option<ColorOpCurve1dType>>,
        multiply: StaticCopyMap<PlaneOpTarget, f64>,
        matrix: ColorMatrix,
        criteria: Linearized<Criteria>,
        path_idx: usize,
        path: &[PlaneOpKind],
    ) -> Option<Vec<ColorOpMatch>> {
        if !self.filter.contains(criteria, path_idx) {
            return None;
        }
        let cache = &shared.plane_matcher_cache;
        let mut partials = cache.partials.take();
        let partials_len = self.ops.len() * PARTIALS_ROW_LENGTH;
        if partials.len() < partials_len {
            partials.resize(partials_len, Partial::default());
        }
        let mut matcher = Matcher {
            ops: &self.ops,
            serial: cache.serials.add_fetch(1),
            partials: &mut partials,
            matrix,
            path,
            descriptions,
            multiply,
            curve_type,
        };
        let mut matches = Vec::new();
        let ok = matcher.match_at(0, 0);
        if ok {
            let mut prev_i = 0;
            let mut i = 0;
            let mut j = 0;
            while i < self.ops.len() {
                for x in prev_i + 1..i {
                    let op = &self.ops[x];
                    matches.push(ColorOpMatch {
                        id: op.op,
                        bypass: op.bypass.map(|id| DrmPropertyValue { id, value: 1 }),
                        ty: ColorOpMatchType::Bypass,
                    });
                }
                let linear_idx = i * PARTIALS_ROW_LENGTH + j;
                let partial = &matcher.partials[linear_idx];
                let op = partial.op;
                if not_matches!(op.ty, ColorOpMatchType::Bypass) {
                    j += 1;
                }
                matches.push(op);
                prev_i = i;
                i = partial.next_i;
            }
            fixup_matches(&mut matches);
            debug_assert_eq!(j, path.len());
        }
        cache.partials.set(partials);
        ok.then_some(matches)
    }
}

fn fixup_matches(matches: &mut [ColorOpMatch]) {
    let mut last_mul = None;
    let mut next_mul = 1.0;
    for m in matches {
        match &mut m.ty {
            ColorOpMatchType::Multiply { multiplier, .. } => {
                *multiplier *= next_mul;
                next_mul = 1.0;
                last_mul = Some(multiplier);
            }
            ColorOpMatchType::Matrix3x4 {
                factor_before,
                factor_after,
                ..
            } => {
                if let Some(v) = factor_before.take()
                    && let Some(m) = last_mul.take()
                {
                    *m *= v;
                }
                if let Some(v) = factor_after.take() {
                    next_mul = v;
                }
            }
            _ => {}
        }
    }
}

impl PlaneColorPipelineMatch {
    pub fn log(&self) {
        let level = Level::Debug;
        if !log_enabled!(level) {
            return;
        }
        log::log!(level, "programming:");
        log::log!(level, "  plane: {:?}", self.plane);
        log::log!(level, "  pipeline: {:?}", self.pipeline.map(|p| p.value));
        for m in &self.matches {
            log::log!(level, "  - id: {:?}", m.id);
            match m.ty {
                ColorOpMatchType::Bypass => {
                    log::log!(level, "    bypass");
                }
                ColorOpMatchType::Curve1d { name, .. } => {
                    log::log!(level, "    type: Curve1d");
                    log::log!(level, "    curve: {name}");
                }
                ColorOpMatchType::Lut1d {
                    eotf,
                    size,
                    scale_for_dst_lut,
                    input_is_scaled_for_dst_lut,
                    ..
                } => {
                    log::log!(level, "    type: Lut1d");
                    let (src, dst) = match eotf {
                        CurveConfig::Eotf(e) => (Some(e), None),
                        CurveConfig::InvEotf(e) => (None, Some(e)),
                        CurveConfig::Both(s, d) => (Some(s), Some(d)),
                    };
                    log::log!(level, "    src_eotf: {src:?}");
                    log::log!(level, "    dst_eotf: {dst:?}");
                    log::log!(level, "    size: {size:?}");
                    log::log!(level, "    scale_for_dst_lut: {scale_for_dst_lut:?}");
                    log::log!(
                        level,
                        "    input_is_scaled_for_dst_lut: {input_is_scaled_for_dst_lut:?}"
                    );
                }
                ColorOpMatchType::Matrix3x4 { matrix, .. } => {
                    log::log!(level, "    type: Matrix3x4");
                    let matrix = format!("{:#?}", format_matrix(&matrix.0));
                    let lines: Vec<_> = matrix.lines().collect();
                    log::log!(level, "    matrix: {}", lines[0]);
                    for line in &lines[1..] {
                        log::log!(level, "            {line}");
                    }
                }
                ColorOpMatchType::Multiply { multiplier, .. } => {
                    log::log!(level, "    type: Multiply");
                    log::log!(level, "    multiplier: {multiplier}");
                }
                ColorOpMatchType::Lut3d { matrix, size, .. } => {
                    log::log!(level, "    type: Lut3d");
                    log::log!(level, "    size: {size:?}");
                    let matrix = format!("{:#?}", format_matrix(&matrix.0));
                    let lines: Vec<_> = matrix.lines().collect();
                    log::log!(level, "    matrix: {}", lines[0]);
                    for line in &lines[1..] {
                        log::log!(level, "            {line}");
                    }
                }
            }
        }
    }

    pub fn compute_programming(
        &self,
        master: &Rc<DrmMaster>,
        shared: &Shared,
        dst: &Rc<ColorDescription>,
    ) -> Option<(PlaneProgramming, Vec<PlaneColorOpProgramming>)> {
        let Some(pipeline) = self.pipeline else {
            return Some((
                PlaneProgramming {
                    id: self.plane,
                    props: PlaneProps { pipeline: None },
                },
                Default::default(),
            ));
        };
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
        for m in &self.matches {
            let p = match m.ty {
                ColorOpMatchType::Bypass => PlaneColorOpProgramming {
                    id: m.id,
                    props: PlaneColorOpProps {
                        bypass: m.bypass,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ColorOpMatchType::Curve1d { ty, .. } => PlaneColorOpProgramming {
                    id: m.id,
                    props: PlaneColorOpProps {
                        bypass: m.bypass,
                        ty: Some(ty),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ColorOpMatchType::Lut1d {
                    interpolation,
                    blob_id,
                    eotf,
                    size,
                    scale_for_dst_lut,
                    input_is_scaled_for_dst_lut,
                } => {
                    let key = Lut1dKey {
                        dst: dst.id,
                        eotf,
                        size,
                        scale_for_dst_lut,
                        input_is_scaled_for_dst_lut,
                    };
                    let blob_key = BlobRegistryKey::Lut1d(key);
                    let blob = match shared.blob_registry.get(&blob_key) {
                        Some(b) => b,
                        None => {
                            let dst_lut_scale = || dst_lut_out_scale(dst).map(|v| v as f32);
                            let curve = create_lut_curve(
                                key.eotf,
                                key.size,
                                key.input_is_scaled_for_dst_lut
                                    .and_then(dst_lut_scale)
                                    .map(|v| 1.0 / v),
                                key.scale_for_dst_lut.and_then(dst_lut_scale),
                            );
                            let lut = create_lut::<u32>(LutConfig::Curve(&curve), None, None);
                            let blob = create_blob!(&*lut);
                            shared.blob_registry.insert(blob_key, blob)
                        }
                    };
                    PlaneColorOpProgramming {
                        id: m.id,
                        props: PlaneColorOpProps {
                            bypass: m.bypass,
                            interpolation: Some(interpolation),
                            data_id: Some(DrmPropertyValue {
                                id: blob_id,
                                value: blob.id(),
                            }),
                            ..Default::default()
                        },
                        _data_blob: Some(blob),
                    }
                }
                ColorOpMatchType::Matrix3x4 {
                    blob_id, matrix, ..
                } => {
                    let map = |r: usize, c: usize| to_s3132(matrix.0[r][c].0);
                    #[rustfmt::skip]
                    let matrix = [
                        map(0, 0), map(0, 1), map(0, 2), map(0, 3),
                        map(1, 0), map(1, 1), map(1, 2), map(1, 3),
                        map(2, 0), map(2, 1), map(2, 2), map(2, 3),
                    ];
                    let key = MatrixKey { matrix };
                    let blob_key = BlobRegistryKey::Matrix(key);
                    let blob = match shared.blob_registry.get(&blob_key) {
                        Some(b) => b,
                        None => {
                            let blob = create_blob!(&key.matrix);
                            shared.blob_registry.insert(blob_key, blob)
                        }
                    };
                    PlaneColorOpProgramming {
                        id: m.id,
                        props: PlaneColorOpProps {
                            bypass: m.bypass,
                            data_id: Some(DrmPropertyValue {
                                id: blob_id,
                                value: blob.id(),
                            }),
                            ..Default::default()
                        },
                        _data_blob: Some(blob),
                    }
                }
                ColorOpMatchType::Lut3d {
                    interpolation,
                    blob_id,
                    matrix,
                    size,
                } => {
                    let key = Lut3dKey { matrix, size };
                    let blob_key = BlobRegistryKey::Lut3d(key);
                    let blob = match shared.blob_registry.get(&blob_key) {
                        Some(b) => b,
                        None => {
                            let m = key.matrix.to_f32();
                            let out = fill_lut_3d(m, key.size);
                            let blob = create_blob!(&*out);
                            shared.blob_registry.insert(blob_key, blob)
                        }
                    };
                    PlaneColorOpProgramming {
                        id: m.id,
                        props: PlaneColorOpProps {
                            bypass: m.bypass,
                            data_id: Some(DrmPropertyValue {
                                id: blob_id,
                                value: blob.id(),
                            }),
                            interpolation: Some(interpolation),
                            ..Default::default()
                        },
                        _data_blob: Some(blob),
                    }
                }
                ColorOpMatchType::Multiply {
                    multiplier_id,
                    multiplier,
                } => PlaneColorOpProgramming {
                    id: m.id,
                    props: PlaneColorOpProps {
                        bypass: m.bypass,
                        multiplier: Some(DrmPropertyValue {
                            id: multiplier_id,
                            value: to_s3132(multiplier),
                        }),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            };
            res.push(p);
        }
        Some((
            PlaneProgramming {
                id: self.plane,
                props: PlaneProps {
                    pipeline: Some(pipeline),
                },
            },
            res,
        ))
    }
}

struct Matcher<'a, 'b, 'c> {
    ops: &'a [Rc<ColorOp>],
    serial: u64,
    partials: &'b mut [Partial],
    matrix: ColorMatrix,
    path: &'a [PlaneOpKind],
    descriptions: StaticCopyMap<PlaneOpTarget, &'c Rc<ColorDescription>>,
    multiply: StaticCopyMap<PlaneOpTarget, f64>,
    curve_type: StaticCopyMap<PlaneOpTarget, Option<ColorOpCurve1dType>>,
}

impl Matcher<'_, '_, '_> {
    fn match_at(&mut self, i: usize, j: usize) -> bool {
        if i == self.ops.len() {
            return j == self.path.len();
        }
        let linear_idx = i * PARTIALS_ROW_LENGTH + j;
        let partial = &self.partials[linear_idx];
        if partial.serial == self.serial {
            return partial.ok;
        }
        let res = self.match_at_uncached(i, j);
        let partial = &mut self.partials[linear_idx];
        partial.serial = self.serial;
        partial.ok = res.is_some();
        if let Some((next_i, ty)) = res {
            partial.next_i = next_i;
            partial.op = {
                let op = &self.ops[i];
                let id = op.op;
                let bypass = op.bypass.map(|id| DrmPropertyValue {
                    id,
                    value: matches!(ty, ColorOpMatchType::Bypass) as u64,
                });
                ColorOpMatch { id, bypass, ty }
            };
        }
        partial.ok
    }

    fn match_at_uncached(&mut self, i: usize, j: usize) -> Option<(usize, ColorOpMatchType)> {
        let op = &self.ops[i];
        let mut skip_to_non_bypass = false;
        'fail: {
            if j < self.path.len() {
                let ty = match (&op.ty, self.path[j]) {
                    (ColorOpType::Curve1d(l), PlaneOpKind::Curve1d { target })
                        if let Some(ty) = self.curve_type[target]
                            && let Some((ty, name)) = l.types[ty] =>
                    {
                        Some(ColorOpMatchType::Curve1d {
                            ty: DrmPropertyValue {
                                id: l.ty,
                                value: ty,
                            },
                            name,
                        })
                    }
                    (
                        ColorOpType::Lut1d(l),
                        PlaneOpKind::Lut1d {
                            eotf,
                            scale_for_dst_lut,
                            input_is_scaled_for_dst_lut,
                        },
                    ) if let Some(ty) = l.interpolations[ColorOpLut1dInterpolationType::Linear] => {
                        let src = self.descriptions[Src].eotf;
                        let dst = self.descriptions[Dst].eotf;
                        let eotf = match eotf {
                            LutEotf::Eotf => CurveConfig::Eotf(src),
                            LutEotf::InvEotf => CurveConfig::InvEotf(dst),
                            LutEotf::Both => CurveConfig::Both(src, dst),
                        };
                        let ty = ColorOpMatchType::Lut1d {
                            interpolation: DrmPropertyValue {
                                id: l.interpolation,
                                value: ty,
                            },
                            blob_id: l.data,
                            eotf,
                            size: l.size,
                            scale_for_dst_lut,
                            input_is_scaled_for_dst_lut,
                        };
                        Some(ty)
                    }
                    (
                        ColorOpType::Matrix3x4(l),
                        PlaneOpKind::Matrix3x4 {
                            apply_src_curve_scale,
                            apply_dst_curve_scale,
                            scale_for_dst_lut,
                            has_multiply_before,
                            has_multiply_after,
                        },
                    ) => {
                        let mut matrix = self.matrix;
                        if apply_src_curve_scale {
                            let s = self.multiply[Src];
                            for r in 0..3 {
                                for c in 0..3 {
                                    matrix.0[r][c].0 *= s;
                                }
                            }
                        }
                        if apply_dst_curve_scale || scale_for_dst_lut {
                            let mut s = 1.0;
                            if apply_dst_curve_scale {
                                s *= self.multiply[Dst];
                            }
                            if scale_for_dst_lut
                                && let Some(v) = dst_lut_out_scale(&self.descriptions[Dst])
                            {
                                s *= v;
                            }
                            for r in 0..3 {
                                for c in 0..4 {
                                    matrix.0[r][c].0 *= s;
                                }
                            }
                        }
                        // AMD clamps at 3.0
                        const MAX: f64 = 3.0;
                        let mut need_multiply_before = false;
                        let mut need_multiply_after = false;
                        for r in &matrix.0 {
                            #[expect(clippy::needless_range_loop)]
                            for c in 0..3 {
                                if r[c].0.abs() > MAX {
                                    need_multiply_before = true;
                                }
                            }
                            if r[3].0.abs() > MAX {
                                need_multiply_after = true;
                            }
                        }
                        if need_multiply_after && !has_multiply_after {
                            break 'fail;
                        }
                        if need_multiply_before && !has_multiply_before && !has_multiply_after {
                            break 'fail;
                        }
                        let mut scale = |range: RangeTo<usize>| {
                            let mut max = 0.0f64;
                            for r in &matrix.0 {
                                for c in &r[range] {
                                    max = max.max(c.0.abs());
                                }
                            }
                            if max >= f64::EPSILON {
                                let scale = MAX / max;
                                for r in &mut matrix.0 {
                                    for c in &mut r[range] {
                                        c.0 *= scale;
                                    }
                                }
                                return Some(1.0 / scale);
                            }
                            None
                        };
                        let mut factor_after = None;
                        if has_multiply_after {
                            factor_after = scale(..4);
                        }
                        let mut factor_before = None;
                        if has_multiply_before {
                            factor_before = scale(..3);
                        }
                        let ty = ColorOpMatchType::Matrix3x4 {
                            blob_id: l.data,
                            matrix,
                            factor_before,
                            factor_after,
                        };
                        Some(ty)
                    }
                    (
                        ColorOpType::Multiplier(l),
                        PlaneOpKind::Multiply {
                            apply_src_curve_scale,
                            apply_dst_curve_scale,
                            scale_for_dst_lut,
                        },
                    ) => {
                        let mut multiplier = 1.0;
                        if apply_src_curve_scale {
                            multiplier *= self.multiply[Src];
                        }
                        if apply_dst_curve_scale {
                            multiplier *= self.multiply[Dst];
                        }
                        if scale_for_dst_lut
                            && let Some(v) = dst_lut_out_scale(&self.descriptions[Dst])
                        {
                            multiplier *= v;
                        }
                        let ty = ColorOpMatchType::Multiply {
                            multiplier_id: l.multiplier,
                            multiplier,
                        };
                        Some(ty)
                    }
                    (ColorOpType::Lut3d(l), PlaneOpKind::Lut3d { scale_for_dst_lut })
                        if let Some(ty) =
                            l.interpolations[ColorOpLut3dInterpolationType::Tetrahedral]
                            && l.size >= 2 =>
                    {
                        let mut matrix = self.matrix;
                        if scale_for_dst_lut
                            && let Some(s) = dst_lut_out_scale(&self.descriptions[Dst])
                        {
                            for r in 0..3 {
                                for c in 0..4 {
                                    matrix.0[r][c].0 *= s;
                                }
                            }
                        }
                        let ty = ColorOpMatchType::Lut3d {
                            interpolation: DrmPropertyValue {
                                id: l.interpolation,
                                value: ty,
                            },
                            blob_id: l.data,
                            matrix,
                            size: l.size,
                        };
                        Some(ty)
                    }
                    _ => None,
                };
                if let Some(ty) = ty {
                    if self.match_at(i + 1, j + 1) {
                        return Some((i + 1, ty));
                    }
                    skip_to_non_bypass = true;
                }
            }
        }
        let next_idx = if skip_to_non_bypass {
            op.next_non_bypass
        } else {
            Some(i + 1)
        };
        if op.bypass.is_some()
            && let Some(idx) = next_idx
            && self.match_at(idx, j)
        {
            return Some((idx, ColorOpMatchType::Bypass));
        }
        None
    }
}
