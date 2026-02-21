use crate::gfx_api::AlphaMode;

pub const AM_PREMULTIPLIED_ELECTRICAL: u32 = 0;
pub const AM_PREMULTIPLIED_OPTICAL: u32 = 1;
pub const AM_STRAIGHT: u32 = 2;

impl AlphaMode {
    pub fn to_vulkan(self) -> u32 {
        match self {
            AlphaMode::PremultipliedElectrical => AM_PREMULTIPLIED_ELECTRICAL,
            AlphaMode::PremultipliedOptical => AM_PREMULTIPLIED_OPTICAL,
            AlphaMode::Straight => AM_STRAIGHT,
        }
    }
}
