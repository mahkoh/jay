use {
    crate::{
        gfx_apis::vulkan::{device::VulkanDevice, util::OnDrop, VulkanError},
        utils::oserror::OsError,
        video::drm::syncobj::SyncObj,
    },
    ash::vk::{
        ExternalSemaphoreHandleTypeFlags, ImportSemaphoreFdInfoKHR, Semaphore, SemaphoreCreateInfo,
        SemaphoreType, SemaphoreTypeCreateInfoKHR,
    },
    std::rc::Rc,
};

pub struct VulkanTimelineSemaphore {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) semaphore: Semaphore,
}

impl Drop for VulkanTimelineSemaphore {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_semaphore(self.semaphore, None);
        }
    }
}

impl VulkanDevice {
    pub fn create_timeline_semaphore(
        self: &Rc<Self>,
        syncobj: &SyncObj,
    ) -> Result<Rc<VulkanTimelineSemaphore>, VulkanError> {
        let fd = uapi::fcntl_dupfd_cloexec(syncobj.fd().raw(), 0)
            .map_err(OsError::from)
            .map_err(VulkanError::Dupfd)?;
        let sem = {
            let mut type_create_info =
                SemaphoreTypeCreateInfoKHR::builder().semaphore_type(SemaphoreType::TIMELINE);
            let create_info = SemaphoreCreateInfo::builder().push_next(&mut type_create_info);
            let sem = unsafe { self.device.create_semaphore(&create_info, None) };
            sem.map_err(VulkanError::CreateSemaphore)?
        };
        let destroy_semaphore = OnDrop(|| unsafe { self.device.destroy_semaphore(sem, None) });
        {
            let fd_info = ImportSemaphoreFdInfoKHR::builder()
                .fd(fd.raw())
                .handle_type(ExternalSemaphoreHandleTypeFlags::OPAQUE_FD)
                .semaphore(sem);
            let res = unsafe { self.external_semaphore_fd.import_semaphore_fd(&fd_info) };
            res.map_err(VulkanError::ImportSyncObj)?;
        }
        fd.unwrap();
        destroy_semaphore.forget();
        Ok(Rc::new(VulkanTimelineSemaphore {
            device: self.clone(),
            semaphore: sem,
        }))
    }
}
