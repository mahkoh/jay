use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        cmm::{
            cmm_description::{ColorDescription, LinearColorDescription, LinearColorDescriptionId},
            cmm_transfer_function::TransferFunction,
            cmm_transform::ColorMatrix,
        },
        cpu_worker::PendingJob,
        gfx_api::{
            AcquireSync, BufferResv, BufferResvUser, GfxApiOpt, GfxBlendBuffer, GfxFormat,
            GfxTexture, GfxWriteModifier, ReleaseSync, SyncFile,
        },
        gfx_apis::vulkan::{
            VulkanError,
            allocator::{VulkanAllocator, VulkanThreadedAllocator},
            buffer_cache::{GenericBufferWriter, VulkanBuffer, VulkanBufferCache},
            command::{VulkanCommandBuffer, VulkanCommandPool},
            descriptor::VulkanDescriptorSetLayout,
            descriptor_buffer::VulkanDescriptorBufferWriter,
            device::VulkanDevice,
            fence::VulkanFence,
            image::{QueueFamily, QueueState, QueueTransfer, VulkanImage, VulkanImageMemory},
            pipeline::{PipelineCreateInfo, VulkanPipeline},
            sampler::VulkanSampler,
            semaphore::VulkanSemaphore,
            shaders::{
                FILL_FRAG, FILL_VERT, FillPushConstants, LEGACY_FILL_FRAG, LEGACY_FILL_VERT,
                LEGACY_TEX_FRAG, LEGACY_TEX_VERT, LegacyFillPushConstants, LegacyTexPushConstants,
                OUT_FRAG, OUT_VERT, OutPushConstants, TEX_FRAG, TEX_VERT, TexColorManagementData,
                TexPushConstants, TexVertex, VulkanShader,
            },
            transfer_functions::{TF_LINEAR, TransferFunctionExt},
        },
        io_uring::IoUring,
        rect::{Rect, Region},
        theme::Color,
        utils::{copyhashmap::CopyHashMap, errorfmt::ErrorFmt, numcell::NumCell, stack::Stack},
        video::dmabuf::{DMA_BUF_SYNC_READ, DMA_BUF_SYNC_WRITE, dma_buf_export_sync_file},
    },
    ahash::AHashMap,
    arrayvec::ArrayVec,
    ash::{
        Device,
        vk::{
            self, AccessFlags2, AttachmentLoadOp, AttachmentStoreOp, BufferUsageFlags,
            ClearAttachment, ClearColorValue, ClearRect, ClearValue, CommandBuffer,
            CommandBufferBeginInfo, CommandBufferSubmitInfo, CommandBufferUsageFlags,
            CopyImageInfo2, DependencyInfoKHR, DescriptorAddressInfoEXT,
            DescriptorBufferBindingInfoEXT, DescriptorDataEXT, DescriptorGetInfoEXT,
            DescriptorImageInfo, DescriptorType, DeviceAddress, DeviceSize, Extent2D, Extent3D,
            ImageAspectFlags, ImageCopy2, ImageLayout, ImageMemoryBarrier2, ImageSubresourceLayers,
            ImageSubresourceRange, Offset2D, Offset3D, PipelineBindPoint, PipelineStageFlags2,
            QUEUE_FAMILY_FOREIGN_EXT, Rect2D, RenderingAttachmentInfo, RenderingInfo,
            SemaphoreSubmitInfo, SemaphoreSubmitInfoKHR, ShaderStageFlags, SubmitInfo2, Viewport,
            WriteDescriptorSet,
        },
    },
    isnt::std_1::{collections::IsntHashMapExt, primitive::IsntSliceExt},
    jay_algorithms::rect::Tag,
    linearize::{Linearize, LinearizeExt, StaticMap, static_map},
    std::{
        any::Any,
        borrow::Cow,
        cell::{Cell, RefCell},
        collections::hash_map::Entry,
        fmt::{Debug, Formatter},
        mem,
        ops::Range,
        ptr,
        rc::{Rc, Weak},
        slice,
    },
    uapi::OwnedFd,
};

pub struct VulkanRenderer {
    pub(super) formats: Rc<AHashMap<u32, GfxFormat>>,
    pub(super) device: Rc<VulkanDevice>,
    pub(super) fill_pipelines: CopyHashMap<vk::Format, FillPipelines>,
    pub(super) tex_pipelines:
        StaticMap<TransferFunction, CopyHashMap<vk::Format, Rc<TexPipelines>>>,
    pub(super) out_pipelines:
        StaticMap<TransferFunction, CopyHashMap<OutPipelineKey, Rc<VulkanPipeline>>>,
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
    pub(super) out_vert_shader: Option<Rc<VulkanShader>>,
    pub(super) out_frag_shader: Option<Rc<VulkanShader>>,
    pub(super) tex_descriptor_set_layouts: ArrayVec<Rc<VulkanDescriptorSetLayout>, 2>,
    pub(super) out_descriptor_set_layout: Option<Rc<VulkanDescriptorSetLayout>>,
    pub(super) defunct: Cell<bool>,
    pub(super) pending_cpu_jobs: CopyHashMap<u64, PendingJob>,
    pub(super) shm_allocator: Rc<VulkanThreadedAllocator>,
    pub(super) _sampler: Rc<VulkanSampler>,
    pub(super) sampler_descriptor: Box<[u8]>,
    pub(super) sampler_descriptor_buffer_cache: Rc<VulkanBufferCache>,
    pub(super) resource_descriptor_buffer_cache: Rc<VulkanBufferCache>,
    pub(super) blend_buffers: RefCell<AHashMap<(u32, u32), Weak<VulkanImage>>>,
    pub(super) shader_buffer_cache: Rc<VulkanBufferCache>,
    pub(super) uniform_buffer_cache: Rc<VulkanBufferCache>,
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

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Linearize)]
pub(super) enum TexCopyType {
    Identity,
    Multiply,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Linearize)]
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
    used_buffers: ArrayVec<VulkanBuffer, 4>,
    paint_bounds: StaticMap<RenderPass, Option<PaintRegion>>,
    paint_regions: StaticMap<RenderPass, Vec<PaintRegion>>,
    clear_rects: StaticMap<RenderPass, Vec<ClearRect>>,
    image_copy_regions: Vec<ImageCopy2<'static>>,
    sampler_descriptor_buffer_writer: VulkanDescriptorBufferWriter,
    resource_descriptor_buffer_writer: VulkanDescriptorBufferWriter,
    regions_1: Vec<Rect>,
    regions_2: Vec<Rect<u32>>,
    ops: StaticMap<RenderPass, Vec<VulkanOp>>,
    ops_tmp: StaticMap<RenderPass, Vec<VulkanOp>>,
    fill_targets: Vec<Point>,
    tex_targets: Vec<[Point; 2]>,
    data_buffer: Vec<u8>,
    out_address: DeviceAddress,
    color_transforms: ColorTransforms,
    uniform_buffer_writer: GenericBufferWriter,
    uniform_buffer_descriptor_cache: Option<Box<[u8]>>,
    blend_buffer_descriptor_buffer_offset: DeviceAddress,
}

type Point = [[f32; 2]; 4];

enum VulkanOp {
    Fill(VulkanFillOp),
    Tex(VulkanTexOp),
}

struct VulkanTexOp {
    tex: Rc<VulkanImage>,
    range: Range<usize>,
    buffer_resv: Option<Rc<dyn BufferResv>>,
    acquire_sync: Option<AcquireSync>,
    release_sync: ReleaseSync,
    alpha: f32,
    source_type: TexSourceType,
    copy_type: TexCopyType,
    range_address: DeviceAddress,
    instances: u32,
    tex_cd: Rc<ColorDescription>,
    color_management_data_address: Option<DeviceAddress>,
    resource_descriptor_buffer_offset: DeviceAddress,
}

struct VulkanFillOp {
    range: Range<usize>,
    color: [f32; 4],
    source_type: TexSourceType,
    range_address: DeviceAddress,
    instances: u32,
}

#[derive(Copy, Clone, Debug, Linearize, Eq, PartialEq)]
pub(super) enum RenderPass {
    BlendBuffer,
    FrameBuffer,
}

#[derive(Copy, Clone)]
struct PaintRegion {
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
    _used_buffers: ArrayVec<VulkanBuffer, 4>,
}

type FillPipelines = Rc<StaticMap<TexSourceType, Rc<VulkanPipeline>>>;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct TexPipelineKey {
    tex_copy_type: TexCopyType,
    tex_source_type: TexSourceType,
    eotf: TransferFunction,
    has_color_management_data: bool,
}

pub(super) struct TexPipelines {
    format: vk::Format,
    oetf: TransferFunction,
    pipelines: CopyHashMap<TexPipelineKey, Rc<VulkanPipeline>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub(super) struct OutPipelineKey {
    format: vk::Format,
    eotf: TransferFunction,
}

impl VulkanDevice {
    pub fn create_renderer(
        self: &Rc<Self>,
        eng: &Rc<AsyncEngine>,
        ring: &Rc<IoUring>,
    ) -> Result<Rc<VulkanRenderer>, VulkanError> {
        let sampler = self.create_sampler()?;
        let fill_vert_shader;
        let fill_frag_shader;
        let tex_vert_shader;
        let tex_frag_shader;
        let out_vert_shader;
        let out_frag_shader;
        let mut tex_descriptor_set_layouts = ArrayVec::new();
        if self.descriptor_buffer.is_some() {
            tex_vert_shader = self.create_shader(TEX_VERT)?;
            tex_frag_shader = self.create_shader(TEX_FRAG)?;
            fill_vert_shader = self.create_shader(FILL_VERT)?;
            fill_frag_shader = self.create_shader(FILL_FRAG)?;
            out_vert_shader = Some(self.create_shader(OUT_VERT)?);
            out_frag_shader = Some(self.create_shader(OUT_FRAG)?);
            tex_descriptor_set_layouts
                .push(self.create_tex_sampler_descriptor_set_layout(&sampler)?);
            tex_descriptor_set_layouts.push(self.create_tex_resource_descriptor_set_layout()?);
        } else {
            tex_vert_shader = self.create_shader(LEGACY_TEX_VERT)?;
            tex_frag_shader = self.create_shader(LEGACY_TEX_FRAG)?;
            fill_vert_shader = self.create_shader(LEGACY_FILL_VERT)?;
            fill_frag_shader = self.create_shader(LEGACY_FILL_FRAG)?;
            out_vert_shader = None;
            out_frag_shader = None;
            tex_descriptor_set_layouts
                .push(self.create_tex_legacy_descriptor_set_layout(&sampler)?);
        }
        let out_descriptor_set_layout = self
            .descriptor_buffer
            .as_ref()
            .map(|db| self.create_out_descriptor_set_layout(db))
            .transpose()?;
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
            VulkanBufferCache::for_descriptor_buffer(self, &allocator, true);
        let resource_descriptor_buffer_cache =
            VulkanBufferCache::for_descriptor_buffer(self, &allocator, false);
        let shader_buffer_cache = {
            // TODO: https://github.com/KhronosGroup/Vulkan-Samples/issues/1286
            let usage = BufferUsageFlags::SHADER_DEVICE_ADDRESS | BufferUsageFlags::STORAGE_BUFFER;
            let align = 8;
            VulkanBufferCache::new(self, &allocator, usage, align)
        };
        let uniform_buffer_cache = {
            let usage = BufferUsageFlags::SHADER_DEVICE_ADDRESS | BufferUsageFlags::UNIFORM_BUFFER;
            let align = align_of::<TexColorManagementData>() as DeviceSize;
            VulkanBufferCache::new(self, &allocator, usage, align)
        };
        let render = Rc::new(VulkanRenderer {
            formats: Rc::new(formats),
            device: self.clone(),
            fill_pipelines: Default::default(),
            tex_pipelines: Default::default(),
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
            tex_descriptor_set_layouts,
            out_descriptor_set_layout,
            defunct: Cell::new(false),
            pending_cpu_jobs: Default::default(),
            shm_allocator,
            sampler_descriptor: self.create_sampler_descriptor(sampler.sampler),
            _sampler: sampler,
            sampler_descriptor_buffer_cache,
            resource_descriptor_buffer_cache,
            blend_buffers: Default::default(),
            shader_buffer_cache,
            uniform_buffer_cache,
        });
        Ok(render)
    }
}

impl VulkanRenderer {
    fn get_or_create_fill_pipelines(
        &self,
        format: vk::Format,
    ) -> Result<FillPipelines, VulkanError> {
        if let Some(pl) = self.fill_pipelines.get(&format) {
            return Ok(pl);
        }
        let create_fill_pipeline = |src_has_alpha| {
            let push_size = if self.device.descriptor_buffer.is_some() {
                size_of::<FillPushConstants>()
            } else {
                size_of::<LegacyFillPushConstants>()
            };
            let info = PipelineCreateInfo {
                format,
                vert: self.fill_vert_shader.clone(),
                frag: self.fill_frag_shader.clone(),
                blend: src_has_alpha,
                src_has_alpha,
                has_alpha_mult: false,
                // all transformations are applied in the compositor
                eotf: TF_LINEAR,
                oetf: TF_LINEAR,
                descriptor_set_layouts: Default::default(),
                has_color_management_data: false,
            };
            self.device.create_pipeline2(info, push_size)
        };
        let fill_pipelines = Rc::new(static_map! {
            TexSourceType::HasAlpha => create_fill_pipeline(true)?,
            TexSourceType::Opaque => create_fill_pipeline(false)?,
        });
        self.fill_pipelines.set(format, fill_pipelines.clone());
        Ok(fill_pipelines)
    }

    fn get_or_create_tex_pipelines(
        &self,
        format: vk::Format,
        target_cd: &ColorDescription,
    ) -> Rc<TexPipelines> {
        let pipelines = &self.tex_pipelines[target_cd.transfer_function];
        match pipelines.get(&format) {
            Some(pl) => pl,
            _ => {
                let pl = Rc::new(TexPipelines {
                    format,
                    oetf: target_cd.transfer_function,
                    pipelines: Default::default(),
                });
                pipelines.set(format, pl.clone());
                pl
            }
        }
    }

    fn get_or_create_tex_pipeline(
        &self,
        pipelines: &TexPipelines,
        tex_cd: &ColorDescription,
        tex_copy_type: TexCopyType,
        tex_source_type: TexSourceType,
        has_color_management_data: bool,
    ) -> Result<Rc<VulkanPipeline>, VulkanError> {
        let key = TexPipelineKey {
            tex_copy_type,
            tex_source_type,
            eotf: tex_cd.transfer_function,
            has_color_management_data,
        };
        if let Some(pl) = pipelines.pipelines.get(&key) {
            return Ok(pl);
        }
        let src_has_alpha = match tex_source_type {
            TexSourceType::Opaque => false,
            TexSourceType::HasAlpha => true,
        };
        let has_alpha_mult = match tex_copy_type {
            TexCopyType::Identity => false,
            TexCopyType::Multiply => true,
        };
        let push_size = if self.device.descriptor_buffer.is_some() {
            size_of::<TexPushConstants>()
        } else {
            size_of::<LegacyTexPushConstants>()
        };
        let info = PipelineCreateInfo {
            format: pipelines.format,
            vert: self.tex_vert_shader.clone(),
            frag: self.tex_frag_shader.clone(),
            blend: src_has_alpha || has_alpha_mult,
            src_has_alpha,
            has_alpha_mult,
            eotf: key.eotf.to_vulkan(),
            oetf: pipelines.oetf.to_vulkan(),
            descriptor_set_layouts: self.tex_descriptor_set_layouts.clone(),
            has_color_management_data,
        };
        let pl = self.device.create_pipeline2(info, push_size)?;
        pipelines.pipelines.set(key, pl.clone());
        Ok(pl)
    }

    fn get_or_create_out_pipeline(
        &self,
        format: vk::Format,
        bb_cd: &ColorDescription,
        fb_cd: &ColorDescription,
    ) -> Result<Rc<VulkanPipeline>, VulkanError> {
        let key = OutPipelineKey {
            format,
            eotf: bb_cd.transfer_function,
        };
        let pipelines = &self.out_pipelines[fb_cd.transfer_function];
        if let Some(pl) = pipelines.get(&key) {
            return Ok(pl);
        }
        let mut descriptor_set_layouts = ArrayVec::new();
        descriptor_set_layouts.push(self.out_descriptor_set_layout.clone().unwrap());
        let out = self
            .device
            .create_pipeline::<OutPushConstants>(PipelineCreateInfo {
                format: key.format,
                vert: self.out_vert_shader.clone().unwrap(),
                frag: self.out_frag_shader.clone().unwrap(),
                blend: false,
                src_has_alpha: true,
                has_alpha_mult: false,
                eotf: key.eotf.to_vulkan(),
                oetf: fb_cd.transfer_function.to_vulkan(),
                descriptor_set_layouts,
                has_color_management_data: false,
            })?;
        pipelines.set(key, out.clone());
        Ok(out)
    }

    pub(super) fn allocate_point(&self) -> u64 {
        self.last_point.fetch_add(1) + 1
    }

    fn create_descriptor_buffers(
        &self,
        buf: CommandBuffer,
        bb: Option<&VulkanImage>,
    ) -> Result<(), VulkanError> {
        let Some(db) = &self.device.descriptor_buffer else {
            return Ok(());
        };
        zone!("create_descriptor_buffers");
        let memory = &mut *self.memory.borrow_mut();
        let sampler_writer = &mut memory.sampler_descriptor_buffer_writer;
        sampler_writer.clear();
        {
            let mut writer = sampler_writer.add_set(&self.tex_descriptor_set_layouts[0]);
            writer.write(
                self.tex_descriptor_set_layouts[0].offsets[0],
                &self.sampler_descriptor,
            );
        }
        let resource_writer = &mut memory.resource_descriptor_buffer_writer;
        resource_writer.clear();
        let uniform_buffer_descriptor_cache = memory
            .uniform_buffer_descriptor_cache
            .get_or_insert_with(|| {
                vec![0u8; self.device.uniform_buffer_descriptor_size].into_boxed_slice()
            });
        if let Some(bb) = bb {
            let layout = self.out_descriptor_set_layout.as_ref().unwrap();
            memory.blend_buffer_descriptor_buffer_offset = resource_writer.next_offset();
            let mut writer = resource_writer.add_set(layout);
            writer.write(layout.offsets[0], &bb.sampled_image_descriptor);
        }
        let tex_descriptor_set_layout = &self.tex_descriptor_set_layouts[1];
        for pass in RenderPass::variants() {
            for cmd in &mut memory.ops[pass] {
                let VulkanOp::Tex(c) = cmd else {
                    continue;
                };
                let tex = &c.tex;
                c.resource_descriptor_buffer_offset = resource_writer.next_offset();
                let mut writer = resource_writer.add_set(tex_descriptor_set_layout);
                writer.write(
                    tex_descriptor_set_layout.offsets[0],
                    &tex.sampled_image_descriptor,
                );
                if let Some(addr) = c.color_management_data_address {
                    let uniform_buffer = DescriptorAddressInfoEXT::default()
                        .address(addr)
                        .range(size_of::<TexColorManagementData>() as _);
                    let info = DescriptorGetInfoEXT::default()
                        .ty(DescriptorType::UNIFORM_BUFFER)
                        .data(DescriptorDataEXT {
                            p_uniform_buffer: &uniform_buffer,
                        });
                    unsafe {
                        db.get_descriptor(&info, uniform_buffer_descriptor_cache);
                    }
                    writer.write(
                        tex_descriptor_set_layout.offsets[1],
                        uniform_buffer_descriptor_cache,
                    );
                }
            }
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
            memory.used_buffers.push(buffer);
        }
        unsafe {
            db.cmd_bind_descriptor_buffers(buf, &infos);
        }
        Ok(())
    }

    fn convert_ops(
        &self,
        opts: &[GfxApiOpt],
        blend_cd: &ColorDescription,
        fb_cd: &ColorDescription,
    ) -> Result<(), VulkanError> {
        zone!("convert_ops");
        let memory = &mut *self.memory.borrow_mut();
        for ops in memory.ops.values_mut() {
            ops.clear();
        }
        for ops in memory.ops_tmp.values_mut() {
            ops.clear();
        }
        memory.tex_targets.clear();
        memory.fill_targets.clear();
        memory.data_buffer.clear();
        memory.uniform_buffer_writer.clear();
        memory.color_transforms.map.clear();
        let sync = |memory: &mut Memory| {
            for pass in RenderPass::variants() {
                let ops = &mut memory.ops_tmp[pass];
                ops.sort_unstable_by_key(|o| {
                    #[derive(Eq, PartialEq, PartialOrd, Ord)]
                    enum Key {
                        Fill { color: [u32; 4] },
                        Tex,
                    }
                    match o {
                        VulkanOp::Fill(f) => Key::Fill {
                            color: f.color.map(|c| c.to_bits()),
                        },
                        VulkanOp::Tex(_) => Key::Tex,
                    }
                });
                let mops = &mut memory.ops[pass];
                if self.device.descriptor_buffer.is_none() {
                    mops.append(ops);
                    continue;
                }
                for (idx, op) in ops.drain(..).enumerate() {
                    match op {
                        VulkanOp::Fill(mut f) => {
                            f.range_address = memory.data_buffer.len() as DeviceAddress;
                            f.instances = f.range.len() as u32;
                            for pos in &memory.fill_targets[f.range.clone()] {
                                memory.data_buffer.extend_from_slice(uapi::as_bytes(pos));
                            }
                            if let Some(VulkanOp::Fill(p)) = mops.last_mut()
                                && p.color == f.color
                                && idx > 0
                            {
                                p.instances += f.instances;
                                continue;
                            }
                            mops.push(VulkanOp::Fill(f));
                        }
                        VulkanOp::Tex(mut c) => {
                            c.range_address = memory.data_buffer.len() as DeviceAddress;
                            c.instances = c.range.len() as u32;
                            for &[pos, tex_pos] in &memory.tex_targets[c.range.clone()] {
                                let vertex = TexVertex { pos, tex_pos };
                                memory
                                    .data_buffer
                                    .extend_from_slice(uapi::as_bytes(&vertex));
                            }
                            mops.push(VulkanOp::Tex(c));
                        }
                    }
                }
            }
        };
        for op in opts {
            match op {
                GfxApiOpt::Sync => {
                    sync(memory);
                }
                GfxApiOpt::FillRect(fr) => {
                    let target = fr.rect.to_points();
                    for pass in RenderPass::variants() {
                        let Some(bounds) = memory.paint_bounds[pass] else {
                            continue;
                        };
                        if !bounds.intersects(&target) {
                            continue;
                        }
                        let ops = &mut memory.ops_tmp[pass];
                        let lo = memory.fill_targets.len();
                        for region in &memory.paint_regions[pass] {
                            let mut target = target;
                            if !region.constrain(&mut target, None) {
                                continue;
                            }
                            memory.fill_targets.push(target);
                        }
                        let hi = memory.fill_targets.len();
                        if lo == hi {
                            continue;
                        }
                        let target_cd = match pass {
                            RenderPass::BlendBuffer => blend_cd,
                            RenderPass::FrameBuffer => fb_cd,
                        };
                        let tf = target_cd.transfer_function;
                        let color = memory
                            .color_transforms
                            .apply_to_color(&fr.cd, target_cd, fr.color);
                        let color = color.to_array2(tf, fr.alpha);
                        let source_type = match color[3] < 1.0 {
                            false => TexSourceType::Opaque,
                            true => TexSourceType::HasAlpha,
                        };
                        ops.push(VulkanOp::Fill(VulkanFillOp {
                            range: lo..hi,
                            color,
                            source_type,
                            range_address: 0,
                            instances: 0,
                        }));
                    }
                }
                GfxApiOpt::CopyTexture(ct) => {
                    let tex = ct.tex.clone().into_vk(&self.device.device)?;
                    if tex.contents_are_undefined.get() {
                        log::warn!("Ignoring undefined texture");
                        continue;
                    }
                    if tex.queue_state.get().acquire(QueueFamily::Gfx) == QueueTransfer::Impossible
                    {
                        log::warn!("Ignoring texture owned by different queue");
                        continue;
                    }
                    let target = ct.target.to_points();
                    let source = ct.source.to_points();
                    for pass in RenderPass::variants() {
                        let Some(bounds) = memory.paint_bounds[pass] else {
                            continue;
                        };
                        if !bounds.intersects(&target) {
                            continue;
                        }
                        let ops = &mut memory.ops_tmp[pass];
                        let lo = memory.tex_targets.len();
                        for region in &memory.paint_regions[pass] {
                            let mut target = target;
                            let mut source = source;
                            if !region.constrain(&mut target, Some(&mut source)) {
                                continue;
                            }
                            memory.tex_targets.push([target, source]);
                        }
                        let hi = memory.tex_targets.len();
                        if lo == hi {
                            continue;
                        }
                        let copy_type = match ct.alpha.is_some() {
                            true => TexCopyType::Multiply,
                            false => TexCopyType::Identity,
                        };
                        let source_type = match tex.format.has_alpha && !ct.opaque {
                            true => TexSourceType::HasAlpha,
                            false => TexSourceType::Opaque,
                        };
                        let target_cd = match pass {
                            RenderPass::BlendBuffer => blend_cd,
                            RenderPass::FrameBuffer => fb_cd,
                        };
                        let color_management_data_address = memory.color_transforms.get_offset(
                            &ct.cd.linear,
                            target_cd,
                            self.device.uniform_buffer_offset_mask,
                            &mut memory.uniform_buffer_writer,
                        );
                        ops.push(VulkanOp::Tex(VulkanTexOp {
                            tex: tex.clone(),
                            range: lo..hi,
                            buffer_resv: ct.buffer_resv.clone(),
                            acquire_sync: Some(ct.acquire_sync.clone()),
                            release_sync: ct.release_sync,
                            alpha: ct.alpha.unwrap_or_default(),
                            source_type,
                            copy_type,
                            range_address: 0,
                            instances: 0,
                            tex_cd: ct.cd.clone(),
                            color_management_data_address,
                            resource_descriptor_buffer_offset: 0,
                        }));
                    }
                }
            }
        }
        sync(memory);
        Ok(())
    }

    fn create_data_buffer(&self) -> Result<(), VulkanError> {
        if self.device.descriptor_buffer.is_none() {
            return Ok(());
        }
        zone!("create_data_buffer");
        let memory = &mut *self.memory.borrow_mut();
        let buf = &mut memory.data_buffer;
        {
            memory.out_address = buf.len() as _;
            for region in &memory.paint_regions[RenderPass::BlendBuffer] {
                buf.extend_from_slice(uapi::as_bytes(&[
                    [region.x2, region.y1],
                    [region.x1, region.y1],
                    [region.x2, region.y2],
                    [region.x1, region.y2],
                ]));
            }
        }
        if buf.is_empty() {
            return Ok(());
        }
        let buffer = self.shader_buffer_cache.allocate(buf.len() as _)?;
        buffer.buffer.allocation.upload(|ptr, _| unsafe {
            ptr::copy_nonoverlapping(buf.as_ptr(), ptr, buf.len());
        })?;
        for ops in memory.ops.values_mut() {
            for op in ops {
                match op {
                    VulkanOp::Fill(f) => {
                        f.range_address += buffer.buffer.address;
                    }
                    VulkanOp::Tex(c) => {
                        c.range_address += buffer.buffer.address;
                    }
                }
            }
        }
        memory.out_address += buffer.buffer.address;
        memory.used_buffers.push(buffer);
        Ok(())
    }

    fn create_uniform_buffer(&self) -> Result<(), VulkanError> {
        if self.device.descriptor_buffer.is_none() {
            return Ok(());
        }
        zone!("create_uniform_buffer");
        let memory = &mut *self.memory.borrow_mut();
        let buf = &memory.uniform_buffer_writer;
        if buf.is_empty() {
            return Ok(());
        }
        let buffer = self.uniform_buffer_cache.allocate(buf.len() as _)?;
        buffer.buffer.allocation.upload(|ptr, _| unsafe {
            ptr::copy_nonoverlapping(buf.as_ptr(), ptr, buf.len());
        })?;
        for ops in memory.ops.values_mut() {
            for op in ops {
                if let VulkanOp::Tex(c) = op
                    && let Some(addr) = &mut c.color_management_data_address
                {
                    *addr += buffer.buffer.address;
                }
            }
        }
        memory.used_buffers.push(buffer);
        Ok(())
    }

    fn collect_memory(&self) {
        zone!("collect_memory");
        let memory = &mut *self.memory.borrow_mut();
        memory.dmabuf_sample.clear();
        memory.queue_transfer.clear();
        let execution = self.allocate_point();
        for pass in RenderPass::variants() {
            for cmd in &mut memory.ops[pass] {
                if let VulkanOp::Tex(c) = cmd {
                    let tex = &c.tex;
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
                        tex: tex.clone(),
                        resv: c.buffer_resv.take(),
                        acquire_sync: c.acquire_sync.take().unwrap(),
                        release_sync: c.release_sync,
                    });
                }
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

    fn initial_barriers(&self, buf: CommandBuffer, fb: &VulkanImage) -> Result<(), VulkanError> {
        zone!("initial_barriers");
        let mut memory = self.memory.borrow_mut();
        let memory = &mut *memory;
        memory.image_barriers.clear();
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
        target: &VulkanImage,
        clear: Option<&Color>,
        clear_cd: &LinearColorDescription,
        pass: RenderPass,
        target_cd: &ColorDescription,
    ) {
        zone!("begin_rendering");
        let memory = &mut *self.memory.borrow_mut();
        let mut load_clear = None;
        let mut manual_clear = None;
        let clear_rects = &memory.clear_rects[pass];
        if let Some(clear) = clear
            && clear_rects.is_not_empty()
        {
            let color = memory
                .color_transforms
                .apply_to_color(clear_cd, target_cd, *clear);
            let clear_value = ClearValue {
                color: ClearColorValue {
                    float32: color.to_array(target_cd.transfer_function),
                },
            };
            let use_load_clear = clear_rects.len() == 1 && {
                let rect = &clear_rects[0].rect;
                rect.offset.x == 0
                    && rect.offset.y == 0
                    && rect.extent.width == target.width
                    && rect.extent.height == target.height
            };
            if use_load_clear {
                load_clear = Some(clear_value);
            } else {
                manual_clear = Some(clear_value);
            }
        }
        let mut rendering_attachment_info = RenderingAttachmentInfo::default()
            .image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .image_view(target.render_view.unwrap_or(target.texture_view))
            .store_op(AttachmentStoreOp::STORE);
        let load_op = if let Some(clear) = load_clear {
            rendering_attachment_info = rendering_attachment_info.clear_value(clear);
            AttachmentLoadOp::CLEAR
        } else if pass == RenderPass::BlendBuffer {
            AttachmentLoadOp::DONT_CARE
        } else {
            AttachmentLoadOp::LOAD
        };
        rendering_attachment_info = rendering_attachment_info.load_op(load_op);
        let rendering_info = RenderingInfo::default()
            .render_area(Rect2D {
                offset: Default::default(),
                extent: Extent2D {
                    width: target.width,
                    height: target.height,
                },
            })
            .layer_count(1)
            .color_attachments(slice::from_ref(&rendering_attachment_info));
        unsafe {
            self.device.device.cmd_begin_rendering(buf, &rendering_info);
        }
        if clear_rects.is_not_empty() {
            if let Some(clear) = manual_clear {
                let clear_attachment = ClearAttachment::default()
                    .color_attachment(0)
                    .clear_value(clear)
                    .aspect_mask(ImageAspectFlags::COLOR);
                unsafe {
                    self.device
                        .device
                        .cmd_clear_attachments(buf, &[clear_attachment], clear_rects);
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
        target: &VulkanImage,
        pass: RenderPass,
        target_cd: &ColorDescription,
    ) -> Result<(), VulkanError> {
        zone!("record_draws");
        let memory = &*self.memory.borrow();
        let fill_pl = self.get_or_create_fill_pipelines(target.format.vk_format)?;
        let tex_pl = self.get_or_create_tex_pipelines(target.format.vk_format, target_cd);
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
        for opt in &memory.ops[pass] {
            match opt {
                VulkanOp::Fill(r) => {
                    let pipeline = &fill_pl[r.source_type];
                    bind(pipeline);
                    if self.device.descriptor_buffer.is_some() {
                        let push = FillPushConstants {
                            color: r.color,
                            vertices: r.range_address,
                            _padding1: 0,
                            _padding2: 0,
                        };
                        unsafe {
                            dev.cmd_push_constants(
                                buf,
                                pipeline.pipeline_layout,
                                ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
                                0,
                                uapi::as_bytes(&push),
                            );
                            dev.cmd_draw(buf, 4, r.instances, 0, 0);
                        }
                    } else {
                        for &pos in &memory.fill_targets[r.range.clone()] {
                            let push = LegacyFillPushConstants {
                                pos,
                                color: r.color,
                            };
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
                VulkanOp::Tex(c) => {
                    let tex = &c.tex;
                    let pipeline = self.get_or_create_tex_pipeline(
                        &tex_pl,
                        &c.tex_cd,
                        c.copy_type,
                        c.source_type,
                        c.color_management_data_address.is_some(),
                    )?;
                    bind(&pipeline);
                    let image_info = DescriptorImageInfo::default()
                        .image_view(tex.texture_view)
                        .image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                    if let Some(db) = &self.device.descriptor_buffer {
                        let push = TexPushConstants {
                            vertices: c.range_address,
                            alpha: c.alpha,
                        };
                        unsafe {
                            db.cmd_set_descriptor_buffer_offsets(
                                buf,
                                PipelineBindPoint::GRAPHICS,
                                pipeline.pipeline_layout,
                                0,
                                &[0, 1],
                                &[0, c.resource_descriptor_buffer_offset],
                            );
                            dev.cmd_push_constants(
                                buf,
                                pipeline.pipeline_layout,
                                ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
                                0,
                                uapi::as_bytes(&push),
                            );
                            dev.cmd_draw(buf, 4, c.instances, 0, 0);
                        }
                    } else {
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
                        }
                        for &[pos, tex_pos] in &memory.tex_targets[c.range.clone()] {
                            let push = LegacyTexPushConstants {
                                pos,
                                tex_pos,
                                alpha: c.alpha,
                            };
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
        }
        Ok(())
    }

    fn blend_buffer_initial_barrier(&self, buf: CommandBuffer, bb: &VulkanImage) {
        zone!("blend_buffer_initial_barrier");
        let memory = &mut *self.memory.borrow_mut();
        memory.image_barriers.clear();
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
        let dep_info = DependencyInfoKHR::default().image_memory_barriers(&memory.image_barriers);
        unsafe {
            self.device.device.cmd_pipeline_barrier2(buf, &dep_info);
        }
    }

    fn blend_buffer_copy(
        &self,
        buf: CommandBuffer,
        fb: &VulkanImage,
        fb_cd: &ColorDescription,
        bb_cd: &ColorDescription,
    ) -> Result<(), VulkanError> {
        zone!("blend_buffer_copy");
        let memory = &*self.memory.borrow();
        let db = self.device.descriptor_buffer.as_ref().unwrap();
        let pipeline = self.get_or_create_out_pipeline(fb.format.vk_format, bb_cd, fb_cd)?;
        let push = OutPushConstants {
            vertices: memory.out_address,
        };
        let instances = memory.paint_regions[RenderPass::BlendBuffer].len() as u32;
        let dev = &self.device.device;
        unsafe {
            dev.cmd_bind_pipeline(buf, PipelineBindPoint::GRAPHICS, pipeline.pipeline);
            db.cmd_set_descriptor_buffer_offsets(
                buf,
                PipelineBindPoint::GRAPHICS,
                pipeline.pipeline_layout,
                0,
                &[1],
                &[memory.blend_buffer_descriptor_buffer_offset],
            );
            dev.cmd_push_constants(
                buf,
                pipeline.pipeline_layout,
                ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
                0,
                uapi::as_bytes(&push),
            );
            dev.cmd_draw(buf, 4, instances, 0, 0);
        }
        Ok(())
    }

    fn blend_buffer_final_barrier(&self, buf: CommandBuffer, bb: &VulkanImage) {
        zone!("blend_buffer_final_barrier");
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
        unsafe {
            self.device
                .device
                .cmd_pipeline_barrier2(buf, &dependency_info);
        }
    }

    fn end_rendering(&self, buf: CommandBuffer) {
        zone!("end_rendering");
        unsafe {
            self.device.device.cmd_end_rendering(buf);
        }
    }

    fn copy_bridge_to_dmabuf(&self, buf: CommandBuffer, fb: &VulkanImage, region: &Region) {
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
        for rect in region.rects() {
            let Some([x1, y1, x2, y2]) = constrain_to_fb(fb, rect) else {
                continue;
            };
            let offset = Offset3D { x: x1, y: y1, z: 0 };
            let extent = Extent3D {
                width: (x2 - x1) as _,
                height: (y2 - y1) as _,
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
        if memory.image_copy_regions.is_not_empty() {
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
                    AcquireSync::Implicit => {
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
                    if let VulkanImageMemory::DmaBuf(buf) = &img.ty
                        && let Err(e) = buf.template.dmabuf.import_sync_file(flag, sync_file)
                    {
                        log::error!("Could not import sync file into dmabuf: {}", ErrorFmt(e));
                        log::warn!("Relying on implicit sync");
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
            if attach_async_shm_sync_file
                && let VulkanImageMemory::Internal(shm) = &texture.tex.ty
                && let Some(data) = &shm.async_data
            {
                data.last_gfx_use.set(Some(sync_file.clone()));
            }
        }
        if attach_async_shm_sync_file
            && let VulkanImageMemory::Internal(shm) = &fb.ty
            && let Some(data) = &shm.async_data
        {
            data.last_gfx_use.set(Some(sync_file.clone()));
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
                .inspect_err(self.device.idl())
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
            _used_buffers: mem::take(&mut memory.used_buffers),
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
        fb_cd: &Rc<ColorDescription>,
        opts: &[GfxApiOpt],
        clear: Option<&Color>,
        clear_cd: &Rc<LinearColorDescription>,
        region: &Region,
        blend_buffer: Option<Rc<VulkanImage>>,
        blend_cd: &Rc<ColorDescription>,
    ) -> Result<Option<SyncFile>, VulkanError> {
        zone!("execute");
        let res = self.try_execute(
            fb,
            fb_acquire_sync,
            fb_release_sync,
            fb_cd,
            opts,
            clear,
            clear_cd,
            region,
            blend_buffer,
            blend_cd,
        );
        let sync_file = {
            let mut memory = self.memory.borrow_mut();
            memory.textures.clear();
            memory.dmabuf_sample.clear();
            memory.queue_transfer.clear();
            memory.wait_semaphores.clear();
            memory.release_fence.take();
            memory.used_buffers.clear();
            memory.ops.clear();
            memory.ops_tmp.clear();
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

    fn create_regions(
        &self,
        fb: &VulkanImage,
        opts: &[GfxApiOpt],
        clear: Option<&Color>,
        region: &Region,
        bb: Option<&VulkanImage>,
    ) -> Result<(), VulkanError> {
        zone!("create_paint_regions");
        let memory = &mut *self.memory.borrow_mut();
        memory.regions_1.clear();
        memory.regions_2.clear();
        let width = fb.width as f32;
        let height = fb.height as f32;
        let mut tag = 0;
        for opt in opts.iter().rev() {
            let (opaque, fb_rect) = match opt {
                GfxApiOpt::Sync => continue,
                GfxApiOpt::FillRect(f) => (f.effective_color().is_opaque(), f.rect),
                GfxApiOpt::CopyTexture(c) => {
                    let opaque = 'opaque: {
                        if let Some(a) = c.alpha
                            && a < 1.0
                        {
                            break 'opaque false;
                        }
                        if !c.opaque {
                            let tex = c.tex.as_vk(&self.device.device)?;
                            if tex.format.has_alpha {
                                break 'opaque false;
                            }
                        }
                        true
                    };
                    (opaque, c.target)
                }
            };
            if opaque || bb.is_none() {
                tag |= 1;
            } else {
                tag += tag & 1;
            }
            let rect = fb_rect.to_rect(width, height);
            if opaque && clear.is_some() {
                memory.regions_1.push(rect);
            }
            memory.regions_2.push(rect.with_tag(tag));
        }
        let clear_region = if clear.is_some() {
            let opaque_region = Region::from_rects2(&memory.regions_1);
            region.subtract_cow(&opaque_region)
        } else {
            Cow::Owned(Region::default())
        };
        let tagged_region = Region::from_rects_tagged(&memory.regions_2).intersect_tagged(region);
        memory.regions_1.clear();
        memory.paint_regions[RenderPass::BlendBuffer].clear();
        memory.paint_regions[RenderPass::FrameBuffer].clear();
        let to_fb = |c: i32, max: u32| 2.0 * (c as f32 / max as f32) - 1.0;
        for rect in tagged_region.rects() {
            if rect.tag() == 0 && clear.is_some() {
                memory.regions_1.push(rect.untag());
            }
            let Some([x1, y1, x2, y2]) = constrain_to_fb(fb, rect) else {
                continue;
            };
            let region = match rect.tag() {
                0 => &mut memory.paint_regions[RenderPass::BlendBuffer],
                _ => &mut memory.paint_regions[RenderPass::FrameBuffer],
            };
            region.push(PaintRegion {
                x1: to_fb(x1, fb.width),
                x2: to_fb(x2, fb.width),
                y1: to_fb(y1, fb.height),
                y2: to_fb(y2, fb.height),
            });
        }
        for pass in RenderPass::variants() {
            let regions = &memory.paint_regions[pass];
            if regions.is_empty() {
                memory.paint_bounds[pass] = None;
            } else {
                let mut union = regions[0];
                for region in &regions[1..] {
                    union = union.union(region);
                }
                memory.paint_bounds[pass] = Some(union);
            }
        }
        let blend_clear = clear_region.intersect(&Region::from_rects2(&memory.regions_1));
        let opaque_clear = clear_region.subtract_cow(&blend_clear);
        // if bb.is_none() {
        //     log::info!("blend_clear = {:?}", blend_clear);
        //     log::info!("opaque_clear = {:?}", opaque_clear);
        // }
        for (pass, clear_region) in [
            (RenderPass::BlendBuffer, &blend_clear),
            (RenderPass::FrameBuffer, &opaque_clear),
        ] {
            memory.clear_rects[pass].clear();
            for rect in clear_region.rects() {
                let Some([x1, y1, x2, y2]) = constrain_to_fb(fb, rect) else {
                    continue;
                };
                memory.clear_rects[pass].push(ClearRect {
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
                    base_array_layer: 0,
                    layer_count: 1,
                });
            }
        }
        Ok(())
    }

    fn elide_blend_buffer(&self, blend_buffer: &mut Option<Rc<VulkanImage>>) {
        if blend_buffer.is_none() {
            return;
        }
        let memory = &*self.memory.borrow();
        if memory.paint_regions[RenderPass::BlendBuffer].is_empty() {
            *blend_buffer = None;
        }
    }

    fn try_execute(
        self: &Rc<Self>,
        fb: &Rc<VulkanImage>,
        fb_acquire_sync: AcquireSync,
        fb_release_sync: ReleaseSync,
        fb_cd: &Rc<ColorDescription>,
        opts: &[GfxApiOpt],
        clear: Option<&Color>,
        clear_cd: &Rc<LinearColorDescription>,
        region: &Region,
        mut blend_buffer: Option<Rc<VulkanImage>>,
        bb_cd: &Rc<ColorDescription>,
    ) -> Result<(), VulkanError> {
        self.check_defunct()?;
        self.create_regions(fb, opts, clear, region, blend_buffer.as_deref())?;
        self.elide_blend_buffer(&mut blend_buffer);
        let bb = blend_buffer.as_deref();
        let buf = self.gfx_command_buffers.allocate()?;
        self.convert_ops(opts, bb_cd, fb_cd)?;
        self.create_data_buffer()?;
        self.create_uniform_buffer()?;
        self.collect_memory();
        self.begin_command_buffer(buf.buffer)?;
        self.create_descriptor_buffers(buf.buffer, bb)?;
        self.initial_barriers(buf.buffer, fb)?;
        self.set_viewport(buf.buffer, fb);
        if let Some(bb) = bb {
            zone!("blend buffer pass");
            let rp = RenderPass::BlendBuffer;
            self.blend_buffer_initial_barrier(buf.buffer, bb);
            self.begin_rendering(buf.buffer, bb, clear, clear_cd, rp, bb_cd);
            self.record_draws(buf.buffer, bb, rp, bb_cd)?;
            self.end_rendering(buf.buffer);
            self.blend_buffer_final_barrier(buf.buffer, bb);
        }
        {
            zone!("frame buffer pass");
            let rp = RenderPass::FrameBuffer;
            self.begin_rendering(buf.buffer, fb, clear, clear_cd, rp, fb_cd);
            self.record_draws(buf.buffer, fb, rp, fb_cd)?;
            if bb.is_some() {
                self.blend_buffer_copy(buf.buffer, fb, fb_cd, bb_cd)?;
            }
            self.end_rendering(buf.buffer);
        }
        self.copy_bridge_to_dmabuf(buf.buffer, fb, region);
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
                self.device.idl()(&e);
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
    fn assert_device(&self, device: &Device) -> Result<(), VulkanError> {
        if self.renderer.device.device.handle() != device.handle() {
            return Err(VulkanError::MixedVulkanDeviceUse);
        }
        Ok(())
    }
}

impl dyn GfxTexture {
    fn as_vk(&self, device: &Device) -> Result<&VulkanImage, VulkanError> {
        let img: &VulkanImage = (self as &dyn Any)
            .downcast_ref()
            .ok_or(VulkanError::NonVulkanBuffer)?;
        img.assert_device(device)?;
        Ok(img)
    }

    pub(super) fn into_vk(self: Rc<Self>, device: &Device) -> Result<Rc<VulkanImage>, VulkanError> {
        let img: Rc<VulkanImage> = (self as Rc<dyn Any>)
            .downcast()
            .ok()
            .ok_or(VulkanError::NonVulkanBuffer)?;
        img.assert_device(device)?;
        Ok(img)
    }
}

impl dyn GfxBlendBuffer {
    pub(super) fn into_vk(self: Rc<Self>, device: &Device) -> Result<Rc<VulkanImage>, VulkanError> {
        let img: Rc<VulkanImage> = (self as Rc<dyn Any>)
            .downcast()
            .ok()
            .ok_or(VulkanError::NonVulkanBuffer)?;
        img.assert_device(device)?;
        Ok(img)
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
    fn intersects(&self, pos: &Point) -> bool {
        let mut p = *pos;
        for [x, y] in &mut p {
            *x = x.clamp(self.x1, self.x2);
            *y = y.clamp(self.y1, self.y2);
        }
        if p[0] == p[1] && p[2] == p[3] {
            return false;
        }
        if p[0] == p[2] && p[1] == p[3] {
            return false;
        }
        true
    }

    fn union(&self, other: &Self) -> Self {
        Self {
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
            x2: self.x2.max(other.x2),
            y2: self.y2.max(other.y2),
        }
    }

    fn constrain(&self, pos: &mut Point, tex_pos: Option<&mut Point>) -> bool {
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

fn constrain_to_fb<T>(fb: &VulkanImage, rect: &Rect<T>) -> Option<[i32; 4]>
where
    T: Tag,
{
    let x1 = rect.x1().max(0);
    let y1 = rect.y1().max(0);
    let x2 = rect.x2();
    let y2 = rect.y2();
    if x1 as u32 > fb.width || y1 as u32 > fb.height || x2 <= 0 || y2 <= 0 {
        return None;
    }
    let x2 = x2.min(fb.width as i32);
    let y2 = y2.min(fb.height as i32);
    Some([x1, y1, x2, y2])
}

#[derive(Default)]
struct ColorTransforms {
    map: AHashMap<[LinearColorDescriptionId; 2], ColorTransform>,
}

struct ColorTransform {
    matrix: ColorMatrix,
    offset: Option<DeviceSize>,
}

impl ColorTransforms {
    fn get_or_create(
        &mut self,
        src: &LinearColorDescription,
        dst: &ColorDescription,
    ) -> Option<&mut ColorTransform> {
        if src.embeds_into(&dst.linear) {
            return None;
        }
        let ct = match self.map.entry([src.id, dst.linear.id]) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(e) => {
                let matrix = src.color_transform(&dst.linear);
                let ct = ColorTransform {
                    matrix,
                    offset: None,
                };
                e.insert(ct)
            }
        };
        Some(ct)
    }

    fn apply_to_color(
        &mut self,
        src: &LinearColorDescription,
        dst: &ColorDescription,
        mut color: Color,
    ) -> Color {
        if let Some(ct) = self.get_or_create(src, dst) {
            color = ct.matrix * color;
        };
        color
    }

    fn get_offset(
        &mut self,
        src: &LinearColorDescription,
        dst: &ColorDescription,
        uniform_buffer_offset_mask: DeviceSize,
        writer: &mut GenericBufferWriter,
    ) -> Option<DeviceSize> {
        let ct = self.get_or_create(src, dst)?;
        if ct.offset.is_none() {
            let data = TexColorManagementData {
                matrix: ct.matrix.to_f32(),
            };
            let offset = writer.write(uniform_buffer_offset_mask, &data);
            ct.offset = Some(offset);
        }
        ct.offset
    }
}
