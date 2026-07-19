use crate::eventfd_cache::EventfdCache;
use crate::syncobj::SyncobjCtx;
use crate::vulkan_core::VulkanCoreInstance;
use ash::Device;
use ash::khr::external_fence_fd;
use ash::khr::external_semaphore_fd;
use std::rc::Rc;

pub trait VulkanDeviceInf: Sized {
    fn instance(&self) -> &VulkanCoreInstance;
    fn device(&self) -> &Device;
    fn external_fence_fd(&self) -> &external_fence_fd::Device;
    fn external_semaphore_fd(&self) -> &external_semaphore_fd::Device;
    fn supports_timeline_opaque_export(&self) -> bool;
    fn sync_ctx(&self) -> Option<&Rc<SyncobjCtx>>;
    fn eventfd_cache(&self) -> &Rc<EventfdCache>;
}
