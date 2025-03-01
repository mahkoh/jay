use crate::cmm::cmm_transfer_function::TransferFunction;

pub const TF_SRGB: u32 = 0;
pub const TF_LINEAR: u32 = 1;

pub trait TransferFunctionExt: Sized {
    fn to_vulkan(self) -> u32;
}

impl TransferFunctionExt for TransferFunction {
    fn to_vulkan(self) -> u32 {
        match self {
            TransferFunction::Srgb => TF_SRGB,
            TransferFunction::Linear => TF_LINEAR,
        }
    }
}
