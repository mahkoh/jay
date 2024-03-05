use jay_config::video::GfxApi;

pub trait GfxApiExt: Sized {
    fn to_str(&self) -> &'static str;

    fn from_str_lossy(s: &str) -> Option<Self>;
}

impl GfxApiExt for GfxApi {
    fn to_str(&self) -> &'static str {
        match self {
            GfxApi::OpenGl => "OpenGl",
            GfxApi::Vulkan => "Vulkan",
            _ => "unknown",
        }
    }

    fn from_str_lossy(s: &str) -> Option<Self> {
        match &*s.to_ascii_lowercase() {
            "opengl" => Some(Self::OpenGl),
            "vulkan" => Some(Self::Vulkan),
            _ => None,
        }
    }
}
