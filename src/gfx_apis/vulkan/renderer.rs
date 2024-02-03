use {
    crate::{
        gfx_api::{AbsoluteRect, BufferPoint, BufferPoints, GfxApiOpt, GfxFormat, GfxTexture},
        gfx_apis::vulkan::{
            allocator::VulkanAllocator,
            command::{VulkanCommandBuffer, VulkanCommandPool},
            device::VulkanDevice,
            image::{VulkanImage, VulkanImageMemory},
            pipeline::{PipelineCreateInfo, VulkanPipeline},
            semaphore::VulkanTimelineSemaphore,
            shaders::{
                FillFragPushConstants, FillVertPushConstants, TexVertPushConstants, FILL_FRAG,
                FILL_VERT, TEX_FRAG, TEX_VERT,
            },
            staging::VulkanStagingBuffer,
            VulkanError,
        },
        theme::Color,
        utils::{copyhashmap::CopyHashMap, errorfmt::ErrorFmt, numcell::NumCell, stack::Stack},
        video::drm::{
            syncobj::{SyncObj, SyncObjPoint},
            wait_for_syncobj::{SyncObjWaiter, WaitForSyncObj, WaitForSyncObjHandle},
            DrmError,
        },
    },
    ahash::AHashMap,
    ash::{
        vk::{
            AccessFlags2, AttachmentLoadOp, AttachmentStoreOp, BufferImageCopy2,
            BufferMemoryBarrier2, ClearColorValue, ClearValue, CommandBuffer,
            CommandBufferBeginInfo, CommandBufferSubmitInfo, CommandBufferUsageFlags,
            CopyBufferToImageInfo2, DependencyInfoKHR, DescriptorImageInfo, DescriptorType,
            Extent2D, Extent3D, Fence, ImageAspectFlags, ImageLayout, ImageMemoryBarrier2,
            ImageMemoryBarrier2Builder, ImageSubresourceLayers, ImageSubresourceRange,
            PipelineBindPoint, PipelineStageFlags2, Rect2D, RenderingAttachmentInfo, RenderingInfo,
            SemaphoreSubmitInfo, SemaphoreWaitInfo, ShaderStageFlags, SubmitInfo2KHR, Viewport,
            WriteDescriptorSet, QUEUE_FAMILY_FOREIGN_EXT,
        },
        Device,
    },
    isnt::std_1::collections::IsntHashMapExt,
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        mem, ptr,
        rc::Rc,
        slice,
    },
};

pub struct VulkanRenderer {
    pub(super) formats: Rc<AHashMap<u32, GfxFormat>>,
    pub(super) device: Rc<VulkanDevice>,
    // pub(super) rgb_sampler: Rc<VulkanSampler>,
    pub(super) fill_pipeline: Rc<VulkanPipeline>,
    pub(super) tex_pipeline: Rc<VulkanPipeline>,
    pub(super) command_pool: Rc<VulkanCommandPool>,
    pub(super) command_buffers: Stack<Rc<VulkanCommandBuffer>>,
    pub(super) total_buffers: NumCell<usize>,
    // pub(super) wait_semaphores: RefCell<Vec<Semaphore>>,
    // pub(super) render_passes: RefCell<VecStorage<&'static [GfxCommand]>>,
    pub(super) memory: RefCell<Memory>,
    pub(super) pending_frames: CopyHashMap<u64, Rc<PendingFrame>>,

    pub(super) allocator: Rc<VulkanAllocator>,

    pub(super) wait_for_syncobj: Rc<WaitForSyncObj>,
    pub(super) semaphore: Rc<VulkanTimelineSemaphore>,
    pub(super) syncobj: Rc<SyncObj>,
    pub(super) last_point: NumCell<u64>,
}

#[derive(Default)]
pub(super) struct Memory {
    sample: Vec<Rc<VulkanImage>>,
    flush: Vec<Rc<VulkanImage>>,
    flush_staging: Vec<(Rc<VulkanImage>, VulkanStagingBuffer)>,
    textures: Vec<Rc<VulkanImage>>,
    image_barriers: Vec<ImageMemoryBarrier2>,
    shm_barriers: Vec<BufferMemoryBarrier2>,
}

pub(super) struct PendingFrame {
    point: u64,
    renderer: Rc<VulkanRenderer>,
    cmd: Cell<Option<Rc<VulkanCommandBuffer>>>,
    _textures: Vec<Rc<VulkanImage>>,
    _staging: Vec<(Rc<VulkanImage>, VulkanStagingBuffer)>,
    handle: Cell<Option<WaitForSyncObjHandle>>,
}

impl VulkanDevice {
    pub fn create_renderer(
        self: &Rc<Self>,
        wait_for_sync_obj: &Rc<WaitForSyncObj>,
    ) -> Result<Rc<VulkanRenderer>, VulkanError> {
        let fill_pipeline = self.create_pipeline::<FillVertPushConstants, FillFragPushConstants>(
            PipelineCreateInfo {
                vert: self.create_shader(FILL_VERT)?,
                frag: self.create_shader(FILL_FRAG)?,
                alpha: true,
                frag_descriptor_set_layout: None,
            },
        )?;
        let sampler = self.create_sampler()?;
        let tex_descriptor_set_layout = self.create_descriptor_set_layout(&sampler)?;
        let tex_pipeline =
            self.create_pipeline::<TexVertPushConstants, ()>(PipelineCreateInfo {
                vert: self.create_shader(TEX_VERT)?,
                frag: self.create_shader(TEX_FRAG)?,
                alpha: true,
                frag_descriptor_set_layout: Some(tex_descriptor_set_layout.clone()),
            })?;
        let command_pool = self.create_command_pool()?;
        let formats: AHashMap<u32, _> = self
            .formats
            .iter()
            .map(|(drm, vk)| {
                (
                    *drm,
                    GfxFormat {
                        format: vk.format,
                        read_modifiers: vk
                            .modifiers
                            .values()
                            .filter(|m| m.texture_max_extents.is_some())
                            .map(|m| m.modifier)
                            .collect(),
                        write_modifiers: vk
                            .modifiers
                            .values()
                            .filter(|m| m.render_max_extents.is_some())
                            .map(|m| m.modifier)
                            .collect(),
                    },
                )
            })
            .collect();
        let syncobj = self
            .gbm
            .drm
            .syncobj_ctx
            .create_sync_obj()
            .map_err(VulkanError::CreateSyncObj)?;
        let semaphore = self.create_timeline_semaphore(&syncobj)?;
        let allocator = self.create_allocator()?;
        Ok(Rc::new(VulkanRenderer {
            formats: Rc::new(formats),
            device: self.clone(),
            fill_pipeline,
            tex_pipeline,
            command_pool,
            command_buffers: Default::default(),
            total_buffers: Default::default(),
            memory: Default::default(),
            pending_frames: Default::default(),
            allocator,
            wait_for_syncobj: wait_for_sync_obj.clone(),
            semaphore,
            syncobj: Rc::new(syncobj),
            last_point: Default::default(),
        }))
    }
}

impl VulkanRenderer {
    fn collect_memory(&self, opts: &[GfxApiOpt]) {
        let mut memory = self.memory.borrow_mut();
        memory.sample.clear();
        memory.flush.clear();
        for cmd in opts {
            if let GfxApiOpt::CopyTexture(c) = cmd {
                let tex = c.tex.clone().into_vk(&self.device.device);
                match &tex.ty {
                    VulkanImageMemory::DmaBuf(_) => memory.sample.push(tex.clone()),
                    VulkanImageMemory::Internal(shm) => {
                        if shm.to_flush.borrow_mut().is_some() {
                            memory.flush.push(tex.clone());
                        }
                    }
                }
                memory.textures.push(tex);
            }
        }
    }

    fn begin_command_buffer(&self, buf: CommandBuffer) -> Result<(), VulkanError> {
        let begin_info =
            CommandBufferBeginInfo::builder().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .device
                .begin_command_buffer(buf, &begin_info)
                .map_err(VulkanError::BeginCommandBuffer)
        }
    }

    fn write_shm_staging_buffers(self: &Rc<Self>) -> Result<(), VulkanError> {
        let mut memory = self.memory.borrow_mut();
        let memory = &mut *memory;
        memory.flush_staging.clear();
        for img in &memory.flush {
            let shm = match &img.ty {
                VulkanImageMemory::DmaBuf(_) => unreachable!(),
                VulkanImageMemory::Internal(s) => s,
            };
            let staging = self.create_staging_buffer(shm.size, true, false, true)?;
            let to_flush = shm.to_flush.borrow_mut();
            let to_flush = to_flush.as_ref().unwrap();
            staging.upload(|mem, size| unsafe {
                let size = size.min(to_flush.len());
                ptr::copy_nonoverlapping(to_flush.as_ptr(), mem, size);
            })?;
            memory.flush_staging.push((img.clone(), staging));
        }
        Ok(())
    }

    fn initial_barriers(&self, buf: CommandBuffer, fb: &VulkanImage) {
        let mut memory = self.memory.borrow_mut();
        let memory = &mut *memory;
        memory.image_barriers.clear();
        memory.shm_barriers.clear();
        let fb_image_memory_barrier = image_barrier()
            .src_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
            .dst_queue_family_index(self.device.graphics_queue_idx)
            .image(fb.image)
            .old_layout(if fb.is_undefined.get() {
                ImageLayout::PREINITIALIZED
            } else {
                ImageLayout::GENERAL
            })
            .new_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .dst_access_mask(AccessFlags2::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
            .build();
        memory.image_barriers.push(fb_image_memory_barrier);
        for img in &memory.sample {
            let image_memory_barrier = image_barrier()
                .src_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                .dst_queue_family_index(self.device.graphics_queue_idx)
                .image(img.image)
                .old_layout(ImageLayout::PREINITIALIZED)
                .new_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .dst_access_mask(AccessFlags2::SHADER_SAMPLED_READ)
                .dst_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER)
                .build();
            memory.image_barriers.push(image_memory_barrier);
        }
        for (img, staging) in &memory.flush_staging {
            let image_memory_barrier = image_barrier()
                .image(img.image)
                .old_layout(if img.is_undefined.get() {
                    ImageLayout::UNDEFINED
                } else {
                    ImageLayout::SHADER_READ_ONLY_OPTIMAL
                })
                .new_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                .dst_access_mask(AccessFlags2::TRANSFER_WRITE)
                .dst_stage_mask(PipelineStageFlags2::TRANSFER)
                .build();
            memory.image_barriers.push(image_memory_barrier);
            let buffer_memory_barrier = BufferMemoryBarrier2::builder()
                .buffer(staging.buffer)
                .offset(0)
                .size(staging.size)
                .src_access_mask(AccessFlags2::HOST_WRITE)
                .src_stage_mask(PipelineStageFlags2::HOST)
                .dst_access_mask(AccessFlags2::TRANSFER_READ)
                .dst_stage_mask(PipelineStageFlags2::TRANSFER)
                .build();
            memory.shm_barriers.push(buffer_memory_barrier);
        }
        let dep_info = DependencyInfoKHR::builder()
            .buffer_memory_barriers(&memory.shm_barriers)
            .image_memory_barriers(&memory.image_barriers);
        unsafe {
            self.device.device.cmd_pipeline_barrier2(buf, &dep_info);
        }
    }

    fn copy_shm_to_image(&self, cmd: CommandBuffer) {
        let memory = self.memory.borrow_mut();
        for (img, staging) in &memory.flush_staging {
            let cpy = BufferImageCopy2::builder()
                .buffer_image_height(img.height)
                .buffer_row_length(img.width)
                .image_extent(Extent3D {
                    width: img.width,
                    height: img.height,
                    depth: 1,
                })
                .image_subresource(ImageSubresourceLayers {
                    aspect_mask: ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .build();
            let info = CopyBufferToImageInfo2::builder()
                .src_buffer(staging.buffer)
                .dst_image(img.image)
                .dst_image_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                .regions(slice::from_ref(&cpy));
            unsafe {
                self.device.device.cmd_copy_buffer_to_image2(cmd, &info);
            }
        }
    }

    fn secondary_barriers(&self, buf: CommandBuffer) {
        let mut memory = self.memory.borrow_mut();
        let memory = &mut *memory;
        if memory.flush.is_empty() {
            return;
        }
        memory.image_barriers.clear();
        for img in &memory.flush {
            let image_memory_barrier = image_barrier()
                .image(img.image)
                .old_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_access_mask(AccessFlags2::TRANSFER_WRITE)
                .src_stage_mask(PipelineStageFlags2::TRANSFER)
                .dst_access_mask(AccessFlags2::SHADER_SAMPLED_READ)
                .dst_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER)
                .build();
            memory.image_barriers.push(image_memory_barrier);
        }
        let dep_info = DependencyInfoKHR::builder().image_memory_barriers(&memory.image_barriers);
        unsafe {
            self.device.device.cmd_pipeline_barrier2(buf, &dep_info);
        }
    }

    fn begin_rendering(&self, buf: CommandBuffer, fb: &VulkanImage, clear: Option<&Color>) {
        let rendering_attachment_info = {
            let mut rai = RenderingAttachmentInfo::builder()
                .image_view(fb.view)
                .image_layout(ImageLayout::GENERAL)
                .load_op(AttachmentLoadOp::LOAD)
                .store_op(AttachmentStoreOp::STORE);
            if let Some(clear) = clear {
                rai = rai
                    .clear_value(ClearValue {
                        color: ClearColorValue {
                            float32: clear.to_array_linear(),
                        },
                    })
                    .load_op(AttachmentLoadOp::CLEAR);
            }
            rai
        };
        let rendering_info = RenderingInfo::builder()
            .render_area(Rect2D {
                offset: Default::default(),
                extent: Extent2D {
                    width: fb.width,
                    height: fb.height,
                },
            })
            .layer_count(1)
            .color_attachments(slice::from_ref(&rendering_attachment_info));
        unsafe {
            self.device.device.cmd_begin_rendering(buf, &rendering_info);
        }
    }

    fn set_viewport(&self, buf: CommandBuffer, fb: &VulkanImage) {
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            width: fb.width as _,
            height: fb.height as _,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let scissor = Rect2D {
            offset: Default::default(),
            extent: Extent2D {
                width: fb.width,
                height: fb.height,
            },
        };
        unsafe {
            self.device
                .device
                .cmd_set_viewport(buf, 0, slice::from_ref(&viewport));
            self.device
                .device
                .cmd_set_scissor(buf, 0, slice::from_ref(&scissor));
        }
    }

    fn record_draws(
        &self,
        buf: CommandBuffer,
        fb: &VulkanImage,
        opts: &[GfxApiOpt],
    ) -> Result<(), VulkanError> {
        let dev = &self.device.device;
        let mut current_pipeline = None;
        let mut bind = |pipeline: &VulkanPipeline| {
            if current_pipeline != Some(pipeline.pipeline) {
                current_pipeline = Some(pipeline.pipeline);
                unsafe {
                    dev.cmd_bind_pipeline(buf, PipelineBindPoint::GRAPHICS, pipeline.pipeline);
                }
            }
        };
        let width = fb.width as f32;
        let height = fb.height as f32;
        for opt in opts {
            match opt {
                GfxApiOpt::Sync => {}
                GfxApiOpt::FillRect(r) => {
                    bind(&self.fill_pipeline);
                    let vert = FillVertPushConstants {
                        pos: r.rect.to_vk(width, height),
                    };
                    let frag = FillFragPushConstants {
                        color: r.color.to_array_linear(),
                    };
                    unsafe {
                        dev.cmd_push_constants(
                            buf,
                            self.fill_pipeline.pipeline_layout,
                            ShaderStageFlags::VERTEX,
                            0,
                            uapi::as_bytes(&vert),
                        );
                        dev.cmd_push_constants(
                            buf,
                            self.fill_pipeline.pipeline_layout,
                            ShaderStageFlags::FRAGMENT,
                            self.fill_pipeline.frag_push_offset,
                            uapi::as_bytes(&frag),
                        );
                        dev.cmd_draw(buf, 4, 1, 0, 0);
                    }
                }
                GfxApiOpt::CopyTexture(c) => {
                    let tex = c.tex.as_vk(&self.device.device);
                    bind(&self.tex_pipeline);
                    let vert = TexVertPushConstants {
                        pos: c.target.to_vk(width, height),
                        tex_pos: c.source.to_vk(),
                    };
                    let image_info = DescriptorImageInfo::builder()
                        .image_view(tex.view)
                        .image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                    let write_descriptor_set = WriteDescriptorSet::builder()
                        .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(slice::from_ref(&image_info))
                        .build();
                    unsafe {
                        self.device.push_descriptor.cmd_push_descriptor_set(
                            buf,
                            PipelineBindPoint::GRAPHICS,
                            self.tex_pipeline.pipeline_layout,
                            0,
                            slice::from_ref(&write_descriptor_set),
                        );
                        dev.cmd_push_constants(
                            buf,
                            self.tex_pipeline.pipeline_layout,
                            ShaderStageFlags::VERTEX,
                            0,
                            uapi::as_bytes(&vert),
                        );
                        dev.cmd_draw(buf, 4, 1, 0, 0);
                    }
                }
            }
        }
        Ok(())
    }

    fn end_rendering(&self, buf: CommandBuffer) {
        unsafe {
            self.device.device.cmd_end_rendering(buf);
        }
    }

    fn final_barriers(&self, buf: CommandBuffer, fb: &VulkanImage) {
        let mut memory = self.memory.borrow_mut();
        let memory = &mut *memory;
        memory.image_barriers.clear();
        memory.shm_barriers.clear();
        let fb_image_memory_barrier = image_barrier()
            .src_queue_family_index(self.device.graphics_queue_idx)
            .dst_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
            .image(fb.image)
            .old_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .new_layout(ImageLayout::GENERAL)
            .src_access_mask(
                AccessFlags2::COLOR_ATTACHMENT_WRITE | AccessFlags2::COLOR_ATTACHMENT_READ,
            )
            .src_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
            .build();
        memory.image_barriers.push(fb_image_memory_barrier);
        // for img in &memory.sample {
        //     let image_memory_barrier = Self::image_barrier()
        //         .src_queue_family_index(self.device.graphics_queue_idx)
        //         .dst_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
        //         .image(img.image)
        //         .src_access_mask(AccessFlags2::SHADER_SAMPLED_READ)
        //         .src_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER)
        //         .build();
        //     memory.image_barriers.push(image_memory_barrier);
        // }
        let dep_info = DependencyInfoKHR::builder()
            .image_memory_barriers(&memory.image_barriers)
            .buffer_memory_barriers(&memory.shm_barriers);
        unsafe {
            self.device.device.cmd_pipeline_barrier2(buf, &dep_info);
        }
    }

    fn end_command_buffer(&self, buf: CommandBuffer) -> Result<(), VulkanError> {
        unsafe {
            self.device
                .device
                .end_command_buffer(buf)
                .map_err(VulkanError::EndCommandBuffer)
        }
    }

    fn submit(&self, buf: CommandBuffer) -> Result<(), VulkanError> {
        let point = self.last_point.get() + 1;
        let command_buffer_info = CommandBufferSubmitInfo::builder()
            .command_buffer(buf)
            .build();
        let release_semaphore = SemaphoreSubmitInfo::builder()
            .semaphore(self.semaphore.semaphore)
            .value(point)
            .stage_mask(PipelineStageFlags2::BOTTOM_OF_PIPE)
            .build();
        let submit_info = SubmitInfo2KHR::builder()
            .signal_semaphore_infos(slice::from_ref(&release_semaphore))
            .command_buffer_infos(slice::from_ref(&command_buffer_info))
            .build();
        unsafe {
            self.device
                .device
                .queue_submit2(
                    self.device.graphics_queue,
                    slice::from_ref(&submit_info),
                    Fence::null(),
                )
                .map_err(VulkanError::Submit)?;
        }
        self.last_point.set(point);
        Ok(())
    }

    fn store_layouts(&self, fb: &VulkanImage) {
        fb.is_undefined.set(false);
        let memory = self.memory.borrow_mut();
        for img in &memory.flush {
            img.is_undefined.set(false);
            let shm = match &img.ty {
                VulkanImageMemory::DmaBuf(_) => unreachable!(),
                VulkanImageMemory::Internal(s) => s,
            };
            shm.to_flush.take();
        }
    }

    fn create_pending_frame(self: &Rc<Self>, buf: Rc<VulkanCommandBuffer>) {
        let mut memory = self.memory.borrow_mut();
        let frame = Rc::new(PendingFrame {
            point: self.last_point.get(),
            renderer: self.clone(),
            cmd: Cell::new(Some(buf)),
            _textures: mem::take(&mut memory.textures),
            _staging: mem::take(&mut memory.flush_staging),
            handle: Cell::new(None),
        });
        let handle = self.wait_for_syncobj.wait(
            &self.syncobj,
            SyncObjPoint(frame.point),
            true,
            frame.clone(),
        );
        let handle = match handle {
            Ok(h) => h,
            Err(e) => {
                log::error!("Could not initiate syncobj wait: {}", ErrorFmt(e));
                self.block(None);
                return;
            }
        };
        frame.handle.set(Some(handle));
        self.pending_frames.set(frame.point, frame);
    }

    pub fn execute(
        self: &Rc<Self>,
        fb: &VulkanImage,
        opts: &[GfxApiOpt],
        clear: Option<&Color>,
    ) -> Result<(), VulkanError> {
        let res = self.try_execute(fb, opts, clear);
        {
            let mut memory = self.memory.borrow_mut();
            memory.flush.clear();
            memory.textures.clear();
            memory.flush_staging.clear();
            memory.sample.clear();
        }
        res
    }

    fn try_execute(
        self: &Rc<Self>,
        fb: &VulkanImage,
        opts: &[GfxApiOpt],
        clear: Option<&Color>,
    ) -> Result<(), VulkanError> {
        let buf = match self.command_buffers.pop() {
            Some(b) => b,
            _ => {
                self.total_buffers.fetch_add(1);
                self.command_pool.allocate_buffer()?
            }
        };
        self.collect_memory(opts);
        self.begin_command_buffer(buf.buffer)?;
        self.write_shm_staging_buffers()?;
        self.initial_barriers(buf.buffer, fb);
        self.copy_shm_to_image(buf.buffer);
        self.secondary_barriers(buf.buffer);
        self.begin_rendering(buf.buffer, fb, clear);
        self.set_viewport(buf.buffer, fb);
        self.record_draws(buf.buffer, fb, opts)?;
        self.end_rendering(buf.buffer);
        self.final_barriers(buf.buffer, fb);
        self.end_command_buffer(buf.buffer)?;
        self.submit(buf.buffer)?;
        self.store_layouts(fb);
        self.create_pending_frame(buf);
        Ok(())
    }

    fn block(&self, point: Option<u64>) {
        log::warn!("Blocking.");
        let point = point.unwrap_or(self.last_point.get());
        let wait_info = SemaphoreWaitInfo::builder()
            .semaphores(slice::from_ref(&self.semaphore.semaphore))
            .values(slice::from_ref(&point));
        let res = unsafe { self.device.device.wait_semaphores(&wait_info, u64::MAX) };
        if let Err(e) = res {
            log::error!("Could not wait for semaphore: {}", ErrorFmt(e));
        }
    }

    pub fn on_drop(&self) {
        let mut pending_frames = self.pending_frames.lock();
        if pending_frames.is_not_empty() {
            log::warn!("Context dropped with pending frames.");
            self.block(None);
        }
        pending_frames.values().for_each(|f| {
            f.handle.take();
        });
        pending_frames.clear();
    }
}

impl Debug for VulkanRenderer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanRenderer").finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct TmpShmTexture(pub i32, pub i32);

impl VulkanImage {
    fn assert_device(&self, device: &Device) {
        assert_eq!(
            self.renderer.device.device.handle(),
            device.handle(),
            "Mixed vulkan device use"
        );
    }
}

impl dyn GfxTexture {
    fn as_vk(&self, device: &Device) -> &VulkanImage {
        let img: &VulkanImage = self
            .as_any()
            .downcast_ref()
            .expect("Non-vulkan texture passed into vulkan");
        img.assert_device(device);
        img
    }

    pub(super) fn into_vk(self: Rc<Self>, device: &Device) -> Rc<VulkanImage> {
        let img: Rc<VulkanImage> = self
            .into_any()
            .downcast()
            .expect("Non-vulkan texture passed into vulkan");
        img.assert_device(device);
        img
    }
}

impl AbsoluteRect {
    fn to_vk(&self, width: f32, height: f32) -> [[f32; 2]; 4] {
        let x1 = 2.0 * self.x1 / width - 1.0;
        let x2 = 2.0 * self.x2 / width - 1.0;
        let y1 = 2.0 * self.y1 / height - 1.0;
        let y2 = 2.0 * self.y2 / height - 1.0;
        [[x2, y1], [x1, y1], [x2, y2], [x1, y2]]
    }
}

impl BufferPoint {
    fn to_vk(&self) -> [f32; 2] {
        [self.x, self.y]
    }
}

impl BufferPoints {
    fn to_vk(&self) -> [[f32; 2]; 4] {
        [
            self.top_right.to_vk(),
            self.top_left.to_vk(),
            self.bottom_right.to_vk(),
            self.bottom_left.to_vk(),
        ]
    }
}

impl SyncObjWaiter for PendingFrame {
    fn done(self: Rc<Self>, result: Result<(), DrmError>) {
        if self.renderer.device.instance.validation_enabled {
            let wait_info = SemaphoreWaitInfo::builder()
                .semaphores(slice::from_ref(&self.renderer.semaphore.semaphore))
                .values(slice::from_ref(&self.point));
            let res = unsafe { self.renderer.device.device.wait_semaphores(&wait_info, 0) };
            if let Err(e) = res {
                log::error!(
                    "Timeline point is not ready even after eventfd signal: {}",
                    ErrorFmt(e)
                );
            }
        }
        if let Err(e) = result {
            log::error!("Pending frame wait failed: {}", ErrorFmt(e));
            self.renderer.block(Some(self.point));
        }
        self.renderer.pending_frames.remove(&self.point);
        if let Some(buf) = self.cmd.take() {
            self.renderer.command_buffers.push(buf);
        }
    }
}

fn image_barrier() -> ImageMemoryBarrier2Builder<'static> {
    ImageMemoryBarrier2::builder().subresource_range(
        ImageSubresourceRange::builder()
            .aspect_mask(ImageAspectFlags::COLOR)
            .layer_count(1)
            .level_count(1)
            .build(),
    )
}
