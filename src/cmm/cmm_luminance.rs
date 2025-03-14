use crate::{
    cmm::cmm_transform::{ColorMatrix, Xyz},
    utils::ordered_float::F64,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Luminance {
    pub min: F64,
    pub max: F64,
    pub white: F64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct TargetLuminance {
    pub min: F64,
    pub max: F64,
}

impl Luminance {
    pub const SRGB: Self = Self {
        min: F64(0.2),
        max: F64(80.0),
        white: F64(80.0),
    };

    pub const BT1886: Self = Self {
        min: F64(0.01),
        max: F64(100.0),
        white: F64(100.0),
    };

    pub const ST2084_PQ: Self = Self {
        min: F64(0.0),
        max: F64(10000.0),
        white: F64(203.0),
    };

    #[expect(dead_code)]
    pub const HLG: Self = Self {
        min: F64(0.005),
        max: F64(1000.0),
        white: F64(203.0),
    };

    pub const WINDOWS_SCRGB: Self = Self {
        min: Self::ST2084_PQ.min,
        max: Self::ST2084_PQ.max,
        // This causes the white balance formula (with target ST2084_PQ) to simplify to
        // `Y * 80 / 10000`, meaning that sRGB pure white maps to a luminance of
        // 80 cd/m^2.
        white: F64(Self::ST2084_PQ.white.0 / 80.0 * Self::ST2084_PQ.max.0),
    };
}

impl Luminance {
    pub fn to_target(&self) -> TargetLuminance {
        TargetLuminance {
            min: self.min,
            max: self.max,
        }
    }
}

impl Default for Luminance {
    fn default() -> Self {
        Self::SRGB
    }
}

#[expect(non_snake_case)]
pub fn white_balance(from: &Luminance, to: &Luminance, w_to: (F64, F64)) -> ColorMatrix<Xyz, Xyz> {
    let a = ((from.max - from.min) / (to.max - to.min) * (to.white - from.min)
        / (from.white - from.min))
        .0;
    let d = ((from.min - to.min) / (to.max - to.min)).0.max(0.0);
    let s = a - d;
    let (F64(x_to), F64(y_to)) = w_to;
    let X_to = x_to / y_to;
    let Y_to = 1.0;
    let Z_to = (1.0 - x_to - y_to) / y_to;
    ColorMatrix::new([
        [s, 0.0, 0.0, d * X_to],
        [0.0, s, 0.0, d * Y_to],
        [0.0, 0.0, s, d * Z_to],
    ])
}
