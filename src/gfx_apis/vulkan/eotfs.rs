use {crate::cmm::cmm_eotf::Eotf, linearize::Linearize};

pub const EOTF_LINEAR: u32 = 1;
pub const EOTF_ST2084_PQ: u32 = 2;
pub const EOTF_GAMMA24: u32 = 3;
pub const EOTF_GAMMA22: u32 = 4;
pub const EOTF_GAMMA28: u32 = 5;
pub const EOTF_ST240: u32 = 6;
pub const EOTF_LOG100: u32 = 8;
pub const EOTF_LOG316: u32 = 9;
pub const EOTF_ST428: u32 = 10;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Linearize)]
pub enum VulkanEotf {
    Linear,
    St2084Pq,
    Bt1886,
    Gamma22,
    Gamma28,
    St240,
    Log100,
    Log316,
    St428,
}

pub trait EotfExt: Sized {
    fn to_vulkan(self) -> VulkanEotf;
}

impl EotfExt for Eotf {
    fn to_vulkan(self) -> VulkanEotf {
        macro_rules! map {
            ($($name:ident,)*) => {
                match self {
                    $(
                        Self::$name { .. } => VulkanEotf::$name,
                    )*
                }
            };
        }
        map! {
            Linear,
            St2084Pq,
            Bt1886,
            Gamma22,
            Gamma28,
            St240,
            Log100,
            Log316,
            St428,
        }
    }
}

impl VulkanEotf {
    pub fn to_vulkan(self) -> u32 {
        match self {
            Self::Linear => EOTF_LINEAR,
            Self::St2084Pq => EOTF_ST2084_PQ,
            Self::Bt1886 => EOTF_GAMMA24,
            Self::Gamma22 => EOTF_GAMMA22,
            Self::Gamma28 => EOTF_GAMMA28,
            Self::St240 => EOTF_ST240,
            Self::Log100 => EOTF_LOG100,
            Self::Log316 => EOTF_LOG316,
            Self::St428 => EOTF_ST428,
        }
    }
}
