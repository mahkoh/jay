use {
    crate::{
        cpu_worker::{
            jobs::{
                img_copy::ImgCopyWork,
                read_write::{ReadWriteJobError, ReadWriteWork},
            },
            CpuJob, CpuWork, CpuWorker,
        },
        format::{Format, FormatShmInfo},
        gfx_api::{
            AsyncShmGfxTextureCallback, PendingShmUpload, ShmMemory, ShmMemoryBacking, SyncFile,
        },
        gfx_apis::vulkan::{
            allocator::VulkanAllocation,
            command::VulkanCommandBuffer,
            fence::VulkanFence,
            image::{QueueFamily, QueueState, QueueTransfer, VulkanImage, VulkanImageMemory},
            renderer::{image_barrier, VulkanRenderer},
            staging::{VulkanStagingBuffer, VulkanStagingShell},
            VulkanError,
        },
        rect::{Rect, Region},
        utils::{clonecell::CloneCell, errorfmt::ErrorFmt, on_drop::OnDrop},
    },
    arrayvec::ArrayVec,
    ash::vk::{
        AccessFlags2, BufferImageCopy2, BufferMemoryBarrier2, CommandBufferBeginInfo,
        CommandBufferSubmitInfo, CommandBufferUsageFlags, CopyBufferToImageInfo2, DependencyInfo,
        DependencyInfoKHR, DeviceSize, Extent3D, ImageAspectFlags, ImageCreateInfo, ImageLayout,
        ImageSubresourceLayers, ImageSubresourceRange, ImageTiling, ImageType, ImageUsageFlags,
        ImageViewCreateInfo, ImageViewType, Offset3D, PipelineStageFlags2, SampleCountFlags,
        SharingMode, SubmitInfo2,
    },
    gpu_alloc::UsageFlags,
    isnt::std_1::primitive::IsntSliceExt,
    std::{
        cell::{Cell, RefCell},
        ptr,
        rc::Rc,
        slice,
    },
    uapi::OwnedFd,
};

pub struct VulkanShmImage {
    pub(super) size: DeviceSize,
    pub(super) stride: u32,
    pub(super) _allocation: VulkanAllocation,
    pub(super) shm_info: &'static FormatShmInfo,
    pub(super) async_data: Option<VulkanShmImageAsyncData>,
}

pub struct VulkanShmImageAsyncData {
    pub(super) busy: Cell<bool>,
    pub(super) io_job: Cell<Option<Box<IoUploadJob>>>,
    pub(super) copy_job: Cell<Option<Box<CopyUploadJob>>>,
    pub(super) staging: CloneCell<Option<Rc<VulkanStagingShell>>>,
    pub(super) callback: Cell<Option<Rc<dyn AsyncShmGfxTextureCallback>>>,
    pub(super) callback_id: Cell<u64>,
    pub(super) regions: RefCell<Vec<BufferImageCopy2<'static>>>,
    pub(super) cpu: Rc<CpuWorker>,
    pub(super) last_sample: Cell<Option<SyncFile>>,
    pub(super) data_copied: Cell<bool>,
}

impl VulkanShmImage {
    pub fn upload(
        &self,
        img: &Rc<VulkanImage>,
        buffer: &[Cell<u8>],
        damage: Option<&[Rect]>,
    ) -> Result<(), VulkanError> {
        img.renderer.check_defunct()?;
        if let Some(damage) = damage {
            if damage.is_empty() {
                return Ok(());
            }
        }
        let copy = |full: bool, off, x, y, width, height| {
            let mut builder = BufferImageCopy2::default()
                .buffer_offset(off)
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
                });
            if full {
                builder = builder
                    .buffer_image_height(img.height)
                    .buffer_row_length(img.stride / self.shm_info.bpp);
            }
            builder
        };
        let mut total_size;
        let cpy_one;
        let mut cpy_many;
        let cpy;
        if let Some(damage) = damage {
            total_size = 0;
            cpy_many = Vec::with_capacity(damage.len());
            for damage in damage {
                let Some(damage) = Rect::new(
                    damage.x1().max(0),
                    damage.y1().max(0),
                    damage.x2().min(img.width as i32),
                    damage.y2().min(img.height as i32),
                ) else {
                    continue;
                };
                if damage.is_empty() {
                    continue;
                }
                cpy_many.push(copy(
                    false,
                    total_size as DeviceSize,
                    damage.x1(),
                    damage.y1(),
                    damage.width() as u32,
                    damage.height() as u32,
                ));
                total_size += damage.width() as u32 * damage.height() as u32 * self.shm_info.bpp;
            }
            cpy = &cpy_many[..];
        } else {
            cpy_one = copy(true, 0, 0, 0, img.width, img.height);
            cpy = slice::from_ref(&cpy_one);
            total_size = img.height * img.stride;
        }
        let staging = img.renderer.device.create_staging_buffer(
            &img.renderer.allocator,
            total_size as u64,
            true,
            false,
            true,
        )?;
        staging.upload(|mem, _| unsafe {
            let buf = buffer.as_ptr() as *const u8;
            if damage.is_some() {
                let mut off = 0;
                for cpy in cpy {
                    let x = cpy.image_offset.x as usize;
                    let y = cpy.image_offset.y as usize;
                    let width = cpy.image_extent.width as usize;
                    let height = cpy.image_extent.height as usize;
                    let stride = self.stride as usize;
                    let bpp = self.shm_info.bpp as usize;
                    for dy in 0..height {
                        let lo = (y + dy) * stride + x * bpp;
                        let len = width * bpp;
                        ptr::copy_nonoverlapping(buf.add(lo), mem.add(off), len);
                        off += len;
                    }
                }
            } else {
                ptr::copy_nonoverlapping(buf, mem, total_size as usize);
            }
        })?;
        let Some((cmd, fence, sync_file, point)) =
            self.submit_buffer_to_image_copy(img, &staging, cpy, false)?
        else {
            return Ok(());
        };
        let future = img.renderer.eng.spawn(
            "await upload",
            await_upload(point, img.clone(), cmd, sync_file, fence, staging),
        );
        img.renderer.pending_submits.set(point, future);
        Ok(())
    }

    fn submit_buffer_to_image_copy(
        &self,
        img: &Rc<VulkanImage>,
        staging: &VulkanStagingBuffer,
        regions: &[BufferImageCopy2],
        use_transfer_queue: bool,
    ) -> Result<Option<(Rc<VulkanCommandBuffer>, Rc<VulkanFence>, SyncFile, u64)>, VulkanError>
    {
        let memory_barrier = |sam, ssm, dam, dsm| {
            BufferMemoryBarrier2::default()
                .buffer(staging.buffer)
                .offset(0)
                .size(staging.size)
                .src_access_mask(sam)
                .src_stage_mask(ssm)
                .dst_access_mask(dam)
                .dst_stage_mask(dsm)
        };
        let mut transfer_queue_family_idx = img.renderer.device.graphics_queue_idx;
        if use_transfer_queue {
            if let Some(idx) = img.renderer.device.distinct_transfer_queue_family_idx {
                transfer_queue_family_idx = idx;
            }
        }
        let mut initial_image_barrier = image_barrier()
            .image(img.image)
            .src_queue_family_index(img.renderer.device.graphics_queue_idx)
            .dst_queue_family_index(transfer_queue_family_idx)
            .dst_access_mask(AccessFlags2::TRANSFER_WRITE)
            .dst_stage_mask(PipelineStageFlags2::TRANSFER)
            .old_layout(if img.is_undefined.get() {
                ImageLayout::UNDEFINED
            } else {
                ImageLayout::SHADER_READ_ONLY_OPTIMAL
            })
            .new_layout(ImageLayout::TRANSFER_DST_OPTIMAL);
        if transfer_queue_family_idx == img.renderer.device.graphics_queue_idx {
            initial_image_barrier = initial_image_barrier
                .src_access_mask(AccessFlags2::SHADER_SAMPLED_READ)
                .src_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER)
        }
        let initial_buffer_barrier = memory_barrier(
            AccessFlags2::HOST_WRITE,
            PipelineStageFlags2::HOST,
            AccessFlags2::TRANSFER_READ,
            PipelineStageFlags2::TRANSFER,
        );
        let initial_dep_info = DependencyInfoKHR::default()
            .buffer_memory_barriers(slice::from_ref(&initial_buffer_barrier))
            .image_memory_barriers(slice::from_ref(&initial_image_barrier));
        let mut final_image_barrier = image_barrier()
            .image(img.image)
            .src_queue_family_index(transfer_queue_family_idx)
            .dst_queue_family_index(img.renderer.device.graphics_queue_idx)
            .src_access_mask(AccessFlags2::TRANSFER_WRITE)
            .src_stage_mask(PipelineStageFlags2::TRANSFER)
            .old_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        if transfer_queue_family_idx == img.renderer.device.graphics_queue_idx {
            final_image_barrier = final_image_barrier
                .dst_access_mask(AccessFlags2::SHADER_SAMPLED_READ)
                .dst_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER);
        }
        let final_buffer_barrier = memory_barrier(
            AccessFlags2::TRANSFER_READ,
            PipelineStageFlags2::TRANSFER,
            AccessFlags2::HOST_WRITE,
            PipelineStageFlags2::HOST,
        );
        let final_dep_info = DependencyInfoKHR::default()
            .buffer_memory_barriers(slice::from_ref(&final_buffer_barrier))
            .image_memory_barriers(slice::from_ref(&final_image_barrier));
        let cpy_info = CopyBufferToImageInfo2::default()
            .src_buffer(staging.buffer)
            .dst_image(img.image)
            .dst_image_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
            .regions(regions);
        let cmd = match &img.renderer.transfer_command_buffers {
            Some(b) if use_transfer_queue => b.allocate()?,
            _ => img.renderer.gfx_command_buffers.allocate()?,
        };
        let dev = &img.renderer.device.device;
        let command_buffer_info = CommandBufferSubmitInfo::default().command_buffer(cmd.buffer);
        let submit_info =
            SubmitInfo2::default().command_buffer_infos(slice::from_ref(&command_buffer_info));
        let begin_info =
            CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        let release_fence = img.renderer.device.create_fence()?;
        unsafe {
            dev.begin_command_buffer(cmd.buffer, &begin_info)
                .map_err(VulkanError::BeginCommandBuffer)?;
            dev.cmd_pipeline_barrier2(cmd.buffer, &initial_dep_info);
            dev.cmd_copy_buffer_to_image2(cmd.buffer, &cpy_info);
            dev.cmd_pipeline_barrier2(cmd.buffer, &final_dep_info);
            dev.end_command_buffer(cmd.buffer)
                .map_err(VulkanError::EndCommandBuffer)?;
            dev.queue_submit2(
                match img.renderer.device.transfer_queue {
                    Some(q) if use_transfer_queue => q,
                    _ => img.renderer.device.graphics_queue,
                },
                slice::from_ref(&submit_info),
                release_fence.fence,
            )
            .map_err(VulkanError::Submit)?;
        }
        img.is_undefined.set(false);
        img.contents_are_undefined.set(false);
        let release_sync_file = match release_fence.export_sync_file() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Could not export sync file from fence: {}", ErrorFmt(e));
                img.renderer.block();
                return Ok(None);
            }
        };
        let point = img.renderer.allocate_point();
        Ok(Some((cmd, release_fence, release_sync_file, point)))
    }
}

async fn await_upload(
    id: u64,
    img: Rc<VulkanImage>,
    buf: Rc<VulkanCommandBuffer>,
    sync_file: SyncFile,
    _fence: Rc<VulkanFence>,
    _staging: VulkanStagingBuffer,
) {
    let res = img.renderer.ring.readable(&sync_file.0).await;
    if let Err(e) = res {
        log::error!(
            "Could not wait for sync file to become readable: {}",
            ErrorFmt(e)
        );
        img.renderer.block();
    }
    img.renderer.gfx_command_buffers.buffers.push(buf);
    img.renderer.pending_submits.remove(&id);
}

impl VulkanShmImageAsyncData {
    fn complete(&self, result: Result<(), VulkanError>) {
        self.busy.set(false);
        self.staging.take().unwrap().busy.set(false);
        if let Some(cb) = self.callback.take() {
            cb.completed(result.map_err(|e| e.into()));
        }
    }
}

impl VulkanShmImage {
    pub fn async_upload(
        &self,
        img: &Rc<VulkanImage>,
        staging: Rc<VulkanStagingShell>,
        client_mem: &Rc<dyn ShmMemory>,
        damage: Region,
        callback: Rc<dyn AsyncShmGfxTextureCallback>,
    ) -> Result<Option<PendingShmUpload>, VulkanError> {
        let data = self.async_data.as_ref().unwrap();
        let res = self.try_async_upload(img, staging, data, client_mem, damage);
        match res {
            Ok(()) => {
                let id = img.renderer.allocate_point();
                data.callback_id.set(id);
                data.callback.set(Some(callback));
                Ok(Some(PendingShmUpload::new(img.clone(), id)))
            }
            Err(e) => Err(e),
        }
    }

    fn try_async_upload(
        &self,
        img: &Rc<VulkanImage>,
        staging: Rc<VulkanStagingShell>,
        data: &VulkanShmImageAsyncData,
        client_mem: &Rc<dyn ShmMemory>,
        mut damage: Region,
    ) -> Result<(), VulkanError> {
        if data.busy.get() {
            return Err(VulkanError::AsyncCopyBusy);
        }
        if staging.busy.get() {
            return Err(VulkanError::StagingBufferBusy);
        }
        if !staging.upload {
            return Err(VulkanError::StagingBufferNoUpload);
        }
        if self.size > client_mem.len() as u64 {
            return Err(VulkanError::InvalidBufferSize);
        }
        data.busy.set(true);
        data.data_copied.set(false);
        staging.busy.set(true);
        data.staging.set(Some(staging.clone()));
        if img.contents_are_undefined.get() {
            damage = Region::new2(Rect::new_sized(0, 0, img.width as _, img.height as _).unwrap());
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

        self.async_release_from_gfx_queue(img, data)?;

        if let Some(staging) = staging.staging.get() {
            return self.async_upload_initiate_copy(img, data, &staging, copies, client_mem);
        }

        let img2 = img.clone();
        let client_mem = client_mem.clone();
        img.renderer
            .device
            .fill_staging_shell(&img.renderer, &data.cpu, staging, move |res| {
                let VulkanImageMemory::Internal(shm) = &img2.ty else {
                    unreachable!();
                };
                if let Err(e) = shm.async_upload_after_allocation(&img2, &client_mem, res) {
                    shm.async_data.as_ref().unwrap().complete(Err(e));
                }
            })
    }

    fn async_release_from_gfx_queue(
        &self,
        img: &Rc<VulkanImage>,
        data: &VulkanShmImageAsyncData,
    ) -> Result<(), VulkanError> {
        img.renderer.check_defunct()?;
        let Some(transfer_queue_idx) = img.renderer.device.distinct_transfer_queue_family_idx
        else {
            let Some(sync_file) = data.last_sample.take() else {
                img.queue_state.set(QueueState::Released {
                    to: QueueFamily::Transfer,
                });
                return Ok(());
            };
            let id = img.renderer.allocate_point();
            let pending = img.renderer.eng.spawn(
                "await_transfer_to_transfer",
                await_gfx_queue_release(id, img.clone(), None, None, sync_file),
            );
            img.renderer.pending_submits.set(id, pending);
            img.queue_state.set(QueueState::Releasing);
            return Ok(());
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
                    .old_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                barriers.push(barrier);
            }
        }
        let barrier = image_barrier()
            .image(img.image)
            .src_queue_family_index(img.renderer.device.graphics_queue_idx)
            .dst_queue_family_index(transfer_queue_idx)
            .src_access_mask(AccessFlags2::SHADER_SAMPLED_READ)
            .src_stage_mask(PipelineStageFlags2::ALL_COMMANDS)
            .old_layout(if img.is_undefined.get() {
                ImageLayout::UNDEFINED
            } else {
                ImageLayout::SHADER_READ_ONLY_OPTIMAL
            })
            .new_layout(ImageLayout::TRANSFER_DST_OPTIMAL);
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
            .map_err(VulkanError::Submit)?;
        }
        let sync_file = release_fence.export_sync_file()?;
        let id = img.renderer.allocate_point();
        let pending = img.renderer.eng.spawn(
            "await_transfer_to_transfer",
            await_gfx_queue_release(id, img.clone(), Some(cmd), Some(release_fence), sync_file),
        );
        img.renderer.pending_submits.set(id, pending);
        img.queue_state.set(QueueState::Releasing);
        Ok(())
    }

    fn async_upload_after_allocation(
        &self,
        img: &Rc<VulkanImage>,
        client_mem: &Rc<dyn ShmMemory>,
        res: Result<Rc<VulkanStagingBuffer>, VulkanError>,
    ) -> Result<(), VulkanError> {
        let staging = res?;
        let data = self.async_data.as_ref().unwrap();
        let copies = &*data.regions.borrow();
        self.async_upload_initiate_copy(img, data, &staging, copies, client_mem)
    }

    fn async_upload_initiate_copy(
        &self,
        img: &Rc<VulkanImage>,
        data: &VulkanShmImageAsyncData,
        staging: &VulkanStagingBuffer,
        copies: &[BufferImageCopy2],
        client_mem: &Rc<dyn ShmMemory>,
    ) -> Result<(), VulkanError> {
        img.renderer.check_defunct()?;

        let id = img.renderer.allocate_point();
        let pending;
        match client_mem.safe_access() {
            ShmMemoryBacking::Ptr(ptr) => {
                let mut job = data.copy_job.take().unwrap_or_else(|| {
                    Box::new(CopyUploadJob {
                        img: None,
                        id,
                        _mem: None,
                        work: unsafe { ImgCopyWork::new() },
                    })
                });
                job.id = id;
                job.img = Some(img.clone());
                job._mem = Some(client_mem.clone());
                job.work.src = ptr as _;
                job.work.dst = staging.allocation.mem.unwrap();
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
                    Box::new(IoUploadJob {
                        img: None,
                        id,
                        _mem: None,
                        work: unsafe { ReadWriteWork::new() },
                        fd: None,
                    })
                });
                job.id = id;
                job.img = Some(img.clone());
                job._mem = Some(client_mem.clone());
                job.fd = Some(fd.clone());
                unsafe {
                    let config = job.work.config();
                    config.fd = fd.raw();
                    config.offset = offset + min_offset as usize;
                    config.ptr = staging.allocation.mem.unwrap().add(min_offset as _);
                    config.len = max_offset.saturating_sub(min_offset) as usize;
                    config.write = false;
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
        let Some((cmd, fence, sync_file, point)) =
            self.submit_buffer_to_image_copy(img, &staging, regions, true)?
        else {
            return Ok(());
        };
        img.queue_state.set(QueueState::Releasing);
        let future = img.renderer.eng.spawn(
            "await async upload",
            await_async_upload(point, img.clone(), cmd, fence, sync_file),
        );
        img.renderer.pending_submits.set(point, future);
        Ok(())
    }
}

pub(super) struct IoUploadJob {
    img: Option<Rc<VulkanImage>>,
    id: u64,
    _mem: Option<Rc<dyn ShmMemory>>,
    fd: Option<Rc<OwnedFd>>,
    work: ReadWriteWork,
}

pub(super) struct CopyUploadJob {
    img: Option<Rc<VulkanImage>>,
    id: u64,
    _mem: Option<Rc<dyn ShmMemory>>,
    work: ImgCopyWork,
}

impl CpuJob for IoUploadJob {
    fn work(&mut self) -> &mut dyn CpuWork {
        &mut self.work
    }

    fn completed(mut self: Box<Self>) {
        self._mem = None;
        self.fd = None;
        let img = self.img.take().unwrap();
        let res = self.work.config().result.take().unwrap();
        complete_async_upload(&img, self.id, res, |data| data.io_job.set(Some(self)));
    }
}

impl CpuJob for CopyUploadJob {
    fn work(&mut self) -> &mut dyn CpuWork {
        &mut self.work
    }

    fn completed(mut self: Box<Self>) {
        self._mem = None;
        let img = self.img.take().unwrap();
        complete_async_upload(&img, self.id, Ok(()), |data| data.copy_job.set(Some(self)));
    }
}

fn complete_async_upload(
    img: &Rc<VulkanImage>,
    id: u64,
    res: Result<(), ReadWriteJobError>,
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
    if let Err(e) = shm.async_upload_copy_buffer_to_image(img, data) {
        data.complete(Err(e));
    }
}

async fn await_gfx_queue_release(
    id: u64,
    img: Rc<VulkanImage>,
    buf: Option<Rc<VulkanCommandBuffer>>,
    _fence: Option<Rc<VulkanFence>>,
    sync_file: SyncFile,
) {
    let res = img.renderer.ring.readable(&sync_file.0).await;
    if let Err(e) = res {
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
    if let Err(e) = shm.async_upload_copy_buffer_to_image(&img, data) {
        data.complete(Err(e));
    }
}

async fn await_async_upload(
    id: u64,
    img: Rc<VulkanImage>,
    buf: Rc<VulkanCommandBuffer>,
    _fence: Rc<VulkanFence>,
    sync_file: SyncFile,
) {
    let res = img.renderer.ring.readable(&sync_file.0).await;
    if let Err(e) = res {
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
    data.complete(Ok(()));
}

impl VulkanRenderer {
    pub fn create_shm_texture(
        self: &Rc<Self>,
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
        data: &[Cell<u8>],
        for_download: bool,
        cpu_worker: Option<&Rc<CpuWorker>>,
    ) -> Result<Rc<VulkanImage>, VulkanError> {
        let Some(shm_info) = &format.shm_info else {
            return Err(VulkanError::UnsupportedShmFormat(format.name));
        };
        if width <= 0 || height <= 0 || stride <= 0 {
            return Err(VulkanError::NonPositiveImageSize);
        }
        let width = width as u32;
        let height = height as u32;
        let stride = stride as u32;
        if stride % shm_info.bpp != 0 || stride / shm_info.bpp < width {
            return Err(VulkanError::InvalidStride);
        }
        let vk_format = self
            .device
            .formats
            .get(&format.drm)
            .ok_or(VulkanError::FormatNotSupported)?;
        let shm = vk_format.shm.as_ref().ok_or(VulkanError::ShmNotSupported)?;
        if width > shm.limits.max_width || height > shm.limits.max_height {
            return Err(VulkanError::ImageTooLarge);
        }
        let size = stride.checked_mul(height).ok_or(VulkanError::ShmOverflow)? as u64;
        let usage = ImageUsageFlags::TRANSFER_SRC
            | match for_download {
                true => ImageUsageFlags::COLOR_ATTACHMENT,
                false => ImageUsageFlags::SAMPLED | ImageUsageFlags::TRANSFER_DST,
            };
        let create_info = ImageCreateInfo::default()
            .image_type(ImageType::TYPE_2D)
            .format(format.vk_format)
            .mip_levels(1)
            .array_layers(1)
            .tiling(ImageTiling::OPTIMAL)
            .samples(SampleCountFlags::TYPE_1)
            .sharing_mode(SharingMode::EXCLUSIVE)
            .initial_layout(ImageLayout::UNDEFINED)
            .extent(Extent3D {
                width,
                height,
                depth: 1,
            })
            .usage(usage);
        let image = unsafe { self.device.device.create_image(&create_info, None) };
        let image = image.map_err(VulkanError::CreateImage)?;
        let destroy_image = OnDrop(|| unsafe { self.device.device.destroy_image(image, None) });
        let memory_requirements =
            unsafe { self.device.device.get_image_memory_requirements(image) };
        let allocation =
            self.allocator
                .alloc(&memory_requirements, UsageFlags::FAST_DEVICE_ACCESS, false)?;
        let res = unsafe {
            self.device
                .device
                .bind_image_memory(image, allocation.memory, allocation.offset)
        };
        res.map_err(VulkanError::BindImageMemory)?;
        let image_view_create_info = ImageViewCreateInfo::default()
            .image(image)
            .format(format.vk_format)
            .view_type(ImageViewType::TYPE_2D)
            .subresource_range(ImageSubresourceRange {
                aspect_mask: ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let view = unsafe {
            self.device
                .device
                .create_image_view(&image_view_create_info, None)
        };
        let view = view.map_err(VulkanError::CreateImageView)?;
        let mut async_data = None;
        if let Some(cpu) = cpu_worker {
            async_data = Some(VulkanShmImageAsyncData {
                busy: Cell::new(false),
                io_job: Default::default(),
                copy_job: Default::default(),
                staging: Default::default(),
                callback: Default::default(),
                callback_id: Cell::new(0),
                regions: Default::default(),
                cpu: cpu.clone(),
                last_sample: Default::default(),
                data_copied: Default::default(),
            });
        }
        let shm = VulkanShmImage {
            size,
            stride,
            _allocation: allocation,
            shm_info,
            async_data,
        };
        destroy_image.forget();
        let img = Rc::new(VulkanImage {
            renderer: self.clone(),
            format,
            width,
            height,
            stride,
            texture_view: view,
            render_view: None,
            image,
            is_undefined: Cell::new(true),
            contents_are_undefined: Cell::new(true),
            queue_state: Cell::new(QueueState::Acquired {
                family: QueueFamily::Gfx,
            }),
            ty: VulkanImageMemory::Internal(shm),
            bridge: None,
        });
        let shm = match &img.ty {
            VulkanImageMemory::DmaBuf(_) => unreachable!(),
            VulkanImageMemory::Internal(s) => s,
        };
        if data.is_not_empty() {
            shm.upload(&img, data, None)?;
        }
        Ok(img)
    }
}
