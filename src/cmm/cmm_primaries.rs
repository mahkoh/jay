use {crate::utils::ordered_float::F64, std::hash::Hash};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum NamedPrimaries {
    Srgb,
    #[expect(dead_code)]
    PalM,
    #[expect(dead_code)]
    Pal,
    #[expect(dead_code)]
    Ntsc,
    #[expect(dead_code)]
    GenericFilm,
    #[expect(dead_code)]
    Bt2020,
    #[expect(dead_code)]
    Cie1931Xyz,
    #[expect(dead_code)]
    DciP3,
    #[expect(dead_code)]
    DisplayP3,
    #[expect(dead_code)]
    AdobeRgb,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Primaries {
    pub r: (F64, F64),
    pub g: (F64, F64),
    pub b: (F64, F64),
    pub wp: (F64, F64),
}

impl Primaries {
    pub const SRGB: Self = Self {
        r: (F64(0.64), F64(0.33)),
        g: (F64(0.3), F64(0.6)),
        b: (F64(0.15), F64(0.06)),
        wp: (F64(0.3127), F64(0.3290)),
    };

    pub const PAL_M: Self = Self {
        r: (F64(0.67), F64(0.33)),
        g: (F64(0.21), F64(0.71)),
        b: (F64(0.14), F64(0.08)),
        wp: (F64(0.310), F64(0.316)),
    };

    pub const PAL: Self = Self {
        r: (F64(0.64), F64(0.33)),
        g: (F64(0.29), F64(0.60)),
        b: (F64(0.15), F64(0.06)),
        wp: (F64(0.3127), F64(0.3290)),
    };

    pub const NTSC: Self = Self {
        r: (F64(0.630), F64(0.340)),
        g: (F64(0.310), F64(0.595)),
        b: (F64(0.155), F64(0.070)),
        wp: (F64(0.3127), F64(0.3290)),
    };

    pub const GENERIC_FILM: Self = Self {
        r: (F64(0.681), F64(0.319)),
        g: (F64(0.243), F64(0.692)),
        b: (F64(0.145), F64(0.049)),
        wp: (F64(0.310), F64(0.316)),
    };

    pub const BT2020: Self = Self {
        r: (F64(0.708), F64(0.292)),
        g: (F64(0.170), F64(0.797)),
        b: (F64(0.131), F64(0.046)),
        wp: (F64(0.3127), F64(0.3290)),
    };

    pub const CIE1931_XYZ: Self = Self {
        r: (F64(1.0), F64(0.0)),
        g: (F64(0.0), F64(1.0)),
        b: (F64(0.0), F64(0.0)),
        wp: (F64(1.0 / 3.0), F64(1.0 / 3.0)),
    };

    pub const DCI_P3: Self = Self {
        r: (F64(0.680), F64(0.320)),
        g: (F64(0.265), F64(0.690)),
        b: (F64(0.150), F64(0.060)),
        wp: (F64(0.314), F64(0.351)),
    };

    pub const DISPLAY_P3: Self = Self {
        r: (F64(0.680), F64(0.320)),
        g: (F64(0.265), F64(0.690)),
        b: (F64(0.150), F64(0.060)),
        wp: (F64(0.3127), F64(0.3290)),
    };

    pub const ADOBE_RGB: Self = Self {
        r: (F64(0.64), F64(0.33)),
        g: (F64(0.21), F64(0.71)),
        b: (F64(0.15), F64(0.06)),
        wp: (F64(0.3127), F64(0.3290)),
    };
}
impl NamedPrimaries {
    #[expect(dead_code)]
    pub const fn primaries(self) -> Primaries {
        match self {
            NamedPrimaries::Srgb => Primaries::SRGB,
            NamedPrimaries::PalM => Primaries::PAL_M,
            NamedPrimaries::Pal => Primaries::PAL,
            NamedPrimaries::Ntsc => Primaries::NTSC,
            NamedPrimaries::GenericFilm => Primaries::GENERIC_FILM,
            NamedPrimaries::Bt2020 => Primaries::BT2020,
            NamedPrimaries::Cie1931Xyz => Primaries::CIE1931_XYZ,
            NamedPrimaries::DciP3 => Primaries::DCI_P3,
            NamedPrimaries::DisplayP3 => Primaries::DISPLAY_P3,
            NamedPrimaries::AdobeRgb => Primaries::ADOBE_RGB,
        }
    }
}
