use linearize::Linearize;

#[derive(Copy, Clone, Debug, Linearize)]
pub enum Criteria {
    None,
    Some {
        src_eotf: SrcEotfCriterion,
        dst_eotf: DstEotfCriterion,
    },
}

#[derive(Copy, Clone, Debug, Linearize)]
pub enum SrcEotfCriterion {
    None,
    Some { curve_needs_scale: bool },
}

#[derive(Copy, Clone, Debug, Linearize)]
pub enum DstEotfCriterion {
    None,
    Some {
        curve_needs_scale: bool,
        scale_lut_input: bool,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum LutEotf {
    Eotf,
    InvEotf,
    Both,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum PlaneOpKind {
    Curve1d {
        target: PlaneOpTarget,
    },
    Lut1d {
        eotf: LutEotf,
        scale_for_dst_lut: bool,
        input_is_scaled_for_dst_lut: bool,
    },
    Lut3d {
        scale_for_dst_lut: bool,
    },
    Matrix3x4 {
        apply_src_curve_scale: bool,
        apply_dst_curve_scale: bool,
        scale_for_dst_lut: bool,
        has_multiply_before: bool,
        has_multiply_after: bool,
    },
    Multiply {
        apply_src_curve_scale: bool,
        apply_dst_curve_scale: bool,
        scale_for_dst_lut: bool,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Linearize)]
pub enum PlaneOpName {
    Other,
    Curve1d,
    Lut1d,
    Lut3d,
    Matrix3x4,
    Multiply,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Linearize)]
pub enum PlaneOpTarget {
    Src,
    Dst,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum CrtcOpKind {
    GammaLut {
        eotf: Option<LutEotf>,
        input_is_scaled_for_dst_lut: bool,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Linearize)]
pub enum CrtcOpName {
    Other,
    GammaLut,
}

bitflags! {
    PathFlags: u8;
        PATH_SUPPORTS_MATRIX,
        PATH_MATRIX_VIA_LUT,
        PATH_HAS_GAMMA_LUT,
        PATH_GAMMA_LUT_IS_RAW,
        PATH_HAS_SRC_CURVE,
        PATH_HAS_DST_CURVE,
}
