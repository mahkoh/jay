use crate::{
    ifs::color_management::{
        ABSOLUTE_NO_ADAPTATION_SINCE, RENDER_INTENT_ABSOLUTE_NO_ADAPTATION,
        RENDER_INTENT_PERCEPTUAL, RENDER_INTENT_RELATIVE, RENDER_INTENT_RELATIVE_BPC,
    },
    object::Version,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub enum RenderIntent {
    #[default]
    Perceptual,
    Relative,
    RelativeBpc,
    AbsoluteNoAdaptation,
}

impl RenderIntent {
    pub fn from_wayland(intent: u32, version: Version) -> Option<Self> {
        let res = match intent {
            RENDER_INTENT_PERCEPTUAL => Self::Perceptual,
            RENDER_INTENT_RELATIVE => Self::Relative,
            RENDER_INTENT_RELATIVE_BPC => Self::RelativeBpc,
            RENDER_INTENT_ABSOLUTE_NO_ADAPTATION if version >= ABSOLUTE_NO_ADAPTATION_SINCE => {
                Self::AbsoluteNoAdaptation
            }
            _ => return None,
        };
        Some(res)
    }

    pub fn black_point_compensation(self) -> bool {
        match self {
            RenderIntent::Perceptual => true,
            RenderIntent::RelativeBpc => true,
            RenderIntent::Relative => false,
            RenderIntent::AbsoluteNoAdaptation => false,
        }
    }

    pub fn bradford_adjustment(self) -> bool {
        match self {
            RenderIntent::Perceptual => true,
            RenderIntent::RelativeBpc => true,
            RenderIntent::Relative => true,
            RenderIntent::AbsoluteNoAdaptation => false,
        }
    }
}
