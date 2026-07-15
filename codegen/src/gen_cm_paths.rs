use crate::gen_cm_paths::types::Criteria;
use crate::gen_cm_paths::types::CrtcOpKind::GammaLut;
use crate::gen_cm_paths::types::CrtcOpKind::{self};
use crate::gen_cm_paths::types::CrtcOpName;
use crate::gen_cm_paths::types::DstEotfCriterion;
use crate::gen_cm_paths::types::LutEotf;
use crate::gen_cm_paths::types::PATH_GAMMA_LUT_IS_RAW;
use crate::gen_cm_paths::types::PATH_HAS_DST_CURVE;
use crate::gen_cm_paths::types::PATH_HAS_GAMMA_LUT;
use crate::gen_cm_paths::types::PATH_HAS_SRC_CURVE;
use crate::gen_cm_paths::types::PATH_MATRIX_VIA_LUT;
use crate::gen_cm_paths::types::PATH_SUPPORTS_MATRIX;
use crate::gen_cm_paths::types::PathFlags;
use crate::gen_cm_paths::types::PlaneOpKind::Curve1d;
use crate::gen_cm_paths::types::PlaneOpKind::Lut1d;
use crate::gen_cm_paths::types::PlaneOpKind::Lut3d;
use crate::gen_cm_paths::types::PlaneOpKind::Matrix3x4;
use crate::gen_cm_paths::types::PlaneOpKind::Multiply;
use crate::gen_cm_paths::types::PlaneOpKind::{self};
use crate::gen_cm_paths::types::PlaneOpName;
use crate::gen_cm_paths::types::PlaneOpTarget::Dst;
use crate::gen_cm_paths::types::PlaneOpTarget::Src;
use crate::gen_cm_paths::types::SrcEotfCriterion;
use crate::update;
use anyhow::Result;
use linearize::Linearize;
use linearize::LinearizeExt;
use linearize::StaticCopyMap;
use linearize::StaticMap;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Write as _;
use std::fmt::{self};
use std::hash::Hash;
use std::ops::Range;

#[path = "../../src/backends/metal/video/metal_cm/metal_cm_paths/types.rs"]
mod types;

#[derive(Copy, Clone, Linearize)]
struct Config {
    plane: PlaneConfig,
    crtc: CrtcConfig,
}

#[derive(Copy, Clone, Linearize)]
enum PlaneConfig {
    Lut1d,
    Split(PlaneSplitConfig),
}

#[derive(Copy, Clone, Linearize)]
struct PlaneSplitConfig {
    src_eotf: PlaneEotfHandling,
    linear: PlaneLinearHandling,
    dst_eotf: PlaneEotfHandling,
}

#[derive(Copy, Clone, Linearize)]
struct CrtcConfig {
    gamma_lut: bool,
}

#[derive(Copy, Clone, Linearize)]
enum PlaneEotfHandling {
    None,
    Curve,
    Lut1d,
}

#[derive(Copy, Clone, Linearize)]
enum PlaneLinearHandling {
    None,
    MultiplyOnly,
    Matrix {
        has_multiply_before: bool,
        has_multiply_after: bool,
    },
    Lut3d {
        has_multiply_before: bool,
        has_multiply_after: bool,
    },
}

fn validate(plane: &PlaneConfig, crtc: &CrtcConfig, criteria: Criteria) -> bool {
    let Criteria::Some { src_eotf, dst_eotf } = criteria else {
        // If there are no requirements, we want to have exactly two paths:
        // - COLOR_PIPELINE disabled & GAMMA_LUT disabled
        // - COLOR_PIPELINE disabled & GAMMA_LUT enabled
        return matches!(
            plane,
            PlaneConfig::Split(PlaneSplitConfig {
                src_eotf: PlaneEotfHandling::None,
                linear: PlaneLinearHandling::None,
                dst_eotf: PlaneEotfHandling::None,
            }),
        );
    };
    if let PlaneConfig::Split(plane) = plane
        && matches!(plane.src_eotf, PlaneEotfHandling::Lut1d)
        && matches!(plane.linear, PlaneLinearHandling::None)
        && matches!(plane.dst_eotf, PlaneEotfHandling::Lut1d)
    {
        // We don't want any paths that are two plane luts with nothing in between.
        return false;
    }
    if let SrcEotfCriterion::None = src_eotf
        && let PlaneConfig::Split(plane) = plane
        && matches!(plane.src_eotf, PlaneEotfHandling::Curve)
    {
        // If we don't have src requirements, then our path cannot contain a src
        // curve.
        return false;
    }
    if let DstEotfCriterion::None = dst_eotf
        && let PlaneConfig::Split(plane) = plane
        && matches!(plane.dst_eotf, PlaneEotfHandling::Curve)
    {
        // If we don't have dst requirements, then our path cannot contain a dst
        // curve.
        return false;
    }
    if let SrcEotfCriterion::Some { .. } = src_eotf
        && let PlaneConfig::Split(plane) = plane
        && matches!(plane.src_eotf, PlaneEotfHandling::None)
        && !crtc.gamma_lut
    {
        // If we have src eotf requirements but neither a plane eotf nor a gamma lut,
        // we cannot implement the src eotf.
        return false;
    }
    if let DstEotfCriterion::Some { .. } = dst_eotf
        && let PlaneConfig::Split(plane) = plane
        && matches!(plane.dst_eotf, PlaneEotfHandling::None)
        && !crtc.gamma_lut
    {
        // If we have dst eotf requirements but neither a plane eotf nor a gamma lut,
        // we cannot implement the dst eotf.
        return false;
    }
    if let SrcEotfCriterion::Some { .. } = src_eotf
        && let PlaneConfig::Split(plane) = plane
        && matches!(plane.src_eotf, PlaneEotfHandling::None)
        && !(matches!(plane.linear, PlaneLinearHandling::None)
            && matches!(plane.dst_eotf, PlaneEotfHandling::None))
    {
        // If there are src eotf requirements but the plane pipeline does not handle eotf,
        // then we might still be able to put everything into the GAMMA_LUT. However, in
        // that case we can't have any plane operations since they would run before the
        // src eotf.
        return false;
    }
    if let DstEotfCriterion::Some {
        scale_lut_input: true,
        ..
    } = dst_eotf
        && let PlaneConfig::Split(plane) = plane
        && !matches!(plane.src_eotf, PlaneEotfHandling::Lut1d)
        && matches!(plane.linear, PlaneLinearHandling::None)
        && !matches!(plane.dst_eotf, PlaneEotfHandling::Curve)
        && !(matches!(src_eotf, SrcEotfCriterion::Some { .. })
            && matches!(plane.src_eotf, PlaneEotfHandling::None))
    {
        // If we have dst eotf requirements and the eotf needs to have its input scaled
        // when implemented via a lut and the eotf is handled via either a plane lut or
        // the GAMMA_LUT, then one of the following must hold:
        // - we have a linear operation to scale the input
        // - or there must also be a src eotf and we handle both in the same lut
        return false;
    }
    if let SrcEotfCriterion::Some { curve_needs_scale } = src_eotf
        && curve_needs_scale
        && let PlaneConfig::Split(plane) = plane
        && matches!(plane.src_eotf, PlaneEotfHandling::Curve)
        && matches!(
            plane.linear,
            PlaneLinearHandling::None
                | PlaneLinearHandling::Lut3d {
                    has_multiply_before: false,
                    ..
                }
        )
    {
        // If we have src eotf requirements and the src curve is pq128, then we
        // need a linear operation to scale the value back to [0, 1].
        return false;
    }
    if let DstEotfCriterion::Some {
        curve_needs_scale: true,
        ..
    } = dst_eotf
        && let PlaneConfig::Split(plane) = plane
        && matches!(plane.dst_eotf, PlaneEotfHandling::Curve)
        && matches!(
            plane.linear,
            PlaneLinearHandling::None
                | PlaneLinearHandling::Lut3d {
                    has_multiply_after: false,
                    ..
                }
        )
    {
        // If we have dst eotf requirements and the eotf is pq125 and the eotf is
        // implemented via a curve, then we need a linear component to scale the value
        // down to [0, 1].
        return false;
    }
    true
}

#[derive(Debug)]
struct Path {
    flags: PathFlags,
    plane: Vec<PlaneOpKind>,
    crtc: Vec<CrtcOpKind>,
}

fn to_path(plane: &PlaneConfig, crtc: &CrtcConfig, crit: &Criteria) -> Path {
    let mut crtc_eotf = None;
    let mut crtc_input_is_scaled_for_dst_lut = false;
    let mut pp = vec![];
    let mut flags = PathFlags::none();
    if let Criteria::Some { src_eotf, dst_eotf } = crit {
        match plane {
            PlaneConfig::Lut1d => {
                pp.push(Lut1d {
                    eotf: LutEotf::Both,
                    scale_for_dst_lut: false,
                    input_is_scaled_for_dst_lut: false,
                });
            }
            PlaneConfig::Split(plane) => {
                let needs_scale_for_dst_lut =
                    matches!(
                        dst_eotf,
                        DstEotfCriterion::Some {
                            scale_lut_input: true,
                            ..
                        }
                    ) && !(matches!(src_eotf, SrcEotfCriterion::Some { .. })
                        && matches!(plane.src_eotf, PlaneEotfHandling::None));
                match plane.src_eotf {
                    PlaneEotfHandling::None => {}
                    PlaneEotfHandling::Curve => {
                        flags |= PATH_HAS_SRC_CURVE;
                        pp.push(Curve1d { target: Src })
                    }
                    PlaneEotfHandling::Lut1d => pp.push(Lut1d {
                        eotf: LutEotf::Eotf,
                        scale_for_dst_lut: needs_scale_for_dst_lut
                            && match plane.linear {
                                PlaneLinearHandling::MultiplyOnly => false,
                                PlaneLinearHandling::Matrix { .. } => false,
                                PlaneLinearHandling::Lut3d { .. } => false,
                                PlaneLinearHandling::None => match plane.dst_eotf {
                                    PlaneEotfHandling::None => crtc.gamma_lut,
                                    PlaneEotfHandling::Curve => false,
                                    PlaneEotfHandling::Lut1d => true,
                                },
                            },
                        input_is_scaled_for_dst_lut: false,
                    }),
                }
                let scale_linear_for_dst_lut = || {
                    needs_scale_for_dst_lut
                        && match plane.dst_eotf {
                            PlaneEotfHandling::None => crtc.gamma_lut,
                            PlaneEotfHandling::Curve => false,
                            PlaneEotfHandling::Lut1d => true,
                        }
                };
                let linear_need_src_curve_scale =
                    matches!(
                        src_eotf,
                        SrcEotfCriterion::Some {
                            curve_needs_scale: true
                        }
                    ) && matches!(plane.src_eotf, PlaneEotfHandling::Curve);
                let linear_need_dst_curve_scale =
                    matches!(
                        dst_eotf,
                        DstEotfCriterion::Some {
                            curve_needs_scale: true,
                            ..
                        }
                    ) && matches!(plane.dst_eotf, PlaneEotfHandling::Curve);
                match plane.linear {
                    PlaneLinearHandling::None => {}
                    PlaneLinearHandling::MultiplyOnly => pp.push(Multiply {
                        apply_src_curve_scale: linear_need_src_curve_scale,
                        apply_dst_curve_scale: linear_need_dst_curve_scale,
                        scale_for_dst_lut: scale_linear_for_dst_lut(),
                    }),
                    PlaneLinearHandling::Lut3d {
                        has_multiply_before,
                        has_multiply_after,
                    } => {
                        flags |= PATH_SUPPORTS_MATRIX;
                        flags |= PATH_MATRIX_VIA_LUT;
                        if has_multiply_before {
                            pp.push(Multiply {
                                apply_src_curve_scale: linear_need_src_curve_scale,
                                apply_dst_curve_scale: false,
                                scale_for_dst_lut: false,
                            });
                        }
                        pp.push(Lut3d {
                            scale_for_dst_lut: scale_linear_for_dst_lut(),
                        });
                        if has_multiply_after {
                            pp.push(Multiply {
                                apply_src_curve_scale: false,
                                apply_dst_curve_scale: linear_need_dst_curve_scale,
                                scale_for_dst_lut: false,
                            });
                        }
                    }
                    PlaneLinearHandling::Matrix {
                        has_multiply_before,
                        has_multiply_after,
                    } => {
                        flags |= PATH_SUPPORTS_MATRIX;
                        if has_multiply_before {
                            pp.push(Multiply {
                                apply_src_curve_scale: linear_need_src_curve_scale,
                                apply_dst_curve_scale: false,
                                scale_for_dst_lut: false,
                            });
                        }
                        pp.push(Matrix3x4 {
                            apply_src_curve_scale: !has_multiply_before
                                && linear_need_src_curve_scale,
                            apply_dst_curve_scale: !has_multiply_after
                                && linear_need_dst_curve_scale,
                            scale_for_dst_lut: !has_multiply_after && scale_linear_for_dst_lut(),
                            has_multiply_before,
                            has_multiply_after,
                        });
                        if has_multiply_after {
                            pp.push(Multiply {
                                apply_src_curve_scale: false,
                                apply_dst_curve_scale: linear_need_dst_curve_scale,
                                scale_for_dst_lut: scale_linear_for_dst_lut(),
                            });
                        }
                    }
                }
                match plane.dst_eotf {
                    PlaneEotfHandling::None => {
                        if needs_scale_for_dst_lut {
                            crtc_input_is_scaled_for_dst_lut = true;
                        }
                    }
                    PlaneEotfHandling::Curve => {
                        flags |= PATH_HAS_DST_CURVE;
                        pp.push(Curve1d { target: Dst })
                    }
                    PlaneEotfHandling::Lut1d => pp.push(Lut1d {
                        eotf: LutEotf::InvEotf,
                        scale_for_dst_lut: false,
                        input_is_scaled_for_dst_lut: needs_scale_for_dst_lut,
                    }),
                }
                let crtc_src_eotf = matches!(src_eotf, SrcEotfCriterion::Some { .. })
                    && matches!(plane.src_eotf, PlaneEotfHandling::None);
                let crtc_dst_eotf = matches!(dst_eotf, DstEotfCriterion::Some { .. })
                    && matches!(plane.dst_eotf, PlaneEotfHandling::None);
                crtc_eotf = match (crtc_src_eotf, crtc_dst_eotf) {
                    (false, false) => None,
                    (false, true) => Some(LutEotf::InvEotf),
                    (true, false) => Some(LutEotf::Eotf),
                    (true, true) => Some(LutEotf::Both),
                };
            }
        }
    }
    let mut cp = vec![];
    if crtc.gamma_lut {
        cp.push(GammaLut {
            eotf: crtc_eotf,
            input_is_scaled_for_dst_lut: crtc_input_is_scaled_for_dst_lut,
        });
    }
    if crtc.gamma_lut {
        flags |= PATH_HAS_GAMMA_LUT;
        let distorted = crtc_eotf.is_some() || crtc_input_is_scaled_for_dst_lut;
        if !distorted {
            flags |= PATH_GAMMA_LUT_IS_RAW;
        }
    }
    Path {
        flags,
        plane: pp,
        crtc: cp,
    }
}

fn handle_prefix<T: Debug + Eq + Hash>(
    prefix: &mut String,
    l: &mut HashMap<Vec<T>, Range<usize>>,
    len: &mut usize,
    path: Vec<T>,
) -> Result<Range<usize>> {
    handle_prefix2(prefix, l, len, path, |fmt, t| Debug::fmt(t, fmt))
}

fn handle_prefix2<T: Eq + Hash>(
    prefix: &mut String,
    l: &mut HashMap<Vec<T>, Range<usize>>,
    len: &mut usize,
    path: Vec<T>,
    fmt: impl Fn(&mut Formatter, &T) -> fmt::Result,
) -> Result<Range<usize>> {
    let l = match l.entry(path) {
        Entry::Occupied(v) => v.get().clone(),
        Entry::Vacant(v) => {
            let path = v.key();
            let lo = *len;
            let hi = lo + path.len();
            *len = hi;
            for p in path {
                writeln!(prefix, "    {},", fmt::from_fn(|f| fmt(f, p)))?;
            }
            v.insert(lo..hi);
            lo..hi
        }
    };
    Ok(l)
}

fn handle_element<T: Eq + Hash + Debug>(
    prefix: &mut String,
    l: &mut HashMap<T, usize>,
    path: T,
) -> Result<usize> {
    let len = l.len();
    let l = match l.entry(path) {
        Entry::Occupied(v) => *v.get(),
        Entry::Vacant(v) => {
            let path = v.key();
            writeln!(prefix, "    {path:?},")?;
            v.insert(len);
            len
        }
    };
    Ok(l)
}

fn write_paths() -> Result<()> {
    let mut pairs: Vec<_> = Config::variants().collect();
    pairs.sort_by(|l, r| {
        let count_lut3d = |c: &Config| {
            let mut count = 0;
            if let PlaneConfig::Split(plane) = &c.plane
                && let PlaneLinearHandling::Lut3d { .. } = plane.linear
            {
                count += 1;
            }
            count
        };
        let count_curves = |c: &Config| {
            let mut count = 0;
            if let PlaneConfig::Split(plane) = &c.plane {
                count += matches!(plane.src_eotf, PlaneEotfHandling::Curve) as u32;
                count += matches!(plane.dst_eotf, PlaneEotfHandling::Curve) as u32;
            }
            count
        };
        let count_gamma_lut = |c: &Config| {
            let mut count = 0;
            if c.crtc.gamma_lut {
                count += 1;
            }
            count
        };
        Ordering::Equal
            .then_with(|| count_gamma_lut(l).cmp(&count_gamma_lut(r)))
            .then_with(|| count_lut3d(l).cmp(&count_lut3d(r)))
            .then_with(|| count_curves(l).cmp(&count_curves(r)).reverse())
    });

    let paths = StaticMap::<Criteria, Vec<_>>::from_fn(|crit| {
        pairs
            .iter()
            .filter(|Config { plane, crtc }| validate(plane, crtc, crit))
            .map(|Config { plane, crtc }| to_path(plane, crtc, &crit))
            .collect::<Vec<_>>()
    });

    let mut p_prefix = String::new();
    let mut p = HashMap::<PlaneOpKind, usize>::new();

    let mut c_prefix = String::new();
    let mut c = HashMap::<CrtcOpKind, usize>::new();

    let mut pl_prefix = String::new();
    let mut pl_len = 0;
    let mut pl = HashMap::<Vec<usize>, Range<usize>>::new();

    let mut cl_prefix = String::new();
    let mut cl_len = 0;
    let mut cl = HashMap::<Vec<usize>, Range<usize>>::new();

    let mut xl_prefix = String::new();
    let mut xl_len = 0;
    let mut xl = HashMap::<Vec<(PathFlags, Range<usize>, Range<usize>)>, Range<usize>>::new();

    let mut map = String::new();
    let mut plane_matchers = String::new();
    let mut crtc_matchers = String::new();
    let mut max_plane_len = 0;
    let mut max_crtc_len = 0;
    for (_, paths) in paths {
        let mut x = vec![];
        let mut plane_tts = vec![];
        let mut plane_lens = vec![];
        let mut crtc_tts = vec![];
        let mut crtc_lens = vec![];
        let mut crit_max_plane_len = 0;
        let mut crit_max_crtc_len = 0;
        for path in paths {
            let mut plane_tt = StaticCopyMap::default();
            for name in PlaneOpName::variants() {
                let mut tt: u64 = 0;
                for (idx, kind) in path.plane.iter().enumerate() {
                    let matches = match (name, kind) {
                        (PlaneOpName::Curve1d, Curve1d { .. }) => true,
                        (PlaneOpName::Lut1d, Lut1d { .. }) => true,
                        (PlaneOpName::Lut3d, Lut3d { .. }) => true,
                        (PlaneOpName::Matrix3x4, Matrix3x4 { .. }) => true,
                        (PlaneOpName::Multiply, Multiply { .. }) => true,
                        _ => false,
                    };
                    if matches {
                        tt |= 1 << idx;
                    }
                }
                plane_tt[name] = tt;
            }
            plane_tts.push(plane_tt);
            plane_lens.push(path.plane.len());
            let mut crtc_tt = StaticCopyMap::default();
            for name in CrtcOpName::variants() {
                let mut tt: u64 = 0;
                for (idx, kind) in path.crtc.iter().enumerate() {
                    let matches = match (name, kind) {
                        (CrtcOpName::GammaLut, GammaLut { .. }) => true,
                        _ => false,
                    };
                    if matches {
                        tt |= 1 << idx;
                    }
                }
                crtc_tt[name] = tt;
            }
            crtc_tts.push(crtc_tt);
            crtc_lens.push(path.crtc.len());
            let mut plane = vec![];
            for op in path.plane {
                plane.push(handle_element(&mut p_prefix, &mut p, op)?);
            }
            let mut crtc = vec![];
            for op in path.crtc {
                crtc.push(handle_element(&mut c_prefix, &mut c, op)?);
            }
            max_plane_len = max_plane_len.max(plane.len());
            max_crtc_len = max_crtc_len.max(crtc.len());
            crit_max_plane_len = crit_max_plane_len.max(plane.len());
            crit_max_crtc_len = crit_max_crtc_len.max(crtc.len());
            let pl = handle_prefix(&mut pl_prefix, &mut pl, &mut pl_len, plane)?;
            let cl = handle_prefix(&mut cl_prefix, &mut cl, &mut cl_len, crtc)?;
            x.push((path.flags, pl, cl));
        }
        let xl = handle_prefix2(
            &mut xl_prefix,
            &mut xl,
            &mut xl_len,
            x,
            |fmt, (flags, pl, cl)| {
                write!(
                    fmt,
                    "Path {{ flags: PathFlags({}), pl_lo: {}, pl_len: {}, cl_lo: {}, cl_len: {} }}",
                    flags.0,
                    pl.start,
                    pl.len(),
                    cl.start,
                    cl.len(),
                )
            },
        )?;
        writeln!(map, "        const_slice(&XL, {xl:?}),")?;
        let num_paths = plane_tts.len();
        writeln!(
            plane_matchers,
            "        &MatcherImpl::<{}, PlaneOpName, {num_paths}> {{",
            len_to_bit_mask(crit_max_plane_len + 1),
        )?;
        writeln!(plane_matchers, "                tt: [")?;
        for tt in plane_tts {
            writeln!(
                plane_matchers,
                "                    StaticCopyMap({:?}),",
                tt.0
            )?;
        }
        writeln!(plane_matchers, "                ],")?;
        writeln!(plane_matchers, "                len: {plane_lens:?},")?;
        writeln!(plane_matchers, "        }},")?;
        writeln!(
            crtc_matchers,
            "        &MatcherImpl::<{}, CrtcOpName, {num_paths}> {{",
            len_to_bit_mask(crit_max_crtc_len + 1),
        )?;
        writeln!(crtc_matchers, "                tt: [")?;
        for tt in crtc_tts {
            writeln!(
                crtc_matchers,
                "                    StaticCopyMap({:?}),",
                tt.0
            )?;
        }
        writeln!(crtc_matchers, "                ],")?;
        writeln!(crtc_matchers, "                len: {crtc_lens:?},")?;
        writeln!(crtc_matchers, "        }},")?;
    }

    let mut f = String::new();
    define_w!(f);
    wl!("use linearize::StaticCopyMap;");
    wl!("use super::types::{{*, PlaneOpKind::*, CrtcOpKind::*, PlaneOpTarget::*, LutEotf::*}};");
    wl!("use crate::utils::const_slice::const_slice;");
    wl!("use super::Path;");
    wl!("use super::matcher::{{Matcher, MatcherImpl}};");
    wl!();
    wl!("pub(super) static P: [PlaneOpKind; {}] = [", p.len());
    f.push_str(&p_prefix);
    wl!("];");
    wl!();
    wl!("pub(super) static C: [CrtcOpKind; {}] = [", c.len());
    f.push_str(&c_prefix);
    wl!("];");
    wl!();
    wl!(
        "pub(super) static PL: [{}; {pl_len}] = [",
        len_to_ty(p.len())
    );
    f.push_str(&pl_prefix);
    wl!("];");
    wl!();
    wl!(
        "pub(super) static CL: [{}; {cl_len}] = [",
        len_to_ty(c.len())
    );
    f.push_str(&cl_prefix);
    wl!("];");
    wl!();
    wl!("pub(super) static XL: [Path; {xl_len}] = [");
    f.push_str(&xl_prefix);
    wl!("];");
    wl!();
    wl!("pub static PATHS: StaticCopyMap<Criteria, &'static [Path]> = {{");
    wl!("    let map = [");
    f.push_str(&map);
    wl!("    ];");
    wl!("    StaticCopyMap(map)");
    wl!("}};");
    wl!();
    wl!(
        "pub static PLANE_MATCHERS: StaticCopyMap<Criteria, &'static dyn Matcher<PlaneOpName>> = {{"
    );
    wl!("    let map: [&dyn Matcher<PlaneOpName>; _] = [");
    f.push_str(&plane_matchers);
    wl!("    ];");
    wl!("    StaticCopyMap(map)");
    wl!("}};");
    wl!();
    wl!("pub static CRTC_MATCHERS: StaticCopyMap<Criteria, &'static dyn Matcher<CrtcOpName>> = {{");
    wl!("    let map: [&dyn Matcher<CrtcOpName>; _] = [");
    f.push_str(&crtc_matchers);
    wl!("    ];");
    wl!("    StaticCopyMap(map)");
    wl!("}};");
    wl!();
    wl!("pub const MAX_PLANE_PATH_LEN: usize = {max_plane_len};");
    wl!("pub const MAX_CRTC_PATH_LEN: usize = {max_crtc_len};");
    wl!();
    wl!("pub(super) type PlLoTy = {};", len_to_ty(pl_len));
    wl!("pub(super) type PlLenTy = {};", len_to_ty(max_plane_len));
    wl!("pub(super) type ClLoTy = {};", len_to_ty(cl_len));
    wl!("pub(super) type ClLenTy = {};", len_to_ty(max_crtc_len));

    update(
        "src/backends/metal/video/metal_cm/metal_cm_paths/generated.rs",
        &f,
    )?;

    Ok(())
}

fn len_to_ty(len: usize) -> &'static str {
    const U8_MAX: usize = u8::MAX as usize;
    const U16_MAX: usize = u16::MAX as usize;
    const U32_MAX: usize = u32::MAX as usize;

    #[expect(clippy::match_overlapping_arm)]
    match len.saturating_sub(1) {
        ..=U8_MAX => "u8",
        ..=U16_MAX => "u16",
        ..=U32_MAX => "u32",
        _ => unreachable!(),
    }
}

fn len_to_bit_mask(len: usize) -> &'static str {
    #[expect(clippy::match_overlapping_arm)]
    match len {
        ..=8 => "u8",
        ..=16 => "u16",
        ..=32 => "u32",
        ..=64 => "u64",
        _ => unreachable!(),
    }
}

pub fn main() -> Result<()> {
    write_paths()?;
    Ok(())
}
