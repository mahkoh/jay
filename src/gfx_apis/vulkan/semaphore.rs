use {
    crate::gfx_apis::vulkan::{VulkanError, device::VulkanDevice},
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
            let create_info = SemaphoreCreateInfo::default();
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
    pub fn import_sync_file(&self, sync_file: OwnedFd) -> Result<(), VulkanError> {
        zone!("import_sync_file");
        let fd_info = ImportSemaphoreFdInfoKHR::default()
            .fd(sync_file.raw())
            .flags(SemaphoreImportFlags::TEMPORARY)
            .handle_type(ExternalSemaphoreHandleTypeFlags::SYNC_FD)
            .semaphore(self.semaphore);
        let res = unsafe {
            self.device
                .external_semaphore_fd
                .import_semaphore_fd(&fd_info)
        };
        res.map_err(VulkanError::ImportSyncFile)?;
        mem::forget(sync_file);
        Ok(())
    }
}
