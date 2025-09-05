use crate::cmm::cmm_transfer_function::TransferFunction;

pub const TF_LINEAR: u32 = 1;
pub const TF_ST2084_PQ: u32 = 2;
pub const TF_GAMMA24: u32 = 3;
pub const TF_GAMMA22: u32 = 4;
pub const TF_GAMMA28: u32 = 5;
pub const TF_ST240: u32 = 6;
pub const TF_LOG100: u32 = 8;
pub const TF_LOG316: u32 = 9;
pub const TF_ST428: u32 = 10;

pub trait TransferFunctionExt: Sized {
    fn to_vulkan(self) -> u32;
}

impl TransferFunctionExt for TransferFunction {
    fn to_vulkan(self) -> u32 {
        match self {
            TransferFunction::Linear => TF_LINEAR,
            TransferFunction::St2084Pq => TF_ST2084_PQ,
            TransferFunction::Bt1886 => TF_GAMMA24,
            TransferFunction::Gamma22 => TF_GAMMA22,
            TransferFunction::Gamma28 => TF_GAMMA28,
            TransferFunction::St240 => TF_ST240,
            TransferFunction::Log100 => TF_LOG100,
            TransferFunction::Log316 => TF_LOG316,
            TransferFunction::St428 => TF_ST428,
        }
    }
}
