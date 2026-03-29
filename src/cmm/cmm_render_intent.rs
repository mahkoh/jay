use crate::{ifs::color_management::RENDER_INTENT_PERCEPTUAL, object::Version};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
pub enum RenderIntent {
    #[default]
    Perceptual,
}

impl RenderIntent {
    pub fn from_wayland(intent: u32, _version: Version) -> Option<Self> {
        let res = match intent {
            RENDER_INTENT_PERCEPTUAL => Self::Perceptual,
            _ => return None,
        };
        Some(res)
    }

    pub fn black_point_compensation(self) -> bool {
        match self {
            RenderIntent::Perceptual => true,
        }
    }

    pub fn bradford_adjustment(self) -> bool {
        match self {
            RenderIntent::Perceptual => true,
        }
    }
}
