use ash::{Device, khr::external_fence_fd};

pub trait VulkanDeviceInf: Sized {
    fn device(&self) -> &Device;
    fn external_fence_fd(&self) -> &external_fence_fd::Device;
}
