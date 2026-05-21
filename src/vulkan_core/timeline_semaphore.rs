use {
    crate::{
        syncobj::SyncobjCtx,
        utils::{errorfmt::ErrorFmt, numcell::NumCell},
        video::drm::syncobj::Syncobj,
        vulkan_core::{VulkanCoreError, device::VulkanDeviceInf},
    },
    ash::vk::{
        ExportSemaphoreCreateInfo, ExternalSemaphoreHandleTypeFlags, Semaphore,
        SemaphoreCreateInfo, SemaphoreGetFdInfoKHR, SemaphoreSignalInfo, SemaphoreType,
        SemaphoreTypeCreateInfo,
    },
    run_on_drop::on_drop,
    std::rc::Rc,
    uapi::OwnedFd,
};

pub struct VulkanTimelineSemaphore<D>
where
    D: VulkanDeviceInf,
{
    pub(super) device: Rc<D>,
    pub(super) semaphore: Semaphore,
    pub(super) sync_ctx: Rc<SyncobjCtx>,
    pub(super) syncobj: Rc<Syncobj>,
    pub(super) next_point: NumCell<u64>,
}

impl<D> Drop for VulkanTimelineSemaphore<D>
where
    D: VulkanDeviceInf,
{
    fn drop(&mut self) {
        unsafe {
            self.device.device().destroy_semaphore(self.semaphore, None);
        }
    }
}

pub trait VulkanDeviceTimelineSemaphoreExt: VulkanDeviceInf {
    fn create_timeline_semaphore(
        self: &Rc<Self>,
    ) -> Result<Rc<VulkanTimelineSemaphore<Self>>, VulkanCoreError>;

    fn create_timeline_semaphore_or_log(
        self: &Rc<Self>,
    ) -> Option<Rc<VulkanTimelineSemaphore<Self>>>;
}

impl<D> VulkanDeviceTimelineSemaphoreExt for D
where
    D: VulkanDeviceInf,
{
    fn create_timeline_semaphore(
        self: &Rc<Self>,
    ) -> Result<Rc<VulkanTimelineSemaphore<Self>>, VulkanCoreError> {
        let Some(sync_ctx) = self.sync_ctx() else {
            return Err(VulkanCoreError::NoSyncobjCtx);
        };
        let create = |ty: ExternalSemaphoreHandleTypeFlags| {
            let sem = {
                let mut export_info = ExportSemaphoreCreateInfo::default().handle_types(ty);
                let mut type_info =
                    SemaphoreTypeCreateInfo::default().semaphore_type(SemaphoreType::TIMELINE);
                let info = SemaphoreCreateInfo::default()
                    .push_next(&mut export_info)
                    .push_next(&mut type_info);
                let sem = unsafe { self.device().create_semaphore(&info, None) };
                sem.map_err(VulkanCoreError::CreateSemaphore)?
            };
            let destroy_sem =
                on_drop(move || unsafe { self.device().destroy_semaphore(sem, None) });
            let syncobj = {
                let info = SemaphoreGetFdInfoKHR::default()
                    .semaphore(sem)
                    .handle_type(ty);
                let res = unsafe { self.external_semaphore_fd().get_semaphore_fd(&info) };
                let fd = res.map_err(VulkanCoreError::ExportTimelineSemaphore)?;
                Syncobj::new(&Rc::new(OwnedFd::new(fd)))
            };
            Ok((sem, destroy_sem, syncobj))
        };
        let next_point = NumCell::new(1);
        if self.supports_syncobj_export() {
            let (sem, destroy_sem, sync_obj) =
                create(ExternalSemaphoreHandleTypeFlags::DRM_SYNCOBJ_EXT)?;
            destroy_sem.forget();
            return Ok(Rc::new(VulkanTimelineSemaphore {
                device: self.clone(),
                semaphore: sem,
                sync_ctx: sync_ctx.clone(),
                syncobj: Rc::new(sync_obj),
                next_point,
            }));
        }
        if !self.supports_timeline_opaque_export() {
            return Err(VulkanCoreError::TimelineExportNotSupported);
        }
        let (sem, destroy_sem, syncobj) = create(ExternalSemaphoreHandleTypeFlags::OPAQUE_FD)?;
        let signal = |p| {
            let info = SemaphoreSignalInfo::default().semaphore(sem).value(p);
            unsafe {
                self.device()
                    .signal_semaphore(&info)
                    .map_err(VulkanCoreError::SignalSemaphore)
            }
        };
        for _ in 0..2 {
            let n = next_point.fetch_add(1);
            signal(n)?;
            let signaled = sync_ctx
                .query_last_signaled(&syncobj)
                .map_err(VulkanCoreError::QueryLastSignaled)?;
            if signaled != n {
                return Err(VulkanCoreError::UnsupportedPointMapping);
            }
        }
        destroy_sem.forget();
        Ok(Rc::new(VulkanTimelineSemaphore {
            device: self.clone(),
            semaphore: sem,
            sync_ctx: sync_ctx.clone(),
            syncobj: Rc::new(syncobj),
            next_point,
        }))
    }

    fn create_timeline_semaphore_or_log(
        self: &Rc<Self>,
    ) -> Option<Rc<VulkanTimelineSemaphore<Self>>> {
        self.create_timeline_semaphore()
            .inspect_err(|e| {
                log::warn!("Could not create timeline semaphore: {}", ErrorFmt(e));
            })
            .ok()
    }
}
