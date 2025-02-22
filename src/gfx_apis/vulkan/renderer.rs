use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        cpu_worker::PendingJob,
        format::XRGB8888,
        gfx_api::{
            AcquireSync, BufferResv, BufferResvUser, GfxApiOpt, GfxBlendBuffer, GfxFormat,
            GfxFramebuffer, GfxTexture, GfxWriteModifier, ReleaseSync, SyncFile,
        },
        gfx_apis::vulkan::{
            VulkanError,
            allocator::{VulkanAllocator, VulkanThreadedAllocator},
            command::{VulkanCommandBuffer, VulkanCommandPool},
            descriptor::VulkanDescriptorSetLayout,
            descriptor_buffer::{
                VulkanDescriptorBuffer, VulkanDescriptorBufferCache, VulkanDescriptorBufferWriter,
            },
            device::VulkanDevice,
            fence::VulkanFence,
            format::BLEND_FORMAT,
            image::{QueueFamily, QueueState, QueueTransfer, VulkanImage, VulkanImageMemory},
            pipeline::{PipelineCreateInfo, VulkanPipeline},
            sampler::VulkanSampler,
            semaphore::VulkanSemaphore,
            shaders::{
                FILL_FRAG, FILL_VERT, FillPushConstants, OUT_FRAG, OUT_VERT, OutPushConstants,
                TEX_FRAG, TEX_VERT, TexPushConstants, VulkanShader,
            },
        },
        io_uring::IoUring,
        rect::Region,
        theme::Color,
        utils::{
            copyhashmap::CopyHashMap, errorfmt::ErrorFmt, numcell::NumCell, once::Once,
            stack::Stack,
        },
        video::dmabuf::{DMA_BUF_SYNC_READ, DMA_BUF_SYNC_WRITE, dma_buf_export_sync_file},
    },
    ahash::AHashMap,
    arrayvec::ArrayVec,
    ash::{
        Device,
        vk::{
            self, AccessFlags2, AttachmentLoadOp, AttachmentStoreOp, ClearAttachment,
            ClearColorValue, ClearRect, ClearValue, CommandBuffer, CommandBufferBeginInfo,
            CommandBufferSubmitInfo, CommandBufferUsageFlags, CopyImageInfo2, DependencyInfoKHR,
            DescriptorBufferBindingInfoEXT, DescriptorImageInfo, DescriptorType, DeviceSize,
            Extent2D, Extent3D, ImageAspectFlags, ImageCopy2, ImageLayout, ImageMemoryBarrier2,
            ImageSubresourceLayers, ImageSubresourceRange, Offset2D, Offset3D, PipelineBindPoint,
            PipelineStageFlags2, QUEUE_FAMILY_FOREIGN_EXT, Rect2D, RenderingAttachmentInfo,
            RenderingInfo, SemaphoreSubmitInfo, SemaphoreSubmitInfoKHR, ShaderStageFlags,
            SubmitInfo2, Viewport, WriteDescriptorSet,
        },
    },
    isnt::std_1::{collections::IsntHashMapExt, primitive::IsntSliceExt},
    linearize::{Linearize, StaticMap, static_map},
    std::{
        cell::{Cell, RefCell},
        collections::hash_map::Entry,
        fmt::{Debug, Formatter},
        mem, ptr,
        rc::{Rc, Weak},
        slice,
    },
    uapi::OwnedFd,
};

pub struct VulkanRenderer {
    pub(super) formats: Rc<AHashMap<u32, GfxFormat>>,
    pub(super) device: Rc<VulkanDevice>,
    pub(super) pipelines: CopyHashMap<vk::Format, Rc<VulkanFormatPipelines>>,
    pub(super) pipelines_with_blend_buffer: CopyHashMap<vk::Format, Rc<VulkanFormatPipelines>>,
    pub(super) out_pipelines: RefCell<AHashMap<vk::Format, Rc<VulkanPipeline>>>,
    pub(super) gfx_command_buffers: CachedCommandBuffers,
    pub(super) transfer_command_buffers: Option<CachedCommandBuffers>,
    pub(super) wait_semaphores: Stack<Rc<VulkanSemaphore>>,
    pub(super) memory: RefCell<Memory>,
    pub(super) pending_frames: CopyHashMap<u64, Rc<PendingFrame>>,
    pub(super) pending_submits: CopyHashMap<u64, SpawnedFuture<()>>,
    pub(super) allocator: Rc<VulkanAllocator>,
    pub(super) last_point: NumCell<u64>,
    pub(super) buffer_resv_user: BufferResvUser,
    pub(super) eng: Rc<AsyncEngine>,
    pub(super) ring: Rc<IoUring>,
    pub(super) fill_vert_shader: Rc<VulkanShader>,
    pub(super) fill_frag_shader: Rc<VulkanShader>,
    pub(super) tex_vert_shader: Rc<VulkanShader>,
    pub(super) tex_frag_shader: Rc<VulkanShader>,
    pub(super) out_vert_shader: Rc<VulkanShader>,
    pub(super) out_frag_shader: Rc<VulkanShader>,
    pub(super) tex_descriptor_set_layout: Rc<VulkanDescriptorSetLayout>,
    pub(super) out_descriptor_set_layout: Option<Rc<VulkanDescriptorSetLayout>>,
    pub(super) defunct: Cell<bool>,
    pub(super) pending_cpu_jobs: CopyHashMap<u64, PendingJob>,
    pub(super) shm_allocator: Rc<VulkanThreadedAllocator>,
    pub(super) sampler: Rc<VulkanSampler>,
    pub(super) sampler_descriptor_buffer_cache: Rc<VulkanDescriptorBufferCache>,
    pub(super) resource_descriptor_buffer_cache: Rc<VulkanDescriptorBufferCache>,
    pub(super) blend_buffers: RefCell<AHashMap<(u32, u32), Weak<VulkanImage>>>,
}

pub(super) struct CachedCommandBuffers {
    pub(super) pool: Rc<VulkanCommandPool>,
    pub(super) buffers: Stack<Rc<VulkanCommandBuffer>>,
    pub(super) total_buffers: NumCell<usize>,
}

impl CachedCommandBuffers {
    pub(super) fn allocate(&self) -> Result<Rc<VulkanCommandBuffer>, VulkanError> {
        zone!("allocate_command_buffer");
        let buf = match self.buffers.pop() {
            Some(b) => b,
            _ => {
                self.total_buffers.fetch_add(1);
                self.pool.allocate_buffer()?
            }
        };
        Ok(buf)
    }
}

pub(super) struct UsedTexture {
    tex: Rc<VulkanImage>,
    resv: Option<Rc<dyn BufferResv>>,
    acquire_sync: AcquireSync,
    release_sync: ReleaseSync,
}

#[derive(Linearize)]
pub(super) enum TexCopyType {
    Identity,
    Multiply,
}

#[derive(Linearize)]
pub(super) enum TexSourceType {
    Opaque,
    HasAlpha,
}

#[derive(Default)]
pub(super) struct Memory {
    dmabuf_sample: Vec<Rc<VulkanImage>>,
    queue_transfer: Vec<Rc<VulkanImage>>,
    textures: Vec<UsedTexture>,
    image_barriers: Vec<ImageMemoryBarrier2<'static>>,
    wait_semaphores: Vec<Rc<VulkanSemaphore>>,
    wait_semaphore_infos: Vec<SemaphoreSubmitInfo<'static>>,
    release_fence: Option<Rc<VulkanFence>>,
    release_sync_file: Option<SyncFile>,
    descriptor_buffers: ArrayVec<VulkanDescriptorBuffer, 2>,
    paint_regions: Vec<PaintRegion>,
    clear_rects: Vec<ClearRect>,
    image_copy_regions: Vec<ImageCopy2<'static>>,
    is_full_clear: bool,
    sampler_descriptor_buffer_writer: VulkanDescriptorBufferWriter,
    resource_descriptor_buffer_writer: VulkanDescriptorBufferWriter,
}

struct PaintRegion {
    rect: Rect2D,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

pub(super) struct PendingFrame {
    point: u64,
    renderer: Rc<VulkanRenderer>,
    cmd: Cell<Option<Rc<VulkanCommandBuffer>>>,
    _fb: Rc<VulkanImage>,
    _bb: Option<Rc<VulkanImage>>,
    _textures: Vec<UsedTexture>,
    wait_semaphores: Cell<Vec<Rc<VulkanSemaphore>>>,
    waiter: Cell<Option<SpawnedFuture<()>>>,
    _release_fence: Option<Rc<VulkanFence>>,
    _descriptor_buffers: ArrayVec<VulkanDescriptorBuffer, 2>,
}

pub(super) struct VulkanFormatPipelines {
    pub(super) fill: Rc<VulkanPipeline>,
    pub(super) tex: StaticMap<TexCopyType, StaticMap<TexSourceType, Rc<VulkanPipeline>>>,
}

impl VulkanDevice {
    pub fn create_renderer(
        self: &Rc<Self>,
        eng: &Rc<AsyncEngine>,
        ring: &Rc<IoUring>,
    ) -> Result<Rc<VulkanRenderer>, VulkanError> {
        let fill_vert_shader = self.create_shader(FILL_VERT)?;
        let fill_frag_shader = self.create_shader(FILL_FRAG)?;
        let sampler = self.create_sampler()?;
        let tex_descriptor_set_layout = self.create_tex_descriptor_set_layout(&sampler)?;
        let out_descriptor_set_layout = self
            .descriptor_buffer
            .as_ref()
            .map(|db| self.create_out_descriptor_set_layout(db))
            .transpose()?;
        let out_vert_shader = self.create_shader(OUT_VERT)?;
        let out_frag_shader = self.create_shader(OUT_FRAG)?;
        let tex_vert_shader = self.create_shader(TEX_VERT)?;
        let tex_frag_shader = self.create_shader(TEX_FRAG)?;
        let gfx_command_buffers = self.create_command_pool(self.graphics_queue_idx)?;
        let transfer_command_buffers = self
            .distinct_transfer_queue_family_idx
            .map(|idx| self.create_command_pool(idx))
            .transpose()?;
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
                            .filter(|m| m.texture_limits.is_some())
                            .map(|m| m.modifier)
                            .collect(),
                        write_modifiers: vk
                            .modifiers
                            .values()
                            .filter(|m| m.render_limits.is_some())
                            .map(|m| {
                                (
                                    m.modifier,
                                    GfxWriteModifier {
                                        needs_render_usage: !m.render_needs_bridge,
                                    },
                                )
                            })
                            .collect(),
                    },
                )
            })
            .collect();
        let allocator = self.create_allocator()?;
        let shm_allocator = self.create_threaded_allocator()?;
        let sampler_descriptor_buffer_cache =
            Rc::new(VulkanDescriptorBufferCache::new(self, &allocator, true));
        let resource_descriptor_buffer_cache =
            Rc::new(VulkanDescriptorBufferCache::new(self, &allocator, false));
        let render = Rc::new(VulkanRenderer {
            formats: Rc::new(formats),
            device: self.clone(),
            pipelines: Default::default(),
            pipelines_with_blend_buffer: Default::default(),
            out_pipelines: Default::default(),
            gfx_command_buffers,
            transfer_command_buffers,
            wait_semaphores: Default::default(),
            memory: Default::default(),
            pending_frames: Default::default(),
            pending_submits: Default::default(),
            allocator,
            last_point: Default::default(),
            buffer_resv_user: Default::default(),
            eng: eng.clone(),
            ring: ring.clone(),
            fill_vert_shader,
            fill_frag_shader,
            tex_vert_shader,
            tex_frag_shader,
            out_vert_shader,
            out_frag_shader,
            tex_descriptor_set_layout,
            out_descriptor_set_layout,
            defunct: Cell::new(false),
            pending_cpu_jobs: Default::default(),
            shm_allocator,
            sampler,
            sampler_descriptor_buffer_cache,
            resource_descriptor_buffer_cache,
            blend_buffers: Default::default(),
        });
        render.get_or_create_pipelines(XRGB8888.vk_format, None)?;
        Ok(render)
    }
}

impl VulkanRenderer {
    fn get_or_create_pipelines(
        &self,
        format: vk::Format,
        bb: Option<&VulkanImage>,
    ) -> Result<Rc<VulkanFormatPipelines>, VulkanError> {
        let with_linear_output = bb.is_some();
        let pipelines = match with_linear_output {
            false => &self.pipelines,
            true => &self.pipelines_with_blend_buffer,
        };
        if let Some(pl) = pipelines.get(&format) {
            return Ok(pl);
        }
        let fill = self
            .device
            .create_pipeline::<FillPushConstants>(PipelineCreateInfo {
                format,
                vert: self.fill_vert_shader.clone(),
                frag: self.fill_frag_shader.clone(),
                blend: true,
                src_has_alpha: true,
                has_alpha_mult: false,
                with_linear_output,
                frag_descriptor_set_layout: None,
            })?;
        let create_tex_pipeline = |src_has_alpha, has_alpha_mult| {
            self.device
                .create_pipeline::<TexPushConstants>(PipelineCreateInfo {
                    format,
                    vert: self.tex_vert_shader.clone(),
                    frag: self.tex_frag_shader.clone(),
                    blend: src_has_alpha || has_alpha_mult,
                    src_has_alpha,
                    has_alpha_mult,
                    with_linear_output,
                    frag_descriptor_set_layout: Some(self.tex_descriptor_set_layout.clone()),
                })
        };
        let tex_opaque = create_tex_pipeline(false, false)?;
        let tex_alpha = create_tex_pipeline(true, false)?;
        let tex_mult_opaque = create_tex_pipeline(false, true)?;
        let tex_mult_alpha = create_tex_pipeline(true, true)?;
        let format_pipelines = Rc::new(VulkanFormatPipelines {
            fill,
            tex: static_map! {
                TexCopyType::Identity => static_map! {
                    TexSourceType::HasAlpha => tex_alpha.clone(),
                    TexSourceType::Opaque => tex_opaque.clone(),
                },
                TexCopyType::Multiply => static_map! {
                    TexSourceType::HasAlpha => tex_mult_alpha.clone(),
                    TexSourceType::Opaque => tex_mult_opaque.clone(),
                },
            },
        });
        pipelines.set(format, format_pipelines.clone());
        Ok(format_pipelines)
    }

    pub(super) fn allocate_point(&self) -> u64 {
        self.last_point.fetch_add(1) + 1
    }

    fn create_descriptor_buffers(
        &self,
        buf: CommandBuffer,
        opts: &[GfxApiOpt],
        bb: Option<&VulkanImage>,
    ) -> Result<(), VulkanError> {
        let Some(db) = &self.device.descriptor_buffer else {
            return Ok(());
        };
        zone!("create_descriptor_buffers");
        let version = self.allocate_point();
        let memory = &mut *self.memory.borrow_mut();
        memory.descriptor_buffers.clear();
        let sampler_writer = &mut memory.sampler_descriptor_buffer_writer;
        sampler_writer.clear();
        let resource_writer = &mut memory.resource_descriptor_buffer_writer;
        resource_writer.clear();
        if let Some(bb) = bb {
            let layout = self.out_descriptor_set_layout.as_ref().unwrap();
            let offset = resource_writer.next_offset();
            bb.descriptor_buffer_offset.set(offset);
            let mut writer = resource_writer.add_set(layout);
            writer.write(layout.offsets[0], &bb.sampled_image_descriptor);
        }
        for cmd in opts {
            let GfxApiOpt::CopyTexture(c) = cmd else {
                continue;
            };
            let tex = c.tex.clone().into_vk(&self.device.device);
            if tex.descriptor_buffer_version.replace(version) == version {
                continue;
            }
            let offset = sampler_writer.next_offset();
            tex.descriptor_buffer_offset.set(offset);
            let mut writer = sampler_writer.add_set(&self.tex_descriptor_set_layout);
            writer.write(
                self.tex_descriptor_set_layout.offsets[0],
                &tex.shader_read_only_optimal_descriptor,
            );
        }
        let mut infos = ArrayVec::<_, 2>::new();
        for (writer, cache) in [
            (&sampler_writer, &self.sampler_descriptor_buffer_cache),
            (&resource_writer, &self.resource_descriptor_buffer_cache),
        ] {
            let buffer = cache.allocate(writer.len() as DeviceSize)?;
            buffer.buffer.allocation.upload(|ptr, _| unsafe {
                ptr::copy_nonoverlapping(writer.as_ptr(), ptr, writer.len())
            })?;
            let info = DescriptorBufferBindingInfoEXT::default()
                .usage(cache.usage())
                .address(buffer.buffer.address);
            infos.push(info);
            memory.descriptor_buffers.push(buffer);
        }
        unsafe {
            db.cmd_bind_descriptor_buffers(buf, &infos);
        }
        Ok(())
    }

    fn collect_memory(&self, opts: &[GfxApiOpt]) {
        zone!("collect_memory");
        let mut memory = self.memory.borrow_mut();
        memory.dmabuf_sample.clear();
        memory.queue_transfer.clear();
        let execution = self.allocate_point();
        for cmd in opts {
            if let GfxApiOpt::CopyTexture(c) = cmd {
                let tex = c.tex.clone().into_vk(&self.device.device);
                if tex.contents_are_undefined.get() {
                    continue;
                }
                if tex.execution_version.replace(execution) == execution {
                    continue;
                }
                match tex.queue_state.get().acquire(QueueFamily::Gfx) {
                    QueueTransfer::Unnecessary => {}
                    QueueTransfer::Possible => memory.queue_transfer.push(tex.clone()),
                    QueueTransfer::Impossible => continue,
                }
                if let VulkanImageMemory::DmaBuf(_) = &tex.ty {
                    memory.dmabuf_sample.push(tex.clone())
                }
                memory.textures.push(UsedTexture {
                    tex,
                    resv: c.buffer_resv.clone(),
                    acquire_sync: c.acquire_sync.clone(),
                    release_sync: c.release_sync,
                });
            }
        }
    }

    fn begin_command_buffer(&self, buf: CommandBuffer) -> Result<(), VulkanError> {
        zone!("begin_command_buffer");
        let begin_info =
            CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .device
                .begin_command_buffer(buf, &begin_info)
                .map_err(VulkanError::BeginCommandBuffer)
        }
    }

    fn initial_barriers(
        &self,
        buf: CommandBuffer,
        fb: &VulkanImage,
        bb: Option<&VulkanImage>,
    ) -> Result<(), VulkanError> {
        zone!("initial_barriers");
        let mut memory = self.memory.borrow_mut();
        let memory = &mut *memory;
        memory.image_barriers.clear();
        if let Some(bb) = bb {
            let barrier = image_barrier()
                .image(bb.image)
                .old_layout(if bb.is_undefined.get() {
                    ImageLayout::UNDEFINED
                } else {
                    ImageLayout::SHADER_READ_ONLY_OPTIMAL
                })
                .new_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .src_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER)
                .dst_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                .src_access_mask(AccessFlags2::SHADER_READ)
                .dst_access_mask(AccessFlags2::COLOR_ATTACHMENT_WRITE);
            memory.image_barriers.push(barrier);
        }
        let mut need_fb_barrier = true;
        if let VulkanImageMemory::Internal(..) = &fb.ty {
            need_fb_barrier = fb.is_undefined.get()
                || (self.device.distinct_transfer_queue_family_idx.is_some()
                    && fb.queue_state.get().acquire(QueueFamily::Gfx)
                        != QueueTransfer::Unnecessary);
        }
        if need_fb_barrier {
            let mut fb_image_memory_barrier = image_barrier()
                .image(fb.image)
                .new_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .dst_access_mask(
                    AccessFlags2::COLOR_ATTACHMENT_WRITE | AccessFlags2::COLOR_ATTACHMENT_READ,
                )
                .dst_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT);
            if fb.bridge.is_some() {
                fb_image_memory_barrier = fb_image_memory_barrier
                    .src_access_mask(AccessFlags2::TRANSFER_READ)
                    .src_stage_mask(PipelineStageFlags2::TRANSFER)
                    .old_layout(if fb.is_undefined.get() {
                        ImageLayout::UNDEFINED
                    } else {
                        ImageLayout::TRANSFER_SRC_OPTIMAL
                    });
            } else if let VulkanImageMemory::Internal(..) = &fb.ty {
                let mut queue_transfer = QueueTransfer::Unnecessary;
                if self.device.distinct_transfer_queue_family_idx.is_some() {
                    queue_transfer = fb.queue_state.get().acquire(QueueFamily::Gfx);
                }
                match queue_transfer {
                    QueueTransfer::Unnecessary => {
                        fb_image_memory_barrier =
                            fb_image_memory_barrier.old_layout(ImageLayout::UNDEFINED);
                    }
                    QueueTransfer::Possible => {
                        if let Some(transfer_queue_idx) =
                            self.device.distinct_transfer_queue_family_idx
                        {
                            fb_image_memory_barrier = fb_image_memory_barrier
                                .src_queue_family_index(transfer_queue_idx)
                                .dst_queue_family_index(self.device.graphics_queue_idx)
                                .old_layout(ImageLayout::TRANSFER_SRC_OPTIMAL);
                        }
                    }
                    QueueTransfer::Impossible => return Err(VulkanError::BusyInTransfer),
                }
            } else {
                fb_image_memory_barrier = fb_image_memory_barrier
                    .src_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                    .dst_queue_family_index(self.device.graphics_queue_idx)
                    .old_layout(if fb.is_undefined.get() {
                        ImageLayout::UNDEFINED
                    } else {
                        ImageLayout::GENERAL
                    });
            }
            memory.image_barriers.push(fb_image_memory_barrier);
        }
        for img in &memory.dmabuf_sample {
            let image_memory_barrier = image_barrier()
                .src_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                .dst_queue_family_index(self.device.graphics_queue_idx)
                .image(img.image)
                .old_layout(ImageLayout::GENERAL)
                .new_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .dst_access_mask(AccessFlags2::SHADER_SAMPLED_READ)
                .dst_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER);
            memory.image_barriers.push(image_memory_barrier);
        }
        if let Some(family_idx) = self.device.distinct_transfer_queue_family_idx {
            for img in &memory.queue_transfer {
                let image_memory_barrier = image_barrier()
                    .src_queue_family_index(family_idx)
                    .dst_queue_family_index(self.device.graphics_queue_idx)
                    .image(img.image)
                    .dst_access_mask(AccessFlags2::SHADER_SAMPLED_READ)
                    .dst_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER)
                    .old_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                memory.image_barriers.push(image_memory_barrier);
            }
        }
        let dep_info = DependencyInfoKHR::default().image_memory_barriers(&memory.image_barriers);
        unsafe {
            self.device.device.cmd_pipeline_barrier2(buf, &dep_info);
        }
        Ok(())
    }

    fn begin_rendering(
        &self,
        buf: CommandBuffer,
        fb: &VulkanImage,
        clear: Option<&Color>,
        bb: Option<&VulkanImage>,
    ) {
        zone!("begin_rendering");
        let memory = &mut *self.memory.borrow_mut();
        let mut load_clear = None;
        let mut manual_clear = None;
        memory.is_full_clear = false;
        if let Some(clear) = clear {
            let clear_value = ClearValue {
                color: ClearColorValue {
                    float32: if bb.is_some() {
                        clear.to_array_linear(None)
                    } else {
                        clear.to_array_srgb(None)
                    },
                },
            };
            let use_load_clear = memory.paint_regions.len() == 1 && {
                let rect = &memory.paint_regions[0].rect;
                rect.offset.x == 0
                    && rect.offset.y == 0
                    && rect.extent.width == fb.width
                    && rect.extent.height == fb.height
            };
            if use_load_clear {
                load_clear = Some(clear_value);
            } else {
                manual_clear = Some(clear_value);
            }
            memory.is_full_clear = use_load_clear;
        }
        let mut rendering_attachment_info = RenderingAttachmentInfo::default()
            .image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .store_op(AttachmentStoreOp::STORE);
        rendering_attachment_info = if let Some(bb) = bb {
            rendering_attachment_info
                .image_view(bb.render_view.unwrap_or(bb.texture_view))
                .load_op(AttachmentLoadOp::DONT_CARE)
        } else {
            rendering_attachment_info
                .image_view(fb.render_view.unwrap_or(fb.texture_view))
                .load_op(AttachmentLoadOp::LOAD)
        };
        if let Some(clear) = load_clear {
            rendering_attachment_info = rendering_attachment_info
                .clear_value(clear)
                .load_op(AttachmentLoadOp::CLEAR);
        }
        let rendering_info = RenderingInfo::default()
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
        if memory.paint_regions.is_not_empty() {
            if let Some(clear) = manual_clear {
                let clear_attachment = ClearAttachment::default()
                    .color_attachment(0)
                    .clear_value(clear)
                    .aspect_mask(ImageAspectFlags::COLOR);
                memory.clear_rects.clear();
                for region in &memory.paint_regions {
                    memory.clear_rects.push(ClearRect {
                        rect: region.rect,
                        base_array_layer: 0,
                        layer_count: 1,
                    });
                }
                unsafe {
                    self.device.device.cmd_clear_attachments(
                        buf,
                        &[clear_attachment],
                        &memory.clear_rects,
                    );
                }
            }
        }
    }

    fn set_viewport(&self, buf: CommandBuffer, fb: &VulkanImage) {
        zone!("set_viewport");
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
        bb: Option<&VulkanImage>,
    ) -> Result<(), VulkanError> {
        zone!("record_draws");
        let memory = &*self.memory.borrow();
        let format = match bb.is_some() {
            false => fb.format.vk_format,
            true => BLEND_FORMAT.vk_format,
        };
        let pipelines = self.get_or_create_pipelines(format, bb)?;
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
        for opt in opts {
            match opt {
                GfxApiOpt::Sync => {}
                GfxApiOpt::FillRect(r) => {
                    let push = FillPushConstants {
                        pos: r.rect.to_points(),
                        color: if bb.is_some() {
                            r.color.to_array_linear(r.alpha)
                        } else {
                            r.color.to_array_srgb(r.alpha)
                        },
                    };
                    for region in &memory.paint_regions {
                        let mut push = push;
                        let draw = region.constrain(&mut push.pos, None);
                        if !draw {
                            continue;
                        }
                        bind(&pipelines.fill);
                        unsafe {
                            dev.cmd_push_constants(
                                buf,
                                pipelines.fill.pipeline_layout,
                                ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
                                0,
                                uapi::as_bytes(&push),
                            );
                            dev.cmd_draw(buf, 4, 1, 0, 0);
                        }
                    }
                }
                GfxApiOpt::CopyTexture(c) => {
                    let tex = c.tex.as_vk(&self.device.device);
                    if tex.contents_are_undefined.get() {
                        log::warn!("Ignoring undefined texture");
                        continue;
                    }
                    if tex.queue_state.get().acquire(QueueFamily::Gfx) == QueueTransfer::Impossible
                    {
                        log::warn!("Ignoring texture owned by different queue");
                        continue;
                    }
                    let copy_type = match c.alpha.is_some() {
                        true => TexCopyType::Multiply,
                        false => TexCopyType::Identity,
                    };
                    let source_type = match tex.format.has_alpha {
                        true => TexSourceType::HasAlpha,
                        false => TexSourceType::Opaque,
                    };
                    let pipeline = &pipelines.tex[copy_type][source_type];
                    let push = TexPushConstants {
                        pos: c.target.to_points(),
                        tex_pos: c.source.to_points(),
                        alpha: c.alpha.unwrap_or_default(),
                    };
                    let image_info = DescriptorImageInfo::default()
                        .image_view(tex.texture_view)
                        .image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                    let init = Once::default();
                    for region in &memory.paint_regions {
                        let mut push = push;
                        let draw = region.constrain(&mut push.pos, Some(&mut push.tex_pos));
                        if !draw {
                            continue;
                        }
                        init.exec(|| unsafe {
                            bind(pipeline);
                            if let Some(db) = &self.device.descriptor_buffer {
                                db.cmd_set_descriptor_buffer_offsets(
                                    buf,
                                    PipelineBindPoint::GRAPHICS,
                                    pipeline.pipeline_layout,
                                    0,
                                    &[0],
                                    &[tex.descriptor_buffer_offset.get()],
                                );
                            } else {
                                let write_descriptor_set = WriteDescriptorSet::default()
                                    .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                                    .image_info(slice::from_ref(&image_info));
                                self.device.push_descriptor.cmd_push_descriptor_set(
                                    buf,
                                    PipelineBindPoint::GRAPHICS,
                                    pipeline.pipeline_layout,
                                    0,
                                    slice::from_ref(&write_descriptor_set),
                                );
                            }
                        });
                        unsafe {
                            dev.cmd_push_constants(
                                buf,
                                pipeline.pipeline_layout,
                                ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
                                0,
                                uapi::as_bytes(&push),
                            );
                            dev.cmd_draw(buf, 4, 1, 0, 0);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn copy_blend_buffer(
        &self,
        buf: CommandBuffer,
        fb: &VulkanImage,
        bb: Option<&VulkanImage>,
    ) -> Result<(), VulkanError> {
        let Some(bb) = bb else {
            return Ok(());
        };
        zone!("copy_blend_buffer");
        let memory = &*self.memory.borrow();
        let dev = &self.device.device;
        let db = self.device.descriptor_buffer.as_ref().unwrap();
        let pipeline = match self.out_pipelines.borrow_mut().entry(fb.format.vk_format) {
            Entry::Occupied(pipeline) => pipeline.get().clone(),
            Entry::Vacant(e) => {
                let layout = self.out_descriptor_set_layout.as_ref().unwrap();
                let out = self
                    .device
                    .create_pipeline::<OutPushConstants>(PipelineCreateInfo {
                        format: fb.format.vk_format,
                        vert: self.out_vert_shader.clone(),
                        frag: self.out_frag_shader.clone(),
                        blend: false,
                        src_has_alpha: true,
                        has_alpha_mult: false,
                        with_linear_output: true,
                        frag_descriptor_set_layout: Some(layout.clone()),
                    })?;
                e.insert(out.clone());
                out
            }
        };
        let image_barrier = image_barrier()
            .image(bb.image)
            .old_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .new_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_access_mask(AccessFlags2::COLOR_ATTACHMENT_WRITE)
            .dst_access_mask(AccessFlags2::SHADER_SAMPLED_READ)
            .src_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER);
        let dependency_info =
            DependencyInfoKHR::default().image_memory_barriers(slice::from_ref(&image_barrier));
        let rendering_attachment_info = RenderingAttachmentInfo::default()
            .image_view(fb.render_view.unwrap_or(fb.texture_view))
            .image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(if memory.is_full_clear {
                AttachmentLoadOp::DONT_CARE
            } else {
                AttachmentLoadOp::LOAD
            })
            .store_op(AttachmentStoreOp::STORE);
        let rendering_info = RenderingInfo::default()
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
            dev.cmd_pipeline_barrier2(buf, &dependency_info);
            dev.cmd_begin_rendering(buf, &rendering_info);
            dev.cmd_bind_pipeline(buf, PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            db.cmd_set_descriptor_buffer_offsets(
                buf,
                PipelineBindPoint::GRAPHICS,
                pipeline.pipeline_layout,
                0,
                &[1],
                &[bb.descriptor_buffer_offset.get()],
            );
            for region in &memory.paint_regions {
                let push = OutPushConstants {
                    pos: [
                        [region.x2, region.y1],
                        [region.x1, region.y1],
                        [region.x2, region.y2],
                        [region.x1, region.y2],
                    ],
                };
                dev.cmd_push_constants(
                    buf,
                    pipeline.pipeline_layout,
                    ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
                    0,
                    uapi::as_bytes(&push),
                );
                dev.cmd_draw(buf, 4, 1, 0, 0);
            }
            dev.cmd_end_rendering(buf);
        }
        Ok(())
    }

    fn end_rendering(&self, buf: CommandBuffer) {
        zone!("end_rendering");
        unsafe {
            self.device.device.cmd_end_rendering(buf);
        }
    }

    fn copy_bridge_to_dmabuf(&self, buf: CommandBuffer, fb: &VulkanImage) {
        zone!("copy_bridge_to_dmabuf");
        let Some(bridge) = &fb.bridge else {
            return;
        };
        let mut memory = self.memory.borrow_mut();
        let memory = &mut *memory;
        memory.image_barriers.clear();
        let bridge_image_memory_barrier = image_barrier()
            .image(fb.image)
            .old_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .new_layout(ImageLayout::TRANSFER_SRC_OPTIMAL)
            .src_access_mask(
                AccessFlags2::COLOR_ATTACHMENT_WRITE | AccessFlags2::COLOR_ATTACHMENT_READ,
            )
            .src_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(AccessFlags2::TRANSFER_READ)
            .dst_stage_mask(PipelineStageFlags2::TRANSFER);
        memory.image_barriers.push(bridge_image_memory_barrier);
        let dmabuf_image_memory_barrier = image_barrier()
            .src_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
            .dst_queue_family_index(self.device.graphics_queue_idx)
            .image(bridge.dmabuf_image)
            .old_layout(if fb.is_undefined.get() {
                ImageLayout::UNDEFINED
            } else {
                ImageLayout::GENERAL
            })
            .new_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
            .dst_access_mask(AccessFlags2::TRANSFER_WRITE)
            .dst_stage_mask(PipelineStageFlags2::TRANSFER);
        memory.image_barriers.push(dmabuf_image_memory_barrier);
        let dep_info = DependencyInfoKHR::default().image_memory_barriers(&memory.image_barriers);
        unsafe {
            self.device.device.cmd_pipeline_barrier2(buf, &dep_info);
        }
        let image_subresource_layers = ImageSubresourceLayers::default()
            .aspect_mask(ImageAspectFlags::COLOR)
            .layer_count(1)
            .base_array_layer(0)
            .mip_level(0);
        memory.image_copy_regions.clear();
        for region in &memory.paint_regions {
            let offset = Offset3D {
                x: region.rect.offset.x,
                y: region.rect.offset.y,
                z: 0,
            };
            let extent = Extent3D {
                width: region.rect.extent.width,
                height: region.rect.extent.height,
                depth: 1,
            };
            memory.image_copy_regions.push(
                ImageCopy2::default()
                    .src_subresource(image_subresource_layers)
                    .dst_subresource(image_subresource_layers)
                    .src_offset(offset)
                    .dst_offset(offset)
                    .extent(extent),
            );
        }
        let copy_image_info = CopyImageInfo2::default()
            .src_image(fb.image)
            .src_image_layout(ImageLayout::TRANSFER_SRC_OPTIMAL)
            .dst_image(bridge.dmabuf_image)
            .dst_image_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
            .regions(&memory.image_copy_regions);
        unsafe {
            self.device.device.cmd_copy_image2(buf, &copy_image_info);
        }
    }

    fn final_barriers(&self, buf: CommandBuffer, fb: &VulkanImage) {
        zone!("final_barriers");
        let mut memory = self.memory.borrow_mut();
        let memory = &mut *memory;
        memory.image_barriers.clear();
        if let VulkanImageMemory::DmaBuf(..) = fb.ty {
            let mut fb_image_memory_barrier = image_barrier()
                .src_queue_family_index(self.device.graphics_queue_idx)
                .dst_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                .new_layout(ImageLayout::GENERAL);
            if let Some(bridge) = &fb.bridge {
                fb_image_memory_barrier = fb_image_memory_barrier
                    .image(bridge.dmabuf_image)
                    .old_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                    .src_access_mask(AccessFlags2::TRANSFER_WRITE)
                    .src_stage_mask(PipelineStageFlags2::TRANSFER);
            } else {
                fb_image_memory_barrier = fb_image_memory_barrier
                    .image(fb.image)
                    .old_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .src_access_mask(
                        AccessFlags2::COLOR_ATTACHMENT_WRITE | AccessFlags2::COLOR_ATTACHMENT_READ,
                    )
                    .src_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT);
            }
            memory.image_barriers.push(fb_image_memory_barrier);
        }
        for img in &memory.dmabuf_sample {
            let image_memory_barrier = image_barrier()
                .src_queue_family_index(self.device.graphics_queue_idx)
                .dst_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                .old_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .new_layout(ImageLayout::GENERAL)
                .image(img.image)
                .src_access_mask(AccessFlags2::SHADER_SAMPLED_READ)
                .src_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER);
            memory.image_barriers.push(image_memory_barrier);
        }
        let dep_info = DependencyInfoKHR::default().image_memory_barriers(&memory.image_barriers);
        unsafe {
            self.device.device.cmd_pipeline_barrier2(buf, &dep_info);
        }
    }

    fn end_command_buffer(&self, buf: CommandBuffer) -> Result<(), VulkanError> {
        zone!("end_command_buffer");
        unsafe {
            self.device
                .device
                .end_command_buffer(buf)
                .map_err(VulkanError::EndCommandBuffer)
        }
    }

    fn create_wait_semaphores(
        &self,
        fb: &VulkanImage,
        fb_acquire_sync: &AcquireSync,
    ) -> Result<(), VulkanError> {
        zone!("create_wait_semaphores");
        let mut memory = self.memory.borrow_mut();
        let memory = &mut *memory;
        memory.wait_semaphore_infos.clear();
        let import = |infos: &mut Vec<SemaphoreSubmitInfoKHR>,
                      semaphores: &mut Vec<Rc<VulkanSemaphore>>,
                      img: &VulkanImage,
                      sync: &AcquireSync,
                      flag: u32|
         -> Result<(), VulkanError> {
            if let VulkanImageMemory::DmaBuf(buf) = &img.ty {
                let mut import_sync_file = |fd: OwnedFd| -> Result<(), VulkanError> {
                    let semaphore = self.allocate_semaphore()?;
                    semaphore.import_sync_file(fd)?;
                    infos.push(
                        SemaphoreSubmitInfo::default()
                            .semaphore(semaphore.semaphore)
                            .stage_mask(PipelineStageFlags2::TOP_OF_PIPE),
                    );
                    semaphores.push(semaphore);
                    Ok(())
                };
                match sync {
                    AcquireSync::None => {}
                    AcquireSync::Implicit { .. } => {
                        zone!("import implicit");
                        for plane in &buf.template.dmabuf.planes {
                            let fd = dma_buf_export_sync_file(&plane.fd, flag)
                                .map_err(VulkanError::IoctlExportSyncFile)?;
                            import_sync_file(fd)?;
                        }
                    }
                    AcquireSync::SyncFile { sync_file } => {
                        let fd = uapi::fcntl_dupfd_cloexec(sync_file.raw(), 0)
                            .map_err(|e| VulkanError::Dupfd(e.into()))?;
                        import_sync_file(fd)?;
                    }
                    AcquireSync::Unnecessary => {}
                }
            }
            Ok(())
        };
        for texture in &memory.textures {
            import(
                &mut memory.wait_semaphore_infos,
                &mut memory.wait_semaphores,
                &texture.tex,
                &texture.acquire_sync,
                DMA_BUF_SYNC_READ,
            )?;
        }
        import(
            &mut memory.wait_semaphore_infos,
            &mut memory.wait_semaphores,
            fb,
            fb_acquire_sync,
            DMA_BUF_SYNC_WRITE,
        )?;
        Ok(())
    }

    fn import_release_semaphore(&self, fb: &VulkanImage, fb_release_sync: ReleaseSync) {
        zone!("import_release_semaphore");
        let memory = &mut *self.memory.borrow_mut();
        let sync_file = match memory.release_sync_file.as_ref() {
            Some(sync_file) => sync_file,
            _ => return,
        };
        let import =
            |img: &VulkanImage, sync: ReleaseSync, resv: Option<Rc<dyn BufferResv>>, flag: u32| {
                if sync == ReleaseSync::None {
                    return;
                }
                if let Some(resv) = resv {
                    resv.set_sync_file(self.buffer_resv_user, sync_file);
                } else if sync == ReleaseSync::Implicit {
                    if let VulkanImageMemory::DmaBuf(buf) = &img.ty {
                        if let Err(e) = buf.template.dmabuf.import_sync_file(flag, sync_file) {
                            log::error!("Could not import sync file into dmabuf: {}", ErrorFmt(e));
                            log::warn!("Relying on implicit sync");
                        }
                    }
                }
            };
        let attach_async_shm_sync_file = self.device.transfer_queue.is_some()
            && self.device.distinct_transfer_queue_family_idx.is_none();
        for texture in &mut memory.textures {
            import(
                &texture.tex,
                texture.release_sync,
                texture.resv.take(),
                DMA_BUF_SYNC_READ,
            );
            if attach_async_shm_sync_file {
                if let VulkanImageMemory::Internal(shm) = &texture.tex.ty {
                    if let Some(data) = &shm.async_data {
                        data.last_gfx_use.set(Some(sync_file.clone()));
                    }
                }
            }
        }
        if attach_async_shm_sync_file {
            if let VulkanImageMemory::Internal(shm) = &fb.ty {
                if let Some(data) = &shm.async_data {
                    data.last_gfx_use.set(Some(sync_file.clone()));
                }
            }
        }
        import(fb, fb_release_sync, None, DMA_BUF_SYNC_WRITE);
    }

    fn submit(&self, buf: CommandBuffer) -> Result<(), VulkanError> {
        zone!("submit");
        let mut memory = self.memory.borrow_mut();
        let release_fence = self.device.create_fence()?;
        let command_buffer_info = CommandBufferSubmitInfo::default().command_buffer(buf);
        let submit_info = SubmitInfo2::default()
            .wait_semaphore_infos(&memory.wait_semaphore_infos)
            .command_buffer_infos(slice::from_ref(&command_buffer_info));
        unsafe {
            self.device
                .device
                .queue_submit2(
                    self.device.graphics_queue,
                    slice::from_ref(&submit_info),
                    release_fence.fence,
                )
                .map_err(VulkanError::Submit)?;
        }
        zone!("export_sync_file");
        let release_sync_file = match release_fence.export_sync_file() {
            Ok(s) => Some(s),
            Err(e) => {
                log::error!("Could not export sync file from fence: {}", ErrorFmt(e));
                self.block();
                None
            }
        };
        memory.release_fence = Some(release_fence);
        memory.release_sync_file = release_sync_file;
        Ok(())
    }

    fn store_layouts(&self, fb: &VulkanImage, bb: Option<&VulkanImage>) {
        if let Some(bb) = bb {
            bb.is_undefined.set(false);
        }
        fb.is_undefined.set(false);
        fb.contents_are_undefined.set(false);
        fb.queue_state.set(QueueState::Acquired {
            family: QueueFamily::Gfx,
        });
        let memory = self.memory.borrow();
        for img in &*memory.queue_transfer {
            img.queue_state.set(QueueState::Acquired {
                family: QueueFamily::Gfx,
            });
        }
    }

    fn create_pending_frame(
        self: &Rc<Self>,
        buf: Rc<VulkanCommandBuffer>,
        fb: &Rc<VulkanImage>,
        bb: Option<Rc<VulkanImage>>,
    ) {
        zone!("create_pending_frame");
        let point = self.allocate_point();
        let mut memory = self.memory.borrow_mut();
        let frame = Rc::new(PendingFrame {
            point,
            renderer: self.clone(),
            cmd: Cell::new(Some(buf)),
            _fb: fb.clone(),
            _bb: bb,
            _textures: mem::take(&mut memory.textures),
            wait_semaphores: Cell::new(mem::take(&mut memory.wait_semaphores)),
            waiter: Cell::new(None),
            _release_fence: memory.release_fence.take(),
            _descriptor_buffers: mem::take(&mut memory.descriptor_buffers),
        });
        self.pending_frames.set(frame.point, frame.clone());
        let future = self.eng.spawn(
            "await release",
            await_release(
                memory.release_sync_file.clone(),
                self.ring.clone(),
                frame.clone(),
                self.clone(),
            ),
        );
        frame.waiter.set(Some(future));
    }

    pub fn execute(
        self: &Rc<Self>,
        fb: &Rc<VulkanImage>,
        fb_acquire_sync: AcquireSync,
        fb_release_sync: ReleaseSync,
        opts: &[GfxApiOpt],
        clear: Option<&Color>,
        region: &Region,
        blend_buffer: Option<Rc<VulkanImage>>,
    ) -> Result<Option<SyncFile>, VulkanError> {
        zone!("execute");
        let res = self.try_execute(
            fb,
            fb_acquire_sync,
            fb_release_sync,
            opts,
            clear,
            region,
            blend_buffer,
        );
        let sync_file = {
            let mut memory = self.memory.borrow_mut();
            memory.textures.clear();
            memory.dmabuf_sample.clear();
            memory.queue_transfer.clear();
            memory.wait_semaphores.clear();
            memory.release_fence.take();
            memory.descriptor_buffers.clear();
            memory.release_sync_file.take()
        };
        res.map(|_| sync_file)
    }

    fn allocate_semaphore(&self) -> Result<Rc<VulkanSemaphore>, VulkanError> {
        zone!("allocate_semaphore");
        let semaphore = match self.wait_semaphores.pop() {
            Some(s) => s,
            _ => self.device.create_semaphore()?,
        };
        Ok(semaphore)
    }

    fn create_paint_regions(&self, fb: &VulkanImage, region: &Region) {
        let mut region = region;
        let region_owned;
        if fb.contents_are_undefined.get() {
            region_owned = fb.full_region();
            region = &region_owned;
        }
        let memory = &mut *self.memory.borrow_mut();
        memory.paint_regions.clear();
        for rect in region.rects() {
            let x1 = rect.x1().max(0);
            let y1 = rect.y1().max(0);
            let x2 = rect.x2();
            let y2 = rect.y2();
            if x1 as u32 > fb.width || y1 as u32 > fb.height || x2 <= 0 || y2 <= 0 {
                continue;
            }
            let x2 = x2.min(fb.width as i32);
            let y2 = y2.min(fb.height as i32);
            let to_fb = |c: i32, max: u32| 2.0 * (c as f32 / max as f32) - 1.0;
            memory.paint_regions.push(PaintRegion {
                rect: Rect2D {
                    offset: Offset2D {
                        x: x1 as _,
                        y: y1 as _,
                    },
                    extent: Extent2D {
                        width: (x2 - x1) as u32,
                        height: (y2 - y1) as u32,
                    },
                },
                x1: to_fb(x1, fb.width),
                x2: to_fb(x2, fb.width),
                y1: to_fb(y1, fb.height),
                y2: to_fb(y2, fb.height),
            });
        }
    }

    fn try_execute(
        self: &Rc<Self>,
        fb: &Rc<VulkanImage>,
        fb_acquire_sync: AcquireSync,
        fb_release_sync: ReleaseSync,
        opts: &[GfxApiOpt],
        clear: Option<&Color>,
        region: &Region,
        blend_buffer: Option<Rc<VulkanImage>>,
    ) -> Result<(), VulkanError> {
        let bb = blend_buffer.as_deref();
        self.check_defunct()?;
        self.create_paint_regions(fb, region);
        let buf = self.gfx_command_buffers.allocate()?;
        self.collect_memory(opts);
        self.begin_command_buffer(buf.buffer)?;
        self.create_descriptor_buffers(buf.buffer, opts, bb)?;
        self.initial_barriers(buf.buffer, fb, bb)?;
        self.begin_rendering(buf.buffer, fb, clear, bb);
        self.set_viewport(buf.buffer, fb);
        self.record_draws(buf.buffer, fb, opts, bb)?;
        self.end_rendering(buf.buffer);
        self.copy_blend_buffer(buf.buffer, fb, bb)?;
        self.copy_bridge_to_dmabuf(buf.buffer, fb);
        self.final_barriers(buf.buffer, fb);
        self.end_command_buffer(buf.buffer)?;
        self.create_wait_semaphores(fb, &fb_acquire_sync)?;
        self.submit(buf.buffer)?;
        self.import_release_semaphore(fb, fb_release_sync);
        self.store_layouts(fb, bb);
        self.create_pending_frame(buf, fb, blend_buffer);
        Ok(())
    }

    pub(super) fn block(&self) {
        log::warn!("Blocking.");
        unsafe {
            if let Err(e) = self.device.device.device_wait_idle() {
                log::error!("Could not wait for device idle: {}", ErrorFmt(e));
            }
        }
    }

    pub fn on_drop(&self) {
        self.defunct.set(true);
        let mut pending_frames = self.pending_frames.lock();
        let mut pending_uploads = self.pending_submits.lock();
        if pending_frames.is_not_empty() || pending_uploads.is_not_empty() {
            log::warn!("Context dropped with pending frames.");
            self.block();
        }
        pending_frames.values().for_each(|f| {
            f.waiter.take();
        });
        pending_frames.clear();
        pending_uploads.clear();
    }

    pub(super) fn check_defunct(&self) -> Result<(), VulkanError> {
        match self.defunct.get() {
            true => Err(VulkanError::Defunct),
            false => Ok(()),
        }
    }
}

impl Debug for VulkanRenderer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanRenderer").finish_non_exhaustive()
    }
}

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

impl dyn GfxBlendBuffer {
    pub(super) fn into_vk(self: Rc<Self>, device: &Device) -> Rc<VulkanImage> {
        let img: Rc<VulkanImage> = self
            .into_any()
            .downcast()
            .expect("Non-vulkan blend buffer passed into vulkan");
        img.assert_device(device);
        img
    }
}

pub(super) fn image_barrier() -> ImageMemoryBarrier2<'static> {
    ImageMemoryBarrier2::default().subresource_range(
        ImageSubresourceRange::default()
            .aspect_mask(ImageAspectFlags::COLOR)
            .layer_count(1)
            .level_count(1),
    )
}

async fn await_release(
    sync_file: Option<SyncFile>,
    ring: Rc<IoUring>,
    frame: Rc<PendingFrame>,
    renderer: Rc<VulkanRenderer>,
) {
    let mut is_released = false;
    if let Some(sync_file) = sync_file {
        if let Err(e) = ring.readable(&sync_file).await {
            log::error!(
                "Could not wait for release semaphore to be signaled: {}",
                ErrorFmt(e)
            );
        } else {
            is_released = true;
        }
    }
    if !is_released {
        frame.renderer.block();
    }
    if let Some(buf) = frame.cmd.take() {
        frame.renderer.gfx_command_buffers.buffers.push(buf);
    }
    for wait_semaphore in frame.wait_semaphores.take() {
        frame.renderer.wait_semaphores.push(wait_semaphore);
    }
    renderer.pending_frames.remove(&frame.point);
}

impl PaintRegion {
    fn constrain(&self, pos: &mut [[f32; 2]; 4], tex_pos: Option<&mut [[f32; 2]; 4]>) -> bool {
        zone!("constrain");
        let mut npos = *pos;
        for [x, y] in &mut npos {
            *x = x.clamp(self.x1, self.x2);
            *y = y.clamp(self.y1, self.y2);
        }
        if npos == *pos {
            return true;
        }
        if npos[0] == npos[1] && npos[2] == npos[3] {
            return false;
        }
        if npos[0] == npos[2] && npos[1] == npos[3] {
            return false;
        }
        if let Some(tp) = tex_pos {
            let mut ntp = *tp;
            for i in 0..4 {
                if npos[i] == pos[i] {
                    continue;
                }
                macro_rules! sub {
                    ($l:expr, $r:expr) => {
                        [$l[0] - $r[0], $l[1] - $r[1]]
                    };
                }
                let dx = sub!(npos[i], pos[i]);
                let dy = sub!(pos[(i + 1) & 3], pos[i]);
                let dz = sub!(pos[(i + 2) & 3], pos[i]);
                let det = 1.0 / (dy[0] * dz[1] - dy[1] * dz[0]);
                let alpha = [
                    (dx[0] * dz[1] - dx[1] * dz[0]) * det,
                    (dx[1] * dy[0] - dx[0] * dy[1]) * det,
                ];
                let dy = sub!(tp[(i + 1) & 3], tp[i]);
                let dz = sub!(tp[(i + 2) & 3], tp[i]);
                ntp[i][0] += alpha[0] * dy[0] + alpha[1] * dz[0];
                ntp[i][1] += alpha[0] * dy[1] + alpha[1] * dz[1];
            }
            *tp = ntp;
        }
        *pos = npos;
        true
    }
}
