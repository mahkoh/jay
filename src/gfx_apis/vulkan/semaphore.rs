use {
    crate::gfx_apis::vulkan::{device::VulkanDevice, VulkanError},
    ash::vk::{
        ExternalSemaphoreHandleTypeFlags, ImportSemaphoreFdInfoKHR, Semaphore, SemaphoreCreateInfo,
        SemaphoreImportFlags,
    },
    std::{mem, rc::Rc},
    uapi::OwnedFd,
};

pub struct VulkanSemaphore {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) semaphore: Semaphore,
}

impl Drop for VulkanSemaphore {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_semaphore(self.semaphore, None);
        }
    }
}

impl VulkanDevice {
    pub fn create_semaphore(self: &Rc<Self>) -> Result<Rc<VulkanSemaphore>, VulkanError> {
        let sem = {
            let create_info = SemaphoreCreateInfo::builder();
            let sem = unsafe { self.device.create_semaphore(&create_info, None) };
            sem.map_err(VulkanError::CreateSemaphore)?
        };
        Ok(Rc::new(VulkanSemaphore {
            device: self.clone(),
            semaphore: sem,
        }))
    }
}

impl VulkanSemaphore {
    pub fn import_syncfile(&self, syncfile: OwnedFd) -> Result<(), VulkanError> {
        let fd_info = ImportSemaphoreFdInfoKHR::builder()
            .fd(syncfile.raw())
            .flags(SemaphoreImportFlags::TEMPORARY)
            .handle_type(ExternalSemaphoreHandleTypeFlags::SYNC_FD)
            .semaphore(self.semaphore);
        let res = unsafe {
            self.device
                .external_semaphore_fd
                .import_semaphore_fd(&fd_info)
        };
        mem::forget(syncfile);
        res.map_err(VulkanError::ImportSyncFile)?;
        Ok(())
    }
}
