use {
    crate::{
        gfx_api::SyncFile,
        vulkan_core::{VulkanCoreError, device::VulkanDeviceInf},
    },
    ash::vk::{
        ExportFenceCreateInfo, ExternalFenceHandleTypeFlags, Fence, FenceCreateInfo,
        FenceGetFdInfoKHR,
    },
    std::rc::Rc,
    uapi::OwnedFd,
};

pub struct VulkanFence<D>
where
    D: VulkanDeviceInf,
{
    pub device: Rc<D>,
    pub fence: Fence,
}

pub trait VulkanDeviceFenceExt: VulkanDeviceInf {
    fn create_fence(self: &Rc<Self>) -> Result<Rc<VulkanFence<Self>>, VulkanCoreError>;
}

impl<D> Drop for VulkanFence<D>
where
    D: VulkanDeviceInf,
{
    fn drop(&mut self) {
        unsafe {
            self.device.device().destroy_fence(self.fence, None);
        }
    }
}

impl<D> VulkanDeviceFenceExt for D
where
    D: VulkanDeviceInf,
{
    fn create_fence(self: &Rc<Self>) -> Result<Rc<VulkanFence<Self>>, VulkanCoreError> {
        let fence = {
            let mut export_info = ExportFenceCreateInfo::default()
                .handle_types(ExternalFenceHandleTypeFlags::SYNC_FD);
            let create_info = FenceCreateInfo::default().push_next(&mut export_info);
            let fence = unsafe { self.device().create_fence(&create_info, None) };
            fence.map_err(VulkanCoreError::CreateFence)?
        };
        Ok(Rc::new(VulkanFence {
            device: self.clone(),
            fence,
        }))
    }
}

impl<D> VulkanFence<D>
where
    D: VulkanDeviceInf,
{
    pub fn export_sync_file(&self) -> Result<Option<SyncFile>, VulkanCoreError> {
        let info = FenceGetFdInfoKHR::default()
            .fence(self.fence)
            .handle_type(ExternalFenceHandleTypeFlags::SYNC_FD);
        let res = unsafe { self.device.external_fence_fd().get_fence_fd(&info) };
        let fd = res.map_err(VulkanCoreError::ExportSyncFile)?;
        if fd == -1 {
            Ok(None)
        } else {
            Ok(Some(SyncFile(Rc::new(OwnedFd::new(fd)))))
        }
    }
}
