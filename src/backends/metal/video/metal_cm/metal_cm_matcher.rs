use crate::backend::BackendGammaLut;
use crate::backends::metal::video::metal_cm::MetalCmCrtc;
use crate::backends::metal::video::metal_cm::MetalCmPlane;
use crate::backends::metal::video::metal_cm::Programming;
use crate::backends::metal::video::metal_cm::Shared;
use crate::backends::metal::video::metal_cm::metal_cm_crtc::metal_cm_crtc_matcher::CrtcColorPipelineMatch;
use crate::backends::metal::video::metal_cm::metal_cm_paths::Criteria;
use crate::backends::metal::video::metal_cm::metal_cm_paths::DstEotfCriterion;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PATH_GAMMA_LUT_IS_RAW;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PATH_HAS_DST_CURVE;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PATH_HAS_GAMMA_LUT;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PATH_HAS_SRC_CURVE;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PATH_MATRIX_VIA_LUT;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PATH_SUPPORTS_MATRIX;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PATHS;
use crate::backends::metal::video::metal_cm::metal_cm_paths::Path;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PlaneOpTarget::Dst;
use crate::backends::metal::video::metal_cm::metal_cm_paths::PlaneOpTarget::Src;
use crate::backends::metal::video::metal_cm::metal_cm_paths::SrcEotfCriterion;
use crate::backends::metal::video::metal_cm::metal_cm_plane::ColorOpCurve1dType;
use crate::backends::metal::video::metal_cm::metal_cm_plane::metal_cm_plane_matcher::PlaneColorPipelineMatch;
use crate::cmm::cmm_description::ColorDescription;
use crate::cmm::cmm_eotf::Eotf;
use crate::cmm::cmm_eotf::EotfPow;
use crate::cmm::cmm_render_intent::RenderIntent;
use crate::cmm::cmm_transform::ColorMatrix;
use crate::utils::bhash::BHashMap;
use crate::video::drm::DrmCrtc;
use crate::video::drm::DrmMaster;
use crate::video::drm::DrmPlane;
use arrayvec::ArrayVec;
use hashbrown::hash_map::Entry;
use isnt::std_1::primitive::IsntSliceExt;
use linearize::LinearizeExt;
use linearize::StaticCopyMap;
use linearize::StaticMap;
use linearize::static_copy_map;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
pub struct MatcherCache {
    viable: RefCell<BHashMap<(DrmPlane, DrmCrtc), StaticMap<Criteria, Box<[(usize, Path)]>>>>,
}

pub(super) fn compute_programming(
    master: &Rc<DrmMaster>,
    shared: &Shared,
    plane: &MetalCmPlane,
    crtc: &MetalCmCrtc,
    src: &Rc<ColorDescription>,
    dst: &Rc<ColorDescription>,
    intent: RenderIntent,
    client_gamma_lut: Option<&Rc<BackendGammaLut>>,
    has_cursor_plane: bool,
    use_plane_color_pipelines: bool,
) -> Programming {
    let res = compute_programming_(
        master,
        shared,
        plane,
        crtc,
        src,
        dst,
        intent,
        client_gamma_lut,
        has_cursor_plane,
        use_plane_color_pipelines,
    );
    res.unwrap_or_else(|| Programming {
        failed: true,
        plane: Default::default(),
        plane_color_ops: Default::default(),
        crtc_color_ops: Default::default(),
    })
}

fn compute_programming_(
    master: &Rc<DrmMaster>,
    shared: &Shared,
    plane: &MetalCmPlane,
    crtc: &MetalCmCrtc,
    src: &Rc<ColorDescription>,
    dst: &Rc<ColorDescription>,
    intent: RenderIntent,
    client_gamma_lut: Option<&Rc<BackendGammaLut>>,
    has_cursor_plane: bool,
    use_plane_color_pipelines: bool,
) -> Option<Programming> {
    let (plane, crtc) = find_matches(
        shared,
        plane,
        crtc,
        src,
        dst,
        intent,
        client_gamma_lut,
        has_cursor_plane,
        use_plane_color_pipelines,
    )?;
    plane.log();
    crtc.log();
    let (plane, plane_color_ops) = plane.compute_programming(master, shared, dst)?;
    let crtc_color_ops = crtc.compute_programming(master, shared, dst, client_gamma_lut)?;
    Some(Programming {
        failed: false,
        plane,
        plane_color_ops,
        crtc_color_ops,
    })
}

pub(super) fn find_matches(
    shared: &Shared,
    plane: &MetalCmPlane,
    crtc: &MetalCmCrtc,
    src: &Rc<ColorDescription>,
    dst: &Rc<ColorDescription>,
    intent: RenderIntent,
    client_gamma_lut: Option<&Rc<BackendGammaLut>>,
    has_cursor_plane: bool,
    use_plane_color_pipelines: bool,
) -> Option<(PlaneColorPipelineMatch, CrtcColorPipelineMatch)> {
    let needs_gamma_lut = client_gamma_lut.is_some();
    let needs_matrix = !src.linear.embeds_into(&dst.linear, intent);
    let mut needs_eotf = StaticCopyMap::default();
    if needs_matrix || src.eotf != dst.eotf {
        needs_eotf[Src] = src.eotf != Eotf::Linear;
        needs_eotf[Dst] = dst.eotf != Eotf::Linear;
    }
    let descriptions = static_copy_map! {
        Src => src,
        Dst => dst,
    };
    let plane_curve_types = descriptions.map(|target, v| {
        use ColorOpCurve1dType::*;
        match v.eotf {
            Eotf::St2084Pq => match target {
                Src => Some(Pq125Eotf),
                Dst => Some(Pq125InverseEotf),
            },
            Eotf::Gamma22 | Eotf::Pow(EotfPow::GAMMA22) => match target {
                Src => Some(Gamma22),
                Dst => Some(Gamma22Inverse),
            },
            Eotf::CompoundPower24 => match target {
                Src => Some(SrgbEotf),
                Dst => Some(SrgbInverseEotf),
            },
            _ => None,
        }
    });
    let needs_multiply = descriptions.map_values(|v| v.eotf == Eotf::St2084Pq);
    let src_eotf_criterion = match needs_eotf[Src] {
        false => SrcEotfCriterion::None,
        true => SrcEotfCriterion::Some {
            curve_needs_scale: needs_multiply[Src],
        },
    };
    let dst_eotf_criterion = match needs_eotf[Dst] {
        false => DstEotfCriterion::None,
        true => DstEotfCriterion::Some {
            curve_needs_scale: needs_multiply[Dst],
            scale_lut_input: dst.eotf == Eotf::St2084Pq,
        },
    };
    let multiply = static_copy_map! {
        Src => match descriptions[Src].eotf {
            Eotf::St2084Pq => 1.0 / 125.0,
            _ => 1.0,
        },
        Dst => match descriptions[Dst].eotf {
            Eotf::St2084Pq => 125.0,
            _ => 1.0,
        },
    };
    let matrix = match needs_matrix {
        true => src.linear.color_transform(&dst.linear, intent),
        false => ColorMatrix::IDENTITY,
    };
    let criteria = if !needs_matrix
        && let SrcEotfCriterion::None = src_eotf_criterion
        && let DstEotfCriterion::None = dst_eotf_criterion
    {
        Criteria::None
    } else {
        Criteria::Some {
            src_eotf: src_eotf_criterion,
            dst_eotf: dst_eotf_criterion,
        }
    };
    let criteria = criteria.linearized();
    let viable = &mut *shared.matcher_cache.viable.borrow_mut();
    let viable = match viable.entry((plane.id, crtc.id)) {
        Entry::Occupied(o) => o.into_mut(),
        Entry::Vacant(v) => {
            let map = StaticMap::<Criteria, _>::from_fn(|v| {
                let c = v.linearized();
                let plane_filter = &plane.pipelines.filter;
                let crtc_filter = &crtc.pipelines.filter;
                let mut res = vec![];
                for (path_idx, &path) in PATHS[c].iter().enumerate() {
                    if plane_filter.contains(c, path_idx) && crtc_filter.contains(c, path_idx) {
                        res.push((path_idx, path));
                    }
                }
                res.into_boxed_slice()
            });
            v.insert(map)
        }
    };
    let viable = &viable[criteria];
    let mut plane_path = ArrayVec::new();
    let mut crtc_path = ArrayVec::new();
    'outer: for &(path_idx, path) in viable {
        for (flag, target) in [(PATH_HAS_SRC_CURVE, Src), (PATH_HAS_DST_CURVE, Dst)] {
            if path.flags.contains(flag) {
                let Some(ty) = plane_curve_types[target] else {
                    continue 'outer;
                };
                if !plane.pipelines.supported_curves[ty] {
                    continue 'outer;
                }
            }
        }
        if path.flags.not_contains(PATH_SUPPORTS_MATRIX) && needs_matrix {
            continue;
        }
        if path.flags.contains(PATH_MATRIX_VIA_LUT) && !src.linear.target_contained_in_primary() {
            continue;
        }
        if path.flags.contains(PATH_HAS_GAMMA_LUT) {
            if path.flags.not_contains(PATH_GAMMA_LUT_IS_RAW) && has_cursor_plane {
                continue;
            }
        } else {
            if needs_gamma_lut {
                continue;
            }
        }
        path.plane(&mut plane_path);
        if !use_plane_color_pipelines && plane_path.is_not_empty() {
            continue;
        }
        let res = plane.pipelines.find_match(
            shared,
            descriptions,
            plane_curve_types,
            multiply,
            matrix,
            criteria,
            path_idx,
            &plane_path,
        );
        let Some(plane_match) = res else {
            continue;
        };
        path.crtc(&mut crtc_path);
        let res = crtc
            .pipelines
            .find_match(shared, descriptions, criteria, path_idx, &crtc_path);
        let Some(crtc_match) = res else {
            continue;
        };
        return Some((plane_match, crtc_match));
    }
    None
}
