use {
    crate::{
        cpu_worker::{
            CpuJob, CpuWork, CpuWorker,
            jobs::{
                img_copy::ImgCopyWork,
                read_write::{ReadWriteJobError, ReadWriteWork},
            },
        },
        gfx_api::{
            AsyncShmGfxTextureCallback, PendingShmTransfer, ShmMemory, ShmMemoryBacking, SyncFile,
        },
        gfx_apis::vulkan::{
            VulkanError,
            command::VulkanCommandBuffer,
            fence::VulkanFence,
            image::{QueueFamily, QueueState, QueueTransfer, VulkanImage, VulkanImageMemory},
            renderer::image_barrier,
            shm_image::VulkanShmImage,
            staging::{VulkanStagingBuffer, VulkanStagingShell},
        },
        rect::{Rect, Region},
        utils::{clonecell::CloneCell, errorfmt::ErrorFmt},
    },
    arrayvec::ArrayVec,
    ash::vk::{
        AccessFlags2, BufferImageCopy2, CommandBufferBeginInfo, CommandBufferSubmitInfo,
        CommandBufferUsageFlags, DependencyInfo, Extent3D, ImageAspectFlags, ImageLayout,
        ImageSubresourceLayers, Offset3D, PipelineStageFlags2, SubmitInfo2,
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
        slice,
    },
    uapi::OwnedFd,
};

pub struct VulkanShmImageAsyncData {
    pub(super) busy: Cell<bool>,
    pub(super) io_job: Cell<Option<Box<IoTransferJob>>>,
    pub(super) copy_job: Cell<Option<Box<CopyTransferJob>>>,
    pub(super) staging: CloneCell<Option<Rc<VulkanStagingShell>>>,
    pub(super) client_mem: CloneCell<Option<Rc<dyn ShmMemory>>>,
    pub(super) callback: Cell<Option<Rc<dyn AsyncShmGfxTextureCallback>>>,
    pub(super) callback_id: Cell<u64>,
    pub(super) regions: RefCell<Vec<BufferImageCopy2<'static>>>,
    pub(super) cpu: Rc<CpuWorker>,
    pub(super) last_gfx_use: Cell<Option<SyncFile>>,
    pub(super) data_copied: Cell<bool>,
}

impl VulkanShmImageAsyncData {
    fn complete(&self, result: Result<(), VulkanError>) {
        self.busy.set(false);
        self.staging.take().unwrap().busy.set(false);
        self.client_mem.take();
        if let Some(cb) = self.callback.take() {
            cb.completed(result.map_err(|e| e.into()));
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum TransferType {
    Upload,
    Download,
}

impl VulkanShmImage {
    pub fn async_transfer(
        &self,
        img: &Rc<VulkanImage>,
        staging: Rc<VulkanStagingShell>,
        client_mem: &Rc<dyn ShmMemory>,
        damage: Region,
        callback: Rc<dyn AsyncShmGfxTextureCallback>,
        tt: TransferType,
    ) -> Result<Option<PendingShmTransfer>, VulkanError> {
        if damage.is_empty() {
            return Ok(None);
        }
        let data = self.async_data.as_ref().unwrap();
        let res = self.try_async_transfer(img, staging, data, client_mem, damage, tt);
        match res {
            Ok(()) => {
                let id = img.renderer.allocate_point();
                data.callback_id.set(id);
                data.callback.set(Some(callback));
                Ok(Some(PendingShmTransfer::new(img.clone(), id)))
            }
            Err(e) => Err(e),
        }
    }

    fn try_async_transfer(
        &self,
        img: &Rc<VulkanImage>,
        staging: Rc<VulkanStagingShell>,
        data: &VulkanShmImageAsyncData,
        client_mem: &Rc<dyn ShmMemory>,
        mut damage: Region,
        tt: TransferType,
    ) -> Result<(), VulkanError> {
        if data.busy.get() {
            return Err(VulkanError::AsyncCopyBusy);
        }
        if staging.busy.get() {
            return Err(VulkanError::StagingBufferBusy);
        }
        match tt {
            TransferType::Upload => {
                if !staging.upload {
                    return Err(VulkanError::StagingBufferNoUpload);
                }
            }
            TransferType::Download => {
                if !staging.download {
                    return Err(VulkanError::StagingBufferNoDownload);
                }
            }
        }
        if self.size > client_mem.len() as u64 {
            return Err(VulkanError::InvalidBufferSize);
        }
        data.busy.set(true);
        data.data_copied.set(false);
        staging.busy.set(true);
        data.staging.set(Some(staging.clone()));
        data.client_mem.set(Some(client_mem.clone()));
        if img.contents_are_undefined.get() {
            if tt == TransferType::Download {
                return Err(VulkanError::UndefinedContents);
            }
            damage = Region::new(Rect::new_sized(0, 0, img.width as _, img.height as _).unwrap());
        }

        let copies = &mut *data.regions.borrow_mut();
        copies.clear();

        let mut copy = |x, y, width, height| {
            let buffer_offset = (y as u32 * img.stride + x as u32 * self.shm_info.bpp) as u64;
            let copy = BufferImageCopy2::default()
                .buffer_offset(buffer_offset)
                .image_offset(Offset3D { x, y, z: 0 })
                .image_extent(Extent3D {
                    width,
                    height,
                    depth: 1,
                })
                .image_subresource(ImageSubresourceLayers {
                    aspect_mask: ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .buffer_image_height(img.height)
                .buffer_row_length(img.stride / self.shm_info.bpp);
            copies.push(copy);
        };
        let (width_mask, height_mask) = img.renderer.device.transfer_granularity_mask;
        let width_mask = width_mask as i32;
        let height_mask = height_mask as i32;
        for damage in damage.rects() {
            if damage.x2() < 0 || damage.y2() < 0 {
                continue;
            }
            let x1 = damage.x1().max(0) & !width_mask;
            let y1 = damage.y1().max(0) & !height_mask;
            let x2 = ((damage.x2() + width_mask) & !width_mask).min(img.width as i32);
            let y2 = ((damage.y2() + height_mask) & !height_mask).min(img.height as i32);
            let Some(damage) = Rect::new(x1, y1, x2, y2) else {
                continue;
            };
            if damage.is_empty() {
                continue;
            }
            copy(
                damage.x1(),
                damage.y1(),
                damage.width() as u32,
                damage.height() as u32,
            );
        }

        self.async_release_from_gfx_queue(img, data, tt)?;

        if let Some(staging) = staging.staging.get() {
            return match tt {
                TransferType::Upload => self
                    .async_transfer_initiate_host_copy(img, data, &staging, copies, client_mem, tt),
                TransferType::Download => {
                    self.async_download_copy_image_to_buffer(img, &staging, copies)
                }
            };
        }

        let img2 = img.clone();
        let client_mem = client_mem.clone();
        img.renderer
            .device
            .fill_staging_shell(&img.renderer, &data.cpu, staging, move |res| {
                let VulkanImageMemory::Internal(shm) = &img2.ty else {
                    unreachable!();
                };
                if let Err(e) = shm.async_transfer_after_allocation(&img2, &client_mem, res, tt) {
                    shm.async_data.as_ref().unwrap().complete(Err(e));
                }
            })
    }

    fn async_release_from_gfx_queue(
        &self,
        img: &Rc<VulkanImage>,
        data: &VulkanShmImageAsyncData,
        tt: TransferType,
    ) -> Result<(), VulkanError> {
        img.renderer.check_defunct()?;
        let Some(transfer_queue_idx) = img.renderer.device.distinct_transfer_queue_family_idx
        else {
            let Some(sync_file) = data.last_gfx_use.take() else {
                img.queue_state.set(QueueState::Released {
                    to: QueueFamily::Transfer,
                });
                return Ok(());
            };
            let id = img.renderer.allocate_point();
            let pending = img.renderer.eng.spawn(
                "await_transfer_to_transfer",
                await_gfx_queue_release(id, img.clone(), None, None, Some(sync_file), tt),
            );
            img.renderer.pending_submits.set(id, pending);
            img.queue_state.set(QueueState::Releasing);
            return Ok(());
        };
        let (gfx_access_mask, gfx_layout, transfer_layout) = match tt {
            TransferType::Upload => (
                AccessFlags2::SHADER_SAMPLED_READ,
                ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                ImageLayout::TRANSFER_DST_OPTIMAL,
            ),
            TransferType::Download => (
                AccessFlags2::COLOR_ATTACHMENT_WRITE,
                ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                ImageLayout::TRANSFER_SRC_OPTIMAL,
            ),
        };
        let mut barriers = ArrayVec::<_, 2>::new();
        match img.queue_state.get() {
            QueueState::Acquired { family } => {
                assert_eq!(family, QueueFamily::Gfx);
            }
            QueueState::Releasing => {
                unreachable!();
            }
            QueueState::Released { to } => {
                assert_eq!(to, QueueFamily::Gfx);
                let barrier = image_barrier()
                    .image(img.image)
                    .src_queue_family_index(transfer_queue_idx)
                    .dst_queue_family_index(img.renderer.device.graphics_queue_idx)
                    .dst_stage_mask(PipelineStageFlags2::ALL_COMMANDS)
                    .old_layout(transfer_layout)
                    .new_layout(gfx_layout);
                barriers.push(barrier);
            }
        }
        let barrier = image_barrier()
            .image(img.image)
            .src_queue_family_index(img.renderer.device.graphics_queue_idx)
            .dst_queue_family_index(transfer_queue_idx)
            .src_access_mask(gfx_access_mask)
            .src_stage_mask(PipelineStageFlags2::ALL_COMMANDS)
            .old_layout(if img.is_undefined.get() {
                ImageLayout::UNDEFINED
            } else {
                gfx_layout
            })
            .new_layout(transfer_layout);
        barriers.push(barrier);
        let dep_info = DependencyInfo::default().image_memory_barriers(&barriers);
        let release_fence = img.renderer.device.create_fence()?;
        let dev = &img.renderer.device.device;
        let begin_info =
            CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        let cmd = img.renderer.gfx_command_buffers.allocate()?;
        let command_buffer_info = CommandBufferSubmitInfo::default().command_buffer(cmd.buffer);
        let submit_info =
            SubmitInfo2::default().command_buffer_infos(slice::from_ref(&command_buffer_info));
        unsafe {
            dev.begin_command_buffer(cmd.buffer, &begin_info)
                .map_err(VulkanError::BeginCommandBuffer)?;
            dev.cmd_pipeline_barrier2(cmd.buffer, &dep_info);
            dev.end_command_buffer(cmd.buffer)
                .map_err(VulkanError::EndCommandBuffer)?;
            dev.queue_submit2(
                img.renderer.device.graphics_queue,
                slice::from_ref(&submit_info),
                release_fence.fence,
            )
            .inspect_err(img.renderer.device.idl())
            .map_err(VulkanError::Submit)?;
        }
        let sync_file = release_fence.export_sync_file()?;
        let id = img.renderer.allocate_point();
        let pending = img.renderer.eng.spawn(
            "await_transfer_to_transfer",
            await_gfx_queue_release(
                id,
                img.clone(),
                Some(cmd),
                Some(release_fence),
                sync_file,
                tt,
            ),
        );
        img.renderer.pending_submits.set(id, pending);
        img.queue_state.set(QueueState::Releasing);
        Ok(())
    }

    fn async_transfer_after_allocation(
        &self,
        img: &Rc<VulkanImage>,
        client_mem: &Rc<dyn ShmMemory>,
        res: Result<Rc<VulkanStagingBuffer>, VulkanError>,
        tt: TransferType,
    ) -> Result<(), VulkanError> {
        let staging = res?;
        let data = self.async_data.as_ref().unwrap();
        let copies = &*data.regions.borrow();
        match tt {
            TransferType::Upload => {
                self.async_transfer_initiate_host_copy(img, data, &staging, copies, client_mem, tt)
            }
            TransferType::Download => {
                self.async_download_copy_image_to_buffer(img, &staging, copies)
            }
        }
    }

    pub(super) fn async_transfer_initiate_host_copy(
        &self,
        img: &Rc<VulkanImage>,
        data: &VulkanShmImageAsyncData,
        staging: &VulkanStagingBuffer,
        copies: &[BufferImageCopy2],
        client_mem: &Rc<dyn ShmMemory>,
        tt: TransferType,
    ) -> Result<(), VulkanError> {
        img.renderer.check_defunct()?;

        if tt == TransferType::Download {
            staging.download(|_, _| ())?;
        }
        let id = img.renderer.allocate_point();
        let pending;
        match client_mem.safe_access() {
            ShmMemoryBacking::Ptr(ptr) => {
                let mut job = data.copy_job.take().unwrap_or_else(|| {
                    Box::new(CopyTransferJob {
                        img: None,
                        id,
                        _mem: None,
                        work: unsafe { ImgCopyWork::new() },
                        tt,
                    })
                });
                job.id = id;
                job.img = Some(img.clone());
                job._mem = Some(client_mem.clone());
                job.tt = tt;
                match tt {
                    TransferType::Upload => {
                        job.work.src = ptr as _;
                        job.work.dst = staging.allocation.mem.unwrap();
                    }
                    TransferType::Download => {
                        job.work.src = staging.allocation.mem.unwrap();
                        job.work.dst = ptr as _;
                    }
                }
                job.work.width = img.width as _;
                job.work.stride = img.stride as _;
                job.work.bpp = self.shm_info.bpp as _;
                job.work.rects.clear();
                for copy in copies {
                    job.work.rects.push(
                        Rect::new_sized(
                            copy.image_offset.x as _,
                            copy.image_offset.y as _,
                            copy.image_extent.width as _,
                            copy.image_extent.height as _,
                        )
                        .unwrap(),
                    );
                }
                pending = data.cpu.submit(job);
            }
            ShmMemoryBacking::Fd(fd, offset) => {
                let mut min_offset = client_mem.len() as u64;
                let mut max_offset = 0;
                for copy in copies {
                    min_offset = min_offset.min(copy.buffer_offset);
                    let len = img.stride * (copy.image_extent.height - 1)
                        + copy.image_extent.width * self.shm_info.bpp;
                    max_offset = max_offset.max(copy.buffer_offset + len as u64);
                }
                let mut job = data.io_job.take().unwrap_or_else(|| {
                    Box::new(IoTransferJob {
                        img: None,
                        id,
                        _mem: None,
                        work: unsafe { ReadWriteWork::new() },
                        fd: None,
                        tt,
                    })
                });
                job.id = id;
                job.img = Some(img.clone());
                job._mem = Some(client_mem.clone());
                job.fd = Some(fd.clone());
                job.tt = tt;
                unsafe {
                    let config = job.work.config();
                    config.fd = fd.raw();
                    config.offset = offset + min_offset as usize;
                    config.ptr = staging.allocation.mem.unwrap().add(min_offset as _);
                    config.len = max_offset.saturating_sub(min_offset) as usize;
                    config.write = tt == TransferType::Download;
                }
                pending = data.cpu.submit(job);
            }
        }

        img.renderer.pending_cpu_jobs.set(id, pending);

        Ok(())
    }

    fn async_upload_copy_buffer_to_image(
        &self,
        img: &Rc<VulkanImage>,
        data: &VulkanShmImageAsyncData,
    ) -> Result<(), VulkanError> {
        if !data.data_copied.get() {
            return Ok(());
        }
        if img.queue_state.get().acquire(QueueFamily::Transfer) == QueueTransfer::Impossible {
            return Ok(());
        }
        img.renderer.check_defunct()?;
        let regions = &*data.regions.borrow();
        let staging = data.staging.get().unwrap().staging.get().unwrap();
        staging.upload(|_, _| ())?;
        let (cmd, fence, sync_file, point) =
            self.submit_buffer_image_copy(img, &staging, regions, true, TransferType::Upload)?;
        img.queue_state.set(QueueState::Releasing);
        let future = img.renderer.eng.spawn(
            "await async upload",
            await_async_transfer_release_to_gfx(
                point,
                img.clone(),
                cmd,
                fence,
                sync_file,
                TransferType::Upload,
            ),
        );
        img.renderer.pending_submits.set(point, future);
        Ok(())
    }

    fn async_download_copy_image_to_buffer(
        &self,
        img: &Rc<VulkanImage>,
        staging: &VulkanStagingBuffer,
        copies: &[BufferImageCopy2],
    ) -> Result<(), VulkanError> {
        if img.queue_state.get().acquire(QueueFamily::Transfer) == QueueTransfer::Impossible {
            return Ok(());
        }
        img.renderer.check_defunct()?;
        let (cmd, fence, sync_file, point) =
            self.submit_buffer_image_copy(img, &staging, copies, true, TransferType::Download)?;
        img.queue_state.set(QueueState::Releasing);
        let future = img.renderer.eng.spawn(
            "await async image to buffer copy",
            await_async_transfer_release_to_gfx(
                point,
                img.clone(),
                cmd,
                fence,
                sync_file,
                TransferType::Download,
            ),
        );
        img.renderer.pending_submits.set(point, future);
        Ok(())
    }
}

pub(super) struct IoTransferJob {
    img: Option<Rc<VulkanImage>>,
    id: u64,
    _mem: Option<Rc<dyn ShmMemory>>,
    fd: Option<Rc<OwnedFd>>,
    work: ReadWriteWork,
    tt: TransferType,
}

pub(super) struct CopyTransferJob {
    img: Option<Rc<VulkanImage>>,
    id: u64,
    _mem: Option<Rc<dyn ShmMemory>>,
    work: ImgCopyWork,
    tt: TransferType,
}

impl CpuJob for IoTransferJob {
    fn work(&mut self) -> &mut dyn CpuWork {
        &mut self.work
    }

    fn completed(mut self: Box<Self>) {
        self._mem = None;
        self.fd = None;
        let img = self.img.take().unwrap();
        let res = self.work.config().result.take().unwrap();
        complete_async_host_copy(&img, self.id, res, self.tt, |data| {
            data.io_job.set(Some(self))
        });
    }
}

impl CpuJob for CopyTransferJob {
    fn work(&mut self) -> &mut dyn CpuWork {
        &mut self.work
    }

    fn completed(mut self: Box<Self>) {
        self._mem = None;
        let img = self.img.take().unwrap();
        complete_async_host_copy(&img, self.id, Ok(()), self.tt, |data| {
            data.copy_job.set(Some(self))
        });
    }
}

fn complete_async_host_copy(
    img: &Rc<VulkanImage>,
    id: u64,
    res: Result<(), ReadWriteJobError>,
    tt: TransferType,
    store: impl FnOnce(&VulkanShmImageAsyncData),
) {
    img.renderer.pending_cpu_jobs.remove(&id);
    let VulkanImageMemory::Internal(shm) = &img.ty else {
        unreachable!();
    };
    let data = shm.async_data.as_ref().unwrap();
    store(data);
    if let Err(e) = res {
        data.complete(Err(VulkanError::AsyncCopyToStaging(e)));
    }
    data.data_copied.set(true);
    match tt {
        TransferType::Upload => {
            let res = shm.async_upload_copy_buffer_to_image(img, data);
            if let Err(e) = res {
                data.complete(Err(e));
            }
        }
        TransferType::Download => data.complete(Ok(())),
    }
}

async fn await_gfx_queue_release(
    id: u64,
    img: Rc<VulkanImage>,
    buf: Option<Rc<VulkanCommandBuffer>>,
    _fence: Option<Rc<VulkanFence>>,
    sync_file: Option<SyncFile>,
    tt: TransferType,
) {
    if let Some(sync_file) = sync_file
        && let Err(e) = img.renderer.ring.readable(&sync_file.0).await
    {
        log::error!(
            "Could not wait for sync file to become readable: {}",
            ErrorFmt(e)
        );
        img.renderer.block();
    }
    if let Some(buf) = buf {
        img.renderer.gfx_command_buffers.buffers.push(buf);
    }
    img.renderer.pending_submits.remove(&id);
    img.queue_state.set(QueueState::Released {
        to: QueueFamily::Transfer,
    });
    let VulkanImageMemory::Internal(shm) = &img.ty else {
        unreachable!();
    };
    let data = shm.async_data.as_ref().unwrap();
    let res = match tt {
        TransferType::Upload => shm.async_upload_copy_buffer_to_image(&img, data),
        TransferType::Download => match data.staging.get().unwrap().staging.get() {
            Some(staging) => {
                let copies = &*data.regions.borrow();
                shm.async_download_copy_image_to_buffer(&img, &staging, copies)
            }
            None => Ok(()),
        },
    };
    if let Err(e) = res {
        data.complete(Err(e));
    }
}

pub async fn await_async_transfer_release_to_gfx(
    id: u64,
    img: Rc<VulkanImage>,
    buf: Rc<VulkanCommandBuffer>,
    _fence: Rc<VulkanFence>,
    sync_file: Option<SyncFile>,
    tt: TransferType,
) {
    if let Some(sync_file) = sync_file
        && let Err(e) = img.renderer.ring.readable(&sync_file.0).await
    {
        log::error!(
            "Could not wait for sync file to become readable: {}",
            ErrorFmt(e)
        );
        img.renderer.block();
    }
    match &img.renderer.transfer_command_buffers {
        Some(b) => b.buffers.push(buf),
        None => img.renderer.gfx_command_buffers.buffers.push(buf),
    }
    img.queue_state.set(QueueState::Released {
        to: QueueFamily::Gfx,
    });
    img.renderer.pending_submits.remove(&id);
    let VulkanImageMemory::Internal(shm) = &img.ty else {
        unreachable!();
    };
    let data = shm.async_data.as_ref().unwrap();
    match tt {
        TransferType::Upload => {
            data.complete(Ok(()));
        }
        TransferType::Download => {
            let data = shm.async_data.as_ref().unwrap();
            let staging = data.staging.get().unwrap().staging.get().unwrap();
            let client_mem = data.client_mem.get().unwrap();
            let copies = &*data.regions.borrow();
            let res = shm.async_transfer_initiate_host_copy(
                &img,
                data,
                &staging,
                copies,
                &client_mem,
                tt,
            );
            if let Err(e) = res {
                data.complete(Err(e));
            }
        }
    }
}
