use {
    crate::{
        gfx_api::{FdSync, ReservedSyncobjPoint},
        utils::errorfmt::ErrorFmt,
        video::drm::syncobj::SyncobjPoint,
        vulkan_core::{
            VulkanCoreError,
            device::VulkanDeviceInf,
            fence::{VulkanDeviceFenceExt, VulkanFence},
            timeline_semaphore::VulkanTimelineSemaphore,
        },
    },
    ash::vk::{Fence, PipelineStageFlags2, SemaphoreSubmitInfo, SemaphoreWaitInfo, SubmitInfo2},
    std::{rc::Rc, slice},
};

pub enum VulkanSync<D>
where
    D: VulkanDeviceInf,
{
    Fence(Rc<VulkanFence<D>>),
    TimelineSemaphore {
        tls: Rc<VulkanTimelineSemaphore<D>>,
        pending: Rc<ReservedSyncobjPoint>,
    },
}

impl<D> VulkanSync<D>
where
    D: VulkanDeviceInf,
{
    pub fn handle_validation(&self) {
        if let VulkanSync::TimelineSemaphore { tls, pending } = self
            && tls.device.instance().validation
        {
            let info = SemaphoreWaitInfo::default()
                .semaphores(slice::from_ref(&tls.semaphore))
                .values(slice::from_ref(&pending.point.0));
            unsafe {
                let _ = tls.device.device().wait_semaphores(&info, 0);
            }
        }
    }

    pub fn fence(&self) -> Fence {
        match self {
            VulkanSync::Fence(f) => f.fence,
            VulkanSync::TimelineSemaphore { .. } => Fence::null(),
        }
    }

    pub fn to_sync(&self, block: impl FnOnce()) -> Option<FdSync> {
        match self {
            VulkanSync::Fence(release_fence) => {
                zone!("export_sync_file");
                let release_sync_file = match release_fence.export_sync_file() {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!("Could not export sync file from fence: {}", ErrorFmt(e));
                        block();
                        None
                    }
                };
                release_sync_file.map(FdSync::SyncFile)
            }
            VulkanSync::TimelineSemaphore { pending, .. } => Some(FdSync::Syncobj(pending.clone())),
        }
    }
}

pub trait VulkanDeviceSyncExt: VulkanDeviceInf {
    fn create_sync<'a>(
        self: &Rc<Self>,
        tls: Option<&Rc<VulkanTimelineSemaphore<Self>>>,
        semaphore_submit_info: &'a mut SemaphoreSubmitInfo,
        submit_info: &mut SubmitInfo2<'a>,
    ) -> Result<VulkanSync<Self>, VulkanCoreError>;
}

impl<D> VulkanDeviceSyncExt for D
where
    D: VulkanDeviceInf,
{
    fn create_sync<'a>(
        self: &Rc<Self>,
        tls: Option<&Rc<VulkanTimelineSemaphore<Self>>>,
        semaphore_submit_info: &'a mut SemaphoreSubmitInfo,
        submit_info: &mut SubmitInfo2<'a>,
    ) -> Result<VulkanSync<Self>, VulkanCoreError> {
        if let Some(tls) = tls {
            match create_tls_sync(self, tls, semaphore_submit_info, submit_info) {
                Ok(s) => return Ok(s),
                Err(e) => {
                    log::warn!("Could not create sync obj sync: {}", ErrorFmt(e));
                }
            }
        }
        self.create_fence().map(VulkanSync::Fence)
    }
}

fn create_tls_sync<'a, D>(
    device: &Rc<D>,
    tls: &Rc<VulkanTimelineSemaphore<D>>,
    semaphore_submit_info: &'a mut SemaphoreSubmitInfo,
    submit_info: &mut SubmitInfo2<'a>,
) -> Result<VulkanSync<D>, VulkanCoreError>
where
    D: VulkanDeviceInf,
{
    let point = SyncobjPoint(tls.next_point.fetch_add(1));
    let eventfd = device
        .eventfd_cache()
        .acquire()
        .map_err(VulkanCoreError::AcquireEventfd)?;
    tls.sync_ctx
        .wait_for_point(&eventfd.fd, &tls.syncobj, point, true)
        .map_err(VulkanCoreError::CreateSyncobjWait)?;
    let pending = Rc::new(ReservedSyncobjPoint {
        ctx: tls.sync_ctx.clone(),
        syncobj: tls.syncobj.clone(),
        point,
        sync_file: Default::default(),
        signaled: eventfd,
    });
    *semaphore_submit_info = SemaphoreSubmitInfo::default()
        .semaphore(tls.semaphore)
        .value(point.0)
        .stage_mask(PipelineStageFlags2::ALL_COMMANDS);
    *submit_info = submit_info.signal_semaphore_infos(slice::from_ref(semaphore_submit_info));
    Ok(VulkanSync::TimelineSemaphore {
        tls: tls.clone(),
        pending,
    })
}
