use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        cpu_worker::PendingJob,
        format::{Format, XRGB8888},
        gfx_api::{
            AcquireSync, BufferResv, BufferResvUser, GfxApiOpt, GfxFormat, GfxFramebuffer,
            GfxTexture, GfxWriteModifier, ReleaseSync, SyncFile,
        },
        gfx_apis::vulkan::{
            allocator::{VulkanAllocator, VulkanThreadedAllocator},
            command::{VulkanCommandBuffer, VulkanCommandPool},
            descriptor::VulkanDescriptorSetLayout,
            device::VulkanDevice,
            fence::VulkanFence,
            image::{VulkanImage, VulkanImageMemory},
            pipeline::{PipelineCreateInfo, VulkanPipeline},
            semaphore::VulkanSemaphore,
            shaders::{
                FillFragPushConstants, FillVertPushConstants, TexFragPushConstants,
                TexVertPushConstants, VulkanShader, FILL_FRAG, FILL_VERT, TEX_FRAG,
                TEX_FRAG_MULT_ALPHA, TEX_FRAG_MULT_OPAQUE, TEX_VERT,
            },
            VulkanError,
        },
        io_uring::IoUring,
        theme::Color,
        utils::{copyhashmap::CopyHashMap, errorfmt::ErrorFmt, numcell::NumCell, stack::Stack},
        video::dmabuf::{dma_buf_export_sync_file, DMA_BUF_SYNC_READ, DMA_BUF_SYNC_WRITE},
    },
    ahash::AHashMap,
    ash::{
        vk,
        vk::{
            AccessFlags2, AttachmentLoadOp, AttachmentStoreOp, BufferImageCopy,
            BufferMemoryBarrier2, ClearColorValue, ClearValue, CommandBuffer,
            CommandBufferBeginInfo, CommandBufferSubmitInfo, CommandBufferUsageFlags,
            CopyImageInfo2, DependencyInfo, DependencyInfoKHR, DescriptorImageInfo, DescriptorType,
            Extent2D, Extent3D, Fence, ImageAspectFlags, ImageCopy2, ImageLayout,
            ImageMemoryBarrier2, ImageSubresourceLayers, ImageSubresourceRange, PipelineBindPoint,
            PipelineStageFlags2, Rect2D, RenderingAttachmentInfo, RenderingInfo,
            SemaphoreSubmitInfo, SemaphoreSubmitInfoKHR, ShaderStageFlags, SubmitInfo2, Viewport,
            WriteDescriptorSet, QUEUE_FAMILY_FOREIGN_EXT,
        },
        Device,
    },
    enum_map::{enum_map, Enum, EnumMap},
    isnt::std_1::collections::IsntHashMapExt,
    std::{
        cell::{Cell, RefCell},
        fmt::{Debug, Formatter},
        mem, ptr,
        rc::Rc,
        slice,
    },
    uapi::OwnedFd,
};

pub struct VulkanRenderer {
    pub(super) formats: Rc<AHashMap<u32, GfxFormat>>,
    pub(super) device: Rc<VulkanDevice>,
    pub(super) pipelines: CopyHashMap<vk::Format, Rc<VulkanFormatPipelines>>,
    pub(super) command_pool: Rc<VulkanCommandPool>,
    pub(super) command_buffers: Stack<Rc<VulkanCommandBuffer>>,
    pub(super) wait_semaphores: Stack<Rc<VulkanSemaphore>>,
    pub(super) total_buffers: NumCell<usize>,
    pub(super) memory: RefCell<Memory>,
    pub(super) pending_frames: CopyHashMap<u64, Rc<PendingFrame>>,
    pub(super) pending_uploads: CopyHashMap<u64, SpawnedFuture<()>>,
    pub(super) allocator: Rc<VulkanAllocator>,
    pub(super) last_point: NumCell<u64>,
    pub(super) buffer_resv_user: BufferResvUser,
    pub(super) eng: Rc<AsyncEngine>,
    pub(super) ring: Rc<IoUring>,
    pub(super) fill_vert_shader: Rc<VulkanShader>,
    pub(super) fill_frag_shader: Rc<VulkanShader>,
    pub(super) tex_vert_shader: Rc<VulkanShader>,
    pub(super) tex_frag_shader: Rc<VulkanShader>,
    pub(super) tex_frag_mult_opaque_shader: Rc<VulkanShader>,
    pub(super) tex_frag_mult_alpha_shader: Rc<VulkanShader>,
    pub(super) tex_descriptor_set_layout: Rc<VulkanDescriptorSetLayout>,
    pub(super) defunct: Cell<bool>,
    pub(super) pending_cpu_jobs: CopyHashMap<u64, PendingJob>,
    pub(super) shm_allocator: Rc<VulkanThreadedAllocator>,
}

pub(super) struct UsedTexture {
    tex: Rc<VulkanImage>,
    resv: Option<Rc<dyn BufferResv>>,
    acquire_sync: AcquireSync,
    release_sync: ReleaseSync,
}

#[derive(Enum)]
pub(super) enum TexCopyType {
    Identity,
    Multiply,
}

#[derive(Enum)]
pub(super) enum TexSourceType {
    Opaque,
    HasAlpha,
}

#[derive(Default)]
pub(super) struct Memory {
    sample: Vec<Rc<VulkanImage>>,
    textures: Vec<UsedTexture>,
    image_barriers: Vec<ImageMemoryBarrier2<'static>>,
    wait_semaphores: Vec<Rc<VulkanSemaphore>>,
    wait_semaphore_infos: Vec<SemaphoreSubmitInfo<'static>>,
    release_fence: Option<Rc<VulkanFence>>,
    release_sync_file: Option<SyncFile>,
}

pub(super) struct PendingFrame {
    point: u64,
    renderer: Rc<VulkanRenderer>,
    cmd: Cell<Option<Rc<VulkanCommandBuffer>>>,
    _textures: Vec<UsedTexture>,
    wait_semaphores: Cell<Vec<Rc<VulkanSemaphore>>>,
    waiter: Cell<Option<SpawnedFuture<()>>>,
    _release_fence: Option<Rc<VulkanFence>>,
}

pub(super) struct VulkanFormatPipelines {
    pub(super) fill: Rc<VulkanPipeline>,
    pub(super) tex: EnumMap<TexCopyType, EnumMap<TexSourceType, Rc<VulkanPipeline>>>,
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
        let tex_descriptor_set_layout = self.create_descriptor_set_layout(&sampler)?;
        let tex_vert_shader = self.create_shader(TEX_VERT)?;
        let tex_frag_shader = self.create_shader(TEX_FRAG)?;
        let tex_frag_mult_opaque_shader = self.create_shader(TEX_FRAG_MULT_OPAQUE)?;
        let tex_frag_mult_alpha_shader = self.create_shader(TEX_FRAG_MULT_ALPHA)?;
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
        let render = Rc::new(VulkanRenderer {
            formats: Rc::new(formats),
            device: self.clone(),
            pipelines: Default::default(),
            command_pool,
            command_buffers: Default::default(),
            wait_semaphores: Default::default(),
            total_buffers: Default::default(),
            memory: Default::default(),
            pending_frames: Default::default(),
            pending_uploads: Default::default(),
            allocator,
            last_point: Default::default(),
            buffer_resv_user: Default::default(),
            eng: eng.clone(),
            ring: ring.clone(),
            fill_vert_shader,
            fill_frag_shader,
            tex_vert_shader,
            tex_frag_shader,
            tex_frag_mult_opaque_shader,
            tex_frag_mult_alpha_shader,
            tex_descriptor_set_layout,
            defunct: Cell::new(false),
            pending_cpu_jobs: Default::default(),
            shm_allocator,
        });
        render.get_or_create_pipelines(XRGB8888.vk_format)?;
        Ok(render)
    }
}

impl VulkanRenderer {
    fn get_or_create_pipelines(
        &self,
        format: vk::Format,
    ) -> Result<Rc<VulkanFormatPipelines>, VulkanError> {
        if let Some(pl) = self.pipelines.get(&format) {
            return Ok(pl);
        }
        let fill = self
            .device
            .create_pipeline::<FillVertPushConstants, FillFragPushConstants>(
                PipelineCreateInfo {
                    format,
                    vert: self.fill_vert_shader.clone(),
                    frag: self.fill_frag_shader.clone(),
                    alpha: true,
                    frag_descriptor_set_layout: None,
                },
            )?;
        let create_tex_pipeline = |alpha| {
            self.device
                .create_pipeline::<TexVertPushConstants, ()>(PipelineCreateInfo {
                    format,
                    vert: self.tex_vert_shader.clone(),
                    frag: self.tex_frag_shader.clone(),
                    alpha,
                    frag_descriptor_set_layout: Some(self.tex_descriptor_set_layout.clone()),
                })
        };
        let create_tex_mult_pipeline = |frag: &Rc<VulkanShader>| {
            self.device
                .create_pipeline::<TexVertPushConstants, TexFragPushConstants>(PipelineCreateInfo {
                    format,
                    vert: self.tex_vert_shader.clone(),
                    frag: frag.clone(),
                    alpha: true,
                    frag_descriptor_set_layout: Some(self.tex_descriptor_set_layout.clone()),
                })
        };
        let tex_opaque = create_tex_pipeline(false)?;
        let tex_alpha = create_tex_pipeline(true)?;
        let tex_mult_opaque = create_tex_mult_pipeline(&self.tex_frag_mult_opaque_shader)?;
        let tex_mult_alpha = create_tex_mult_pipeline(&self.tex_frag_mult_alpha_shader)?;
        let pipelines = Rc::new(VulkanFormatPipelines {
            fill,
            tex: enum_map! {
                TexCopyType::Identity => enum_map! {
                    TexSourceType::HasAlpha => tex_alpha.clone(),
                    TexSourceType::Opaque => tex_opaque.clone(),
                },
                TexCopyType::Multiply => enum_map! {
                    TexSourceType::HasAlpha => tex_mult_alpha.clone(),
                    TexSourceType::Opaque => tex_mult_opaque.clone(),
                },
            },
        });
        self.pipelines.set(format, pipelines.clone());
        Ok(pipelines)
    }

    pub(super) fn allocate_point(&self) -> u64 {
        self.last_point.fetch_add(1) + 1
    }

    fn collect_memory(&self, opts: &[GfxApiOpt]) {
        zone!("collect_memory");
        let mut memory = self.memory.borrow_mut();
        memory.sample.clear();
        for cmd in opts {
            if let GfxApiOpt::CopyTexture(c) = cmd {
                let tex = c.tex.clone().into_vk(&self.device.device);
                if tex.contents_are_undefined.get() {
                    continue;
                }
                if let VulkanImageMemory::DmaBuf(_) = &tex.ty {
                    memory.sample.push(tex.clone())
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

    fn initial_barriers(&self, buf: CommandBuffer, fb: &VulkanImage) {
        zone!("initial_barriers");
        let mut memory = self.memory.borrow_mut();
        let memory = &mut *memory;
        memory.image_barriers.clear();
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
        for img in &memory.sample {
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
        let dep_info = DependencyInfoKHR::default().image_memory_barriers(&memory.image_barriers);
        unsafe {
            self.device.device.cmd_pipeline_barrier2(buf, &dep_info);
        }
    }

    fn begin_rendering(&self, buf: CommandBuffer, fb: &VulkanImage, clear: Option<&Color>) {
        zone!("begin_rendering");
        let rendering_attachment_info = {
            let mut rai = RenderingAttachmentInfo::default()
                .image_view(fb.render_view.unwrap_or(fb.texture_view))
                .image_layout(ImageLayout::GENERAL)
                .load_op(AttachmentLoadOp::LOAD)
                .store_op(AttachmentStoreOp::STORE);
            if let Some(clear) = clear {
                rai = rai
                    .clear_value(ClearValue {
                        color: ClearColorValue {
                            float32: clear.to_array_srgb(),
                        },
                    })
                    .load_op(AttachmentLoadOp::CLEAR);
            }
            rai
        };
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
    ) -> Result<(), VulkanError> {
        zone!("record_draws");
        let pipelines = self.get_or_create_pipelines(fb.format.vk_format)?;
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
                    bind(&pipelines.fill);
                    let vert = FillVertPushConstants {
                        pos: r.rect.to_points(),
                    };
                    let frag = FillFragPushConstants {
                        color: r.color.to_array_srgb(),
                    };
                    unsafe {
                        dev.cmd_push_constants(
                            buf,
                            pipelines.fill.pipeline_layout,
                            ShaderStageFlags::VERTEX,
                            0,
                            uapi::as_bytes(&vert),
                        );
                        dev.cmd_push_constants(
                            buf,
                            pipelines.fill.pipeline_layout,
                            ShaderStageFlags::FRAGMENT,
                            pipelines.fill.frag_push_offset,
                            uapi::as_bytes(&frag),
                        );
                        dev.cmd_draw(buf, 4, 1, 0, 0);
                    }
                }
                GfxApiOpt::CopyTexture(c) => {
                    let tex = c.tex.as_vk(&self.device.device);
                    if tex.contents_are_undefined.get() {
                        log::warn!("Ignoring undefined texture");
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
                    bind(pipeline);
                    let vert = TexVertPushConstants {
                        pos: c.target.to_points(),
                        tex_pos: c.source.to_points(),
                    };
                    let image_info = DescriptorImageInfo::default()
                        .image_view(tex.texture_view)
                        .image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                    let write_descriptor_set = WriteDescriptorSet::default()
                        .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(slice::from_ref(&image_info));
                    unsafe {
                        self.device.push_descriptor.cmd_push_descriptor_set(
                            buf,
                            PipelineBindPoint::GRAPHICS,
                            pipeline.pipeline_layout,
                            0,
                            slice::from_ref(&write_descriptor_set),
                        );
                        dev.cmd_push_constants(
                            buf,
                            pipeline.pipeline_layout,
                            ShaderStageFlags::VERTEX,
                            0,
                            uapi::as_bytes(&vert),
                        );
                        if let Some(alpha) = c.alpha {
                            let frag = TexFragPushConstants { alpha };
                            dev.cmd_push_constants(
                                buf,
                                pipeline.pipeline_layout,
                                ShaderStageFlags::FRAGMENT,
                                mem::size_of_val(&vert) as _,
                                uapi::as_bytes(&frag),
                            );
                        }
                        dev.cmd_draw(buf, 4, 1, 0, 0);
                    }
                }
            }
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
        let image_copy = ImageCopy2::default()
            .src_subresource(image_subresource_layers)
            .dst_subresource(image_subresource_layers)
            .extent(Extent3D {
                width: fb.width,
                height: fb.height,
                depth: 1,
            });
        let copy_image_info = CopyImageInfo2::default()
            .src_image(fb.image)
            .src_image_layout(ImageLayout::TRANSFER_SRC_OPTIMAL)
            .dst_image(bridge.dmabuf_image)
            .dst_image_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
            .regions(slice::from_ref(&image_copy));
        unsafe {
            self.device.device.cmd_copy_image2(buf, &copy_image_info);
        }
    }

    fn final_barriers(&self, buf: CommandBuffer, fb: &VulkanImage) {
        zone!("final_barriers");
        let mut memory = self.memory.borrow_mut();
        let memory = &mut *memory;
        memory.image_barriers.clear();
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
        for img in &memory.sample {
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

    fn create_wait_semaphores(&self, fb: &VulkanImage) -> Result<(), VulkanError> {
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
            &AcquireSync::Implicit,
            DMA_BUF_SYNC_WRITE,
        )?;
        Ok(())
    }

    fn import_release_semaphore(&self, fb: &VulkanImage) {
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
        for texture in &mut memory.textures {
            import(
                &texture.tex,
                texture.release_sync,
                texture.resv.take(),
                DMA_BUF_SYNC_READ,
            );
        }
        import(fb, ReleaseSync::Implicit, None, DMA_BUF_SYNC_WRITE);
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

    fn store_layouts(&self, fb: &VulkanImage) {
        fb.is_undefined.set(false);
    }

    fn create_pending_frame(self: &Rc<Self>, buf: Rc<VulkanCommandBuffer>) {
        zone!("create_pending_frame");
        let point = self.allocate_point();
        let mut memory = self.memory.borrow_mut();
        let frame = Rc::new(PendingFrame {
            point,
            renderer: self.clone(),
            cmd: Cell::new(Some(buf)),
            _textures: mem::take(&mut memory.textures),
            wait_semaphores: Cell::new(mem::take(&mut memory.wait_semaphores)),
            waiter: Cell::new(None),
            _release_fence: memory.release_fence.take(),
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

    pub fn read_pixels(
        self: &Rc<Self>,
        tex: &Rc<VulkanImage>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
        dst: &[Cell<u8>],
    ) -> Result<(), VulkanError> {
        if x < 0 || y < 0 || width <= 0 || height <= 0 || stride <= 0 {
            return Err(VulkanError::InvalidShmParameters {
                x,
                y,
                width,
                height,
                stride,
            });
        }
        let width = width as u32;
        let height = height as u32;
        let stride = stride as u32;
        if x == 0 && y == 0 && width == tex.width && height == tex.height && format == tex.format {
            return self.read_all_pixels(tex, stride, dst);
        }
        let tmp_tex = self.create_shm_texture(
            format,
            width as i32,
            height as i32,
            stride as i32,
            &[],
            true,
            None,
        )?;
        (&*tmp_tex as &dyn GfxFramebuffer)
            .copy_texture(
                &(tex.clone() as _),
                AcquireSync::None,
                ReleaseSync::None,
                x,
                y,
            )
            .map_err(VulkanError::GfxError)?;
        self.read_all_pixels(&tmp_tex, stride, dst)
    }

    fn read_all_pixels(
        self: &Rc<Self>,
        tex: &VulkanImage,
        stride: u32,
        dst: &[Cell<u8>],
    ) -> Result<(), VulkanError> {
        let Some(shm_info) = &tex.format.shm_info else {
            return Err(VulkanError::UnsupportedShmFormat(tex.format.name));
        };
        if stride < tex.width * shm_info.bpp || stride % shm_info.bpp != 0 {
            return Err(VulkanError::InvalidStride);
        }
        let size = stride as u64 * tex.height as u64;
        if size != dst.len() as u64 {
            return Err(VulkanError::InvalidBufferSize);
        }
        let region = BufferImageCopy::default()
            .buffer_row_length(stride / shm_info.bpp)
            .buffer_image_height(tex.height)
            .image_subresource(ImageSubresourceLayers {
                aspect_mask: ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_extent(Extent3D {
                width: tex.width,
                height: tex.height,
                depth: 1,
            });
        let staging =
            self.device
                .create_staging_buffer(&self.allocator, size, false, true, true)?;
        let initial_tex_barrier;
        let initial_buffer_barrier = BufferMemoryBarrier2::default()
            .buffer(staging.buffer)
            .offset(0)
            .size(staging.size)
            .dst_access_mask(AccessFlags2::TRANSFER_WRITE)
            .dst_stage_mask(PipelineStageFlags2::TRANSFER);
        let mut initial_barriers = DependencyInfo::default()
            .buffer_memory_barriers(slice::from_ref(&initial_buffer_barrier));
        if tex.bridge.is_none() {
            initial_tex_barrier = image_barrier()
                .src_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                .dst_queue_family_index(self.device.graphics_queue_idx)
                .image(tex.image)
                .old_layout(ImageLayout::GENERAL)
                .new_layout(ImageLayout::TRANSFER_SRC_OPTIMAL)
                .dst_access_mask(AccessFlags2::TRANSFER_READ)
                .dst_stage_mask(PipelineStageFlags2::TRANSFER);
            initial_barriers =
                initial_barriers.image_memory_barriers(slice::from_ref(&initial_tex_barrier));
        }
        let final_tex_barrier;
        let final_buffer_barrier = BufferMemoryBarrier2::default()
            .buffer(staging.buffer)
            .offset(0)
            .size(staging.size)
            .src_access_mask(AccessFlags2::TRANSFER_WRITE)
            .src_stage_mask(PipelineStageFlags2::TRANSFER)
            .dst_access_mask(AccessFlags2::HOST_READ)
            .dst_stage_mask(PipelineStageFlags2::HOST);
        let mut final_barriers = DependencyInfo::default()
            .buffer_memory_barriers(slice::from_ref(&final_buffer_barrier));
        if tex.bridge.is_none() {
            final_tex_barrier = image_barrier()
                .src_queue_family_index(self.device.graphics_queue_idx)
                .dst_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                .image(tex.image)
                .old_layout(ImageLayout::TRANSFER_SRC_OPTIMAL)
                .new_layout(ImageLayout::GENERAL)
                .src_access_mask(AccessFlags2::TRANSFER_READ)
                .src_stage_mask(PipelineStageFlags2::TRANSFER);
            final_barriers =
                final_barriers.image_memory_barriers(slice::from_ref(&final_tex_barrier));
        }
        let buf = self.allocate_command_buffer()?;
        let mut semaphores = vec![];
        let mut semaphore_infos = vec![];
        if let VulkanImageMemory::DmaBuf(buf) = &tex.ty {
            for plane in &buf.template.dmabuf.planes {
                let fd = dma_buf_export_sync_file(&plane.fd, DMA_BUF_SYNC_READ)
                    .map_err(VulkanError::IoctlExportSyncFile)?;
                let semaphore = self.allocate_semaphore()?;
                semaphore.import_sync_file(fd)?;
                let semaphore_info = SemaphoreSubmitInfo::default()
                    .semaphore(semaphore.semaphore)
                    .stage_mask(PipelineStageFlags2::TOP_OF_PIPE);
                semaphores.push(semaphore);
                semaphore_infos.push(semaphore_info);
            }
        }
        let command_buffer_info = CommandBufferSubmitInfo::default().command_buffer(buf.buffer);
        let submit_info = SubmitInfo2::default()
            .wait_semaphore_infos(&semaphore_infos)
            .command_buffer_infos(slice::from_ref(&command_buffer_info));
        let begin_info =
            CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .device
                .begin_command_buffer(buf.buffer, &begin_info)
                .map_err(VulkanError::BeginCommandBuffer)?;
            self.device
                .device
                .cmd_pipeline_barrier2(buf.buffer, &initial_barriers);
            self.device.device.cmd_copy_image_to_buffer(
                buf.buffer,
                tex.image,
                ImageLayout::TRANSFER_SRC_OPTIMAL,
                staging.buffer,
                &[region],
            );
            self.device
                .device
                .cmd_pipeline_barrier2(buf.buffer, &final_barriers);
            self.device
                .device
                .end_command_buffer(buf.buffer)
                .map_err(VulkanError::EndCommandBuffer)?;
            self.device
                .device
                .queue_submit2(
                    self.device.graphics_queue,
                    slice::from_ref(&submit_info),
                    Fence::null(),
                )
                .map_err(VulkanError::Submit)?;
        }
        self.block();
        self.command_buffers.push(buf);
        for semaphore in semaphores {
            self.wait_semaphores.push(semaphore);
        }
        staging.download(|mem, size| unsafe {
            ptr::copy_nonoverlapping(mem, dst.as_ptr() as _, size);
        })?;
        Ok(())
    }

    pub fn execute(
        self: &Rc<Self>,
        fb: &VulkanImage,
        opts: &[GfxApiOpt],
        clear: Option<&Color>,
    ) -> Result<Option<SyncFile>, VulkanError> {
        zone!("execute");
        let res = self.try_execute(fb, opts, clear);
        let sync_file = {
            let mut memory = self.memory.borrow_mut();
            memory.textures.clear();
            memory.sample.clear();
            memory.wait_semaphores.clear();
            memory.release_fence.take();
            memory.release_sync_file.take()
        };
        res.map(|_| sync_file)
    }

    pub(super) fn allocate_command_buffer(&self) -> Result<Rc<VulkanCommandBuffer>, VulkanError> {
        zone!("allocate_command_buffer");
        let buf = match self.command_buffers.pop() {
            Some(b) => b,
            _ => {
                self.total_buffers.fetch_add(1);
                self.command_pool.allocate_buffer()?
            }
        };
        Ok(buf)
    }

    fn allocate_semaphore(&self) -> Result<Rc<VulkanSemaphore>, VulkanError> {
        zone!("allocate_semaphore");
        let semaphore = match self.wait_semaphores.pop() {
            Some(s) => s,
            _ => self.device.create_semaphore()?,
        };
        Ok(semaphore)
    }

    fn try_execute(
        self: &Rc<Self>,
        fb: &VulkanImage,
        opts: &[GfxApiOpt],
        clear: Option<&Color>,
    ) -> Result<(), VulkanError> {
        self.check_defunct()?;
        let buf = self.allocate_command_buffer()?;
        self.collect_memory(opts);
        self.begin_command_buffer(buf.buffer)?;
        self.initial_barriers(buf.buffer, fb);
        self.begin_rendering(buf.buffer, fb, clear);
        self.set_viewport(buf.buffer, fb);
        self.record_draws(buf.buffer, fb, opts)?;
        self.end_rendering(buf.buffer);
        self.copy_bridge_to_dmabuf(buf.buffer, fb);
        self.final_barriers(buf.buffer, fb);
        self.end_command_buffer(buf.buffer)?;
        self.create_wait_semaphores(fb)?;
        self.submit(buf.buffer)?;
        self.import_release_semaphore(fb);
        self.store_layouts(fb);
        self.create_pending_frame(buf);
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
        let mut pending_uploads = self.pending_uploads.lock();
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
        frame.renderer.command_buffers.push(buf);
    }
    for wait_semaphore in frame.wait_semaphores.take() {
        frame.renderer.wait_semaphores.push(wait_semaphore);
    }
    renderer.pending_frames.remove(&frame.point);
}
