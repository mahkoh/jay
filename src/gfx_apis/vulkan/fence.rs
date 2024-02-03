use {
    crate::gfx_apis::vulkan::{device::VulkanDevice, VulkanError},
    ash::vk::{
        ExportFenceCreateInfo, ExternalFenceHandleTypeFlags, Fence, FenceCreateInfo,
        FenceGetFdInfoKHR,
    },
    std::rc::Rc,
    uapi::OwnedFd,
};

pub struct VulkanFence {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) fence: Fence,
}

impl Drop for VulkanFence {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_fence(self.fence, None);
        }
    }
}

impl VulkanDevice {
    pub fn create_fence(self: &Rc<Self>) -> Result<Rc<VulkanFence>, VulkanError> {
        let fence = {
            let mut export_info = ExportFenceCreateInfo::builder()
                .handle_types(ExternalFenceHandleTypeFlags::SYNC_FD);
            let create_info = FenceCreateInfo::builder().push_next(&mut export_info);
            let fence = unsafe { self.device.create_fence(&create_info, None) };
            fence.map_err(VulkanError::CreateFence)?
        };
        Ok(Rc::new(VulkanFence {
            device: self.clone(),
            fence,
        }))
    }
}

impl VulkanFence {
    pub fn export_syncfile(&self) -> Result<Rc<OwnedFd>, VulkanError> {
        let info = FenceGetFdInfoKHR::builder()
            .fence(self.fence)
            .handle_type(ExternalFenceHandleTypeFlags::SYNC_FD);
        let res = unsafe { self.device.external_fence_fd.get_fence_fd(&info) };
        res.map_err(VulkanError::ExportSyncFile)
            .map(|fd| Rc::new(OwnedFd::new(fd)))
    }
}
