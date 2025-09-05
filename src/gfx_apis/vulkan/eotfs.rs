use crate::cmm::cmm_eotf::Eotf;

pub const EOTF_LINEAR: u32 = 1;
pub const EOTF_ST2084_PQ: u32 = 2;
pub const EOTF_GAMMA24: u32 = 3;
pub const EOTF_GAMMA22: u32 = 4;
pub const EOTF_GAMMA28: u32 = 5;
pub const EOTF_ST240: u32 = 6;
pub const EOTF_LOG100: u32 = 8;
pub const EOTF_LOG316: u32 = 9;
pub const EOTF_ST428: u32 = 10;

pub trait EotfExt: Sized {
    fn to_vulkan(self) -> u32;
}

impl EotfExt for Eotf {
    fn to_vulkan(self) -> u32 {
        match self {
            Eotf::Linear => EOTF_LINEAR,
            Eotf::St2084Pq => EOTF_ST2084_PQ,
            Eotf::Bt1886 => EOTF_GAMMA24,
            Eotf::Gamma22 => EOTF_GAMMA22,
            Eotf::Gamma28 => EOTF_GAMMA28,
            Eotf::St240 => EOTF_ST240,
            Eotf::Log100 => EOTF_LOG100,
            Eotf::Log316 => EOTF_LOG316,
            Eotf::St428 => EOTF_ST428,
        }
    }
}
