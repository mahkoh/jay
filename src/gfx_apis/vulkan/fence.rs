use {
    crate::{
        gfx_api::SyncFile,
        gfx_apis::vulkan::{VulkanError, device::VulkanDevice},
    },
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
            let mut export_info = ExportFenceCreateInfo::default()
                .handle_types(ExternalFenceHandleTypeFlags::SYNC_FD);
            let create_info = FenceCreateInfo::default().push_next(&mut export_info);
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
    pub fn export_sync_file(&self) -> Result<Option<SyncFile>, VulkanError> {
        let info = FenceGetFdInfoKHR::default()
            .fence(self.fence)
            .handle_type(ExternalFenceHandleTypeFlags::SYNC_FD);
        let res = unsafe { self.device.external_fence_fd.get_fence_fd(&info) };
        let fd = res.map_err(VulkanError::ExportSyncFile)?;
        if fd == -1 {
            Ok(None)
        } else {
            Ok(Some(SyncFile(Rc::new(OwnedFd::new(fd)))))
        }
    }
}
