use {
    crate::{
        format::Format,
        gfx_api::{
            AcquireSync, AsyncShmGfxTexture, AsyncShmGfxTextureCallback,
            AsyncShmGfxTextureTransferCancellable, GfxApiOpt, GfxError, GfxFramebuffer, GfxImage,
            GfxInternalFramebuffer, GfxStagingBuffer, GfxTexture, PendingShmTransfer, ReleaseSync,
            ShmGfxTexture, ShmMemory, SyncFile,
        },
        gfx_apis::vulkan::{
            allocator::VulkanAllocation, device::VulkanDevice, format::VulkanModifierLimits,
            renderer::VulkanRenderer, shm_image::VulkanShmImage, transfer::TransferType,
            VulkanError,
        },
        rect::Region,
        theme::Color,
        utils::on_drop::OnDrop,
        video::dmabuf::{DmaBuf, PlaneVec},
    },
    ash::vk::{
        BindImageMemoryInfo, BindImagePlaneMemoryInfo, ComponentMapping, ComponentSwizzle,
        DescriptorDataEXT, DescriptorGetInfoEXT, DescriptorImageInfo, DescriptorType, DeviceMemory,
        DeviceSize, Extent3D, ExternalMemoryHandleTypeFlags, ExternalMemoryImageCreateInfo,
        FormatFeatureFlags, Image, ImageAspectFlags, ImageCreateFlags, ImageCreateInfo,
        ImageDrmFormatModifierExplicitCreateInfoEXT, ImageLayout, ImageMemoryRequirementsInfo2,
        ImagePlaneMemoryRequirementsInfo, ImageSubresourceRange, ImageTiling, ImageType,
        ImageUsageFlags, ImageView, ImageViewCreateInfo, ImageViewType, ImportMemoryFdInfoKHR,
        MemoryAllocateInfo, MemoryDedicatedAllocateInfo, MemoryFdPropertiesKHR,
        MemoryPropertyFlags, MemoryRequirements2, SampleCountFlags, SharingMode, SubresourceLayout,
    },
    gpu_alloc::UsageFlags,
    std::{
        any::Any,
        cell::Cell,
        fmt::{Debug, Formatter},
        mem,
        rc::Rc,
    },
};

pub struct VulkanDmaBufImageTemplate {
    pub(super) renderer: Rc<VulkanRenderer>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) disjoint: bool,
    pub(super) dmabuf: DmaBuf,
    pub(super) render_limits: Option<VulkanModifierLimits>,
    pub(super) texture_limits: Option<VulkanModifierLimits>,
    pub(super) render_needs_bridge: bool,
}

pub struct VulkanImage {
    pub(super) renderer: Rc<VulkanRenderer>,
    pub(super) format: &'static Format,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) stride: u32,
    pub(super) texture_view: ImageView,
    pub(super) render_view: Option<ImageView>,
    pub(super) image: Image,
    pub(super) is_undefined: Cell<bool>,
    pub(super) contents_are_undefined: Cell<bool>,
    pub(super) queue_state: Cell<QueueState>,
    pub(super) ty: VulkanImageMemory,
    pub(super) bridge: Option<VulkanFramebufferBridge>,
    pub(super) shader_read_only_optimal_descriptor: Box<[u8]>,
    pub(super) descriptor_buffer_version: Cell<u64>,
    pub(super) descriptor_buffer_offset: Cell<DeviceSize>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum QueueState {
    Acquired { family: QueueFamily },
    Releasing,
    Released { to: QueueFamily },
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum QueueFamily {
    Gfx,
    Transfer,
}

impl QueueState {
    pub fn acquire(self, new: QueueFamily) -> QueueTransfer {
        match self {
            QueueState::Acquired { family } if family == new => QueueTransfer::Unnecessary,
            QueueState::Released { to } if to == new => QueueTransfer::Possible,
            _ => QueueTransfer::Impossible,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum QueueTransfer {
    Unnecessary,
    Possible,
    Impossible,
}

pub enum VulkanImageMemory {
    DmaBuf(VulkanDmaBufImage),
    Internal(VulkanShmImage),
}

pub struct VulkanDmaBufImage {
    pub(super) template: Rc<VulkanDmaBufImageTemplate>,
    pub(super) mems: PlaneVec<DeviceMemory>,
}

pub struct VulkanFramebufferBridge {
    pub(super) dmabuf_image: Image,
    pub(super) _allocation: VulkanAllocation,
}

impl Drop for VulkanDmaBufImage {
    fn drop(&mut self) {
        unsafe {
            for &mem in &self.mems {
                self.template.renderer.device.device.free_memory(mem, None);
            }
        }
    }
}

impl Drop for VulkanImage {
    fn drop(&mut self) {
        unsafe {
            self.renderer
                .device
                .device
                .destroy_image_view(self.texture_view, None);
            if let Some(render_view) = self.render_view {
                self.renderer
                    .device
                    .device
                    .destroy_image_view(render_view, None);
            }
            self.renderer.device.device.destroy_image(self.image, None);
            if let Some(bridge) = &self.bridge {
                self.renderer
                    .device
                    .device
                    .destroy_image(bridge.dmabuf_image, None);
            }
        }
    }
}

impl VulkanRenderer {
    pub fn import_dmabuf(
        self: &Rc<Self>,
        dmabuf: &DmaBuf,
    ) -> Result<Rc<VulkanDmaBufImageTemplate>, VulkanError> {
        let format = self
            .device
            .formats
            .get(&dmabuf.format.drm)
            .ok_or(VulkanError::FormatNotSupported)?;
        let modifier = format
            .modifiers
            .get(&dmabuf.modifier)
            .ok_or(VulkanError::ModifierNotSupported)?;
        if dmabuf.width <= 0 || dmabuf.height <= 0 {
            return Err(VulkanError::NonPositiveImageSize);
        }
        let width = dmabuf.width as u32;
        let height = dmabuf.height as u32;
        let can_render = match &modifier.render_limits {
            None => false,
            Some(t) => width <= t.max_width && height <= t.max_height,
        };
        let can_texture = match &modifier.texture_limits {
            None => false,
            Some(t) => width <= t.max_width && height <= t.max_height,
        };
        if !can_render && !can_texture {
            if modifier.render_limits.is_none() && modifier.texture_limits.is_none() {
                return Err(VulkanError::ModifierUseNotSupported);
            }
            return Err(VulkanError::ImageTooLarge);
        }
        if modifier.planes != dmabuf.planes.len() {
            return Err(VulkanError::BadPlaneCount);
        }
        let disjoint = dmabuf.is_disjoint();
        if disjoint && !modifier.features.contains(FormatFeatureFlags::DISJOINT) {
            return Err(VulkanError::DisjointNotSupported);
        }
        Ok(Rc::new(VulkanDmaBufImageTemplate {
            renderer: self.clone(),
            width,
            height,
            disjoint,
            dmabuf: dmabuf.clone(),
            render_limits: modifier.render_limits,
            texture_limits: modifier.texture_limits,
            render_needs_bridge: modifier.render_needs_bridge,
        }))
    }
}

impl VulkanDevice {
    pub fn create_image_view(
        &self,
        image: Image,
        format: &'static Format,
        for_rendering: bool,
    ) -> Result<ImageView, VulkanError> {
        let create_info = ImageViewCreateInfo::default()
            .image(image)
            .view_type(ImageViewType::TYPE_2D)
            .format(format.vk_format)
            .components(ComponentMapping {
                r: ComponentSwizzle::IDENTITY,
                g: ComponentSwizzle::IDENTITY,
                b: ComponentSwizzle::IDENTITY,
                a: match format.has_alpha || for_rendering {
                    true => ComponentSwizzle::IDENTITY,
                    false => ComponentSwizzle::ONE,
                },
            })
            .subresource_range(ImageSubresourceRange {
                aspect_mask: ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let view = unsafe { self.device.create_image_view(&create_info, None) };
        view.map_err(VulkanError::CreateImageView)
    }
}

impl VulkanRenderer {
    pub(super) fn sampler_read_only_descriptor(&self, view: ImageView) -> Box<[u8]> {
        let Some(db) = &self.device.descriptor_buffer else {
            return Box::new([]);
        };
        let mut buf =
            vec![0; self.device.combined_image_sampler_descriptor_size].into_boxed_slice();
        let image_info = DescriptorImageInfo::default()
            .sampler(self.sampler.sampler)
            .image_view(view)
            .image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        let info = DescriptorGetInfoEXT::default()
            .ty(DescriptorType::COMBINED_IMAGE_SAMPLER)
            .data(DescriptorDataEXT {
                p_combined_image_sampler: &image_info,
            });
        unsafe {
            db.get_descriptor(&info, &mut buf);
        }
        buf
    }
}

impl VulkanDmaBufImageTemplate {
    pub fn create_framebuffer(self: &Rc<Self>) -> Result<Rc<VulkanImage>, VulkanError> {
        self.create_image(true)
    }

    pub fn create_texture(self: &Rc<Self>) -> Result<Rc<VulkanImage>, VulkanError> {
        self.create_image(false)
    }

    fn create_image(self: &Rc<Self>, for_rendering: bool) -> Result<Rc<VulkanImage>, VulkanError> {
        let device = &self.renderer.device;
        let limits = match for_rendering {
            true => self.render_limits,
            false => self.texture_limits,
        };
        let limits = limits.ok_or(VulkanError::ModifierUseNotSupported)?;
        if self.width > limits.max_width || self.height > limits.max_height {
            return Err(VulkanError::ImageTooLarge);
        }
        let image = {
            let plane_layouts: PlaneVec<_> = self
                .dmabuf
                .planes
                .iter()
                .map(|p| SubresourceLayout {
                    offset: p.offset as _,
                    row_pitch: p.stride as _,
                    size: 0,
                    array_pitch: 0,
                    depth_pitch: 0,
                })
                .collect();
            let mut mod_info = ImageDrmFormatModifierExplicitCreateInfoEXT::default()
                .drm_format_modifier(self.dmabuf.modifier)
                .plane_layouts(&plane_layouts);
            let mut memory_image_create_info = ExternalMemoryImageCreateInfo::default()
                .handle_types(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let flags = match self.disjoint {
                true => ImageCreateFlags::DISJOINT,
                false => ImageCreateFlags::empty(),
            };
            let usage = match for_rendering {
                true => match self.render_needs_bridge {
                    true => ImageUsageFlags::TRANSFER_DST,
                    false => ImageUsageFlags::TRANSFER_SRC | ImageUsageFlags::COLOR_ATTACHMENT,
                },
                false => ImageUsageFlags::TRANSFER_SRC | ImageUsageFlags::SAMPLED,
            };
            let create_info = ImageCreateInfo::default()
                .image_type(ImageType::TYPE_2D)
                .format(self.dmabuf.format.vk_format)
                .mip_levels(1)
                .array_layers(1)
                .tiling(ImageTiling::DRM_FORMAT_MODIFIER_EXT)
                .samples(SampleCountFlags::TYPE_1)
                .sharing_mode(SharingMode::EXCLUSIVE)
                .initial_layout(ImageLayout::UNDEFINED)
                .extent(Extent3D {
                    width: self.width,
                    height: self.height,
                    depth: 1,
                })
                .usage(usage)
                .flags(flags)
                .push_next(&mut memory_image_create_info)
                .push_next(&mut mod_info);
            let image = unsafe { device.device.create_image(&create_info, None) };
            image.map_err(VulkanError::CreateImage)?
        };
        let destroy_image = OnDrop(|| unsafe { device.device.destroy_image(image, None) });
        let num_device_memories = match self.disjoint {
            true => self.dmabuf.planes.len(),
            false => 1,
        };
        let mut device_memories = PlaneVec::new();
        let mut free_device_memories = PlaneVec::new();
        let mut bind_image_plane_memory_infos = PlaneVec::new();
        for plane_idx in 0..num_device_memories {
            let dma_buf_plane = &self.dmabuf.planes[plane_idx];
            let mut memory_fd_properties = MemoryFdPropertiesKHR::default();
            unsafe {
                device
                    .external_memory_fd
                    .get_memory_fd_properties(
                        ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
                        dma_buf_plane.fd.raw(),
                        &mut memory_fd_properties,
                    )
                    .map_err(VulkanError::MemoryFdProperties)?;
            }
            let mut image_memory_requirements_info =
                ImageMemoryRequirementsInfo2::default().image(image);
            let mut image_plane_memory_requirements_info;
            if self.disjoint {
                let plane_aspect = match plane_idx {
                    0 => ImageAspectFlags::MEMORY_PLANE_0_EXT,
                    1 => ImageAspectFlags::MEMORY_PLANE_1_EXT,
                    2 => ImageAspectFlags::MEMORY_PLANE_2_EXT,
                    3 => ImageAspectFlags::MEMORY_PLANE_3_EXT,
                    _ => unreachable!(),
                };
                image_plane_memory_requirements_info =
                    ImagePlaneMemoryRequirementsInfo::default().plane_aspect(plane_aspect);
                image_memory_requirements_info = image_memory_requirements_info
                    .push_next(&mut image_plane_memory_requirements_info);
                bind_image_plane_memory_infos
                    .push(BindImagePlaneMemoryInfo::default().plane_aspect(plane_aspect));
            }
            let mut memory_requirements = MemoryRequirements2::default();
            unsafe {
                device.device.get_image_memory_requirements2(
                    &image_memory_requirements_info,
                    &mut memory_requirements,
                );
            }
            let memory_type_bits = memory_requirements.memory_requirements.memory_type_bits
                & memory_fd_properties.memory_type_bits;
            let memory_type_index = self
                .renderer
                .device
                .find_memory_type(MemoryPropertyFlags::empty(), memory_type_bits)
                .ok_or(VulkanError::MemoryType)?;
            let fd = uapi::fcntl_dupfd_cloexec(dma_buf_plane.fd.raw(), 0)
                .map_err(|e| VulkanError::Dupfd(e.into()))?;
            let mut memory_dedicated_allocate_info =
                MemoryDedicatedAllocateInfo::default().image(image);
            let mut import_memory_fd_info = ImportMemoryFdInfoKHR::default()
                .fd(fd.raw())
                .handle_type(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let memory_allocate_info = MemoryAllocateInfo::default()
                .allocation_size(memory_requirements.memory_requirements.size)
                .memory_type_index(memory_type_index)
                .push_next(&mut import_memory_fd_info)
                .push_next(&mut memory_dedicated_allocate_info);
            let device_memory =
                unsafe { device.device.allocate_memory(&memory_allocate_info, None) };
            let device_memory = device_memory.map_err(VulkanError::AllocateMemory)?;
            fd.unwrap();
            device_memories.push(device_memory);
            free_device_memories.push(OnDrop(move || unsafe {
                device.device.free_memory(device_memory, None)
            }));
        }
        let mut bind_image_memory_infos = Vec::with_capacity(num_device_memories);
        let mut bind_image_plane_memory_infos = bind_image_plane_memory_infos.iter_mut();
        for mem in device_memories.iter().copied() {
            let mut info = BindImageMemoryInfo::default().image(image).memory(mem);
            if self.disjoint {
                info = info.push_next(bind_image_plane_memory_infos.next().unwrap());
            }
            bind_image_memory_infos.push(info);
        }
        let res = unsafe { device.device.bind_image_memory2(&bind_image_memory_infos) };
        res.map_err(VulkanError::BindImageMemory)?;
        let mut primary_image = image;
        let mut destroy_bridge_image = None;
        let mut bridge = None;
        if for_rendering && self.render_needs_bridge {
            let (bridge_image, allocation) = self.create_bridge()?;
            primary_image = bridge_image;
            destroy_bridge_image = Some(OnDrop(|| unsafe {
                device.device.destroy_image(primary_image, None)
            }));
            bridge = Some(VulkanFramebufferBridge {
                dmabuf_image: image,
                _allocation: allocation,
            });
        }
        let texture_view = device.create_image_view(primary_image, self.dmabuf.format, false)?;
        let render_view = device.create_image_view(primary_image, self.dmabuf.format, true)?;
        free_device_memories.drain(..).for_each(mem::forget);
        mem::forget(destroy_image);
        mem::forget(destroy_bridge_image);
        Ok(Rc::new(VulkanImage {
            renderer: self.renderer.clone(),
            texture_view,
            render_view: Some(render_view),
            image: primary_image,
            width: self.width,
            height: self.height,
            stride: 0,
            ty: VulkanImageMemory::DmaBuf(VulkanDmaBufImage {
                template: self.clone(),
                mems: device_memories,
            }),
            format: self.dmabuf.format,
            is_undefined: Cell::new(true),
            contents_are_undefined: Cell::new(false),
            queue_state: Cell::new(QueueState::Acquired {
                family: QueueFamily::Gfx,
            }),
            bridge,
            shader_read_only_optimal_descriptor: self
                .renderer
                .sampler_read_only_descriptor(texture_view),
            descriptor_buffer_version: Cell::new(0),
            descriptor_buffer_offset: Cell::new(0),
        }))
    }

    fn create_bridge(&self) -> Result<(Image, VulkanAllocation), VulkanError> {
        let create_info = ImageCreateInfo::default()
            .image_type(ImageType::TYPE_2D)
            .format(self.dmabuf.format.vk_format)
            .mip_levels(1)
            .array_layers(1)
            .tiling(ImageTiling::OPTIMAL)
            .samples(SampleCountFlags::TYPE_1)
            .sharing_mode(SharingMode::EXCLUSIVE)
            .initial_layout(ImageLayout::UNDEFINED)
            .extent(Extent3D {
                width: self.width,
                height: self.height,
                depth: 1,
            })
            .usage(ImageUsageFlags::COLOR_ATTACHMENT | ImageUsageFlags::TRANSFER_SRC);
        let image = unsafe { self.renderer.device.device.create_image(&create_info, None) };
        let image = image.map_err(VulkanError::CreateImage)?;
        let destroy_image =
            OnDrop(|| unsafe { self.renderer.device.device.destroy_image(image, None) });
        let memory_requirements = unsafe {
            self.renderer
                .device
                .device
                .get_image_memory_requirements(image)
        };
        let allocation = self.renderer.allocator.alloc(
            &memory_requirements,
            UsageFlags::FAST_DEVICE_ACCESS,
            false,
        )?;
        let res = unsafe {
            self.renderer.device.device.bind_image_memory(
                image,
                allocation.memory,
                allocation.offset,
            )
        };
        res.map_err(VulkanError::BindImageMemory)?;
        destroy_image.forget();
        Ok((image, allocation))
    }
}

impl GfxImage for VulkanDmaBufImageTemplate {
    fn to_framebuffer(self: Rc<Self>) -> Result<Rc<dyn GfxFramebuffer>, GfxError> {
        self.create_framebuffer()
            .map(|v| v as _)
            .map_err(|e| e.into())
    }

    fn to_texture(self: Rc<Self>) -> Result<Rc<dyn GfxTexture>, GfxError> {
        self.create_texture().map(|v| v as _).map_err(|e| e.into())
    }

    fn width(&self) -> i32 {
        self.width as i32
    }

    fn height(&self) -> i32 {
        self.height as i32
    }
}

impl Debug for VulkanImage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanDmaBufImage").finish_non_exhaustive()
    }
}

impl GfxFramebuffer for VulkanImage {
    fn physical_size(&self) -> (i32, i32) {
        (self.width as _, self.height as _)
    }

    fn render_with_region(
        &self,
        acquire_sync: AcquireSync,
        release_sync: ReleaseSync,
        ops: &[GfxApiOpt],
        clear: Option<&Color>,
        region: &Region,
    ) -> Result<Option<SyncFile>, GfxError> {
        self.renderer
            .execute(self, acquire_sync, release_sync, ops, clear, region)
            .map_err(|e| e.into())
    }

    fn format(&self) -> &'static Format {
        self.format
    }
}

impl GfxInternalFramebuffer for VulkanImage {
    fn into_fb(self: Rc<Self>) -> Rc<dyn GfxFramebuffer> {
        self
    }

    fn stride(&self) -> i32 {
        let VulkanImageMemory::Internal(shm) = &self.ty else {
            unreachable!();
        };
        shm.stride as _
    }

    fn staging_size(&self) -> usize {
        let VulkanImageMemory::Internal(shm) = &self.ty else {
            unreachable!();
        };
        shm.size as _
    }

    fn download(
        self: Rc<Self>,
        staging: &Rc<dyn GfxStagingBuffer>,
        callback: Rc<dyn AsyncShmGfxTextureCallback>,
        mem: Rc<dyn ShmMemory>,
        damage: Region,
    ) -> Result<Option<PendingShmTransfer>, GfxError> {
        let VulkanImageMemory::Internal(shm) = &self.ty else {
            unreachable!();
        };
        let staging = staging.clone().into_vk(&self.renderer.device.device);
        let pending = shm.async_transfer(
            &self,
            staging,
            &mem,
            damage,
            callback,
            TransferType::Download,
        )?;
        Ok(pending)
    }
}

impl GfxTexture for VulkanImage {
    fn size(&self) -> (i32, i32) {
        (self.width as _, self.height as _)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn dmabuf(&self) -> Option<&DmaBuf> {
        match &self.ty {
            VulkanImageMemory::DmaBuf(b) => Some(&b.template.dmabuf),
            VulkanImageMemory::Internal(_) => None,
        }
    }

    fn format(&self) -> &'static Format {
        self.format
    }
}

impl ShmGfxTexture for VulkanImage {
    fn into_texture(self: Rc<Self>) -> Rc<dyn GfxTexture> {
        self
    }
}

impl AsyncShmGfxTexture for VulkanImage {
    fn staging_size(&self) -> usize {
        let VulkanImageMemory::Internal(shm) = &self.ty else {
            unreachable!();
        };
        shm.size as _
    }

    fn async_upload(
        self: Rc<Self>,
        staging: &Rc<dyn GfxStagingBuffer>,
        callback: Rc<dyn AsyncShmGfxTextureCallback>,
        mem: Rc<dyn ShmMemory>,
        damage: Region,
    ) -> Result<Option<PendingShmTransfer>, GfxError> {
        let VulkanImageMemory::Internal(shm) = &self.ty else {
            unreachable!();
        };
        let staging = staging.clone().into_vk(&self.renderer.device.device);
        let pending =
            shm.async_transfer(&self, staging, &mem, damage, callback, TransferType::Upload)?;
        Ok(pending)
    }

    fn sync_upload(self: Rc<Self>, mem: &[Cell<u8>], damage: Region) -> Result<(), GfxError> {
        let VulkanImageMemory::Internal(shm) = &self.ty else {
            unreachable!();
        };
        if shm.async_data.as_ref().unwrap().busy.get() {
            return Err(VulkanError::AsyncCopyBusy.into());
        }
        shm.upload(&self, mem, Some(damage.rects()))?;
        Ok(())
    }

    fn compatible_with(
        &self,
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
    ) -> bool {
        self.format == format
            && self.width == width as u32
            && self.height == height as u32
            && self.stride == stride as u32
    }

    fn into_texture(self: Rc<Self>) -> Rc<dyn GfxTexture> {
        self
    }
}

impl AsyncShmGfxTextureTransferCancellable for VulkanImage {
    fn cancel(&self, id: u64) {
        let VulkanImageMemory::Internal(shm) = &self.ty else {
            unreachable!();
        };
        let data = shm.async_data.as_ref().unwrap();
        if data.callback_id.get() == id {
            data.callback.take();
        }
    }
}
