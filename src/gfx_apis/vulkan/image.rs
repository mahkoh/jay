use {
    crate::{
        format::Format,
        gfx_api::{GfxApiOpt, GfxError, GfxFramebuffer, GfxImage, GfxTexture, SyncFile},
        gfx_apis::vulkan::{
            allocator::VulkanAllocation, device::VulkanDevice, format::VulkanMaxExtents,
            renderer::VulkanRenderer, util::OnDrop, VulkanError,
        },
        theme::Color,
        utils::clonecell::CloneCell,
        video::dmabuf::{DmaBuf, PlaneVec},
    },
    ash::vk::{
        BindImageMemoryInfo, BindImagePlaneMemoryInfo, ComponentMapping, ComponentSwizzle,
        DeviceMemory, DeviceSize, Extent3D, ExternalMemoryHandleTypeFlags,
        ExternalMemoryImageCreateInfo, FormatFeatureFlags, Image, ImageAspectFlags,
        ImageCreateFlags, ImageCreateInfo, ImageDrmFormatModifierExplicitCreateInfoEXT,
        ImageLayout, ImageMemoryRequirementsInfo2, ImagePlaneMemoryRequirementsInfo,
        ImageSubresourceRange, ImageTiling, ImageType, ImageUsageFlags, ImageView,
        ImageViewCreateInfo, ImageViewType, ImportMemoryFdInfoKHR, MemoryAllocateInfo,
        MemoryDedicatedAllocateInfo, MemoryPropertyFlags, MemoryRequirements2, SampleCountFlags,
        SharingMode, SubresourceLayout,
    },
    gpu_alloc::UsageFlags,
    std::{
        any::Any,
        cell::{Cell, RefCell},
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
    pub(super) render_max_extents: Option<VulkanMaxExtents>,
    pub(super) texture_max_extents: Option<VulkanMaxExtents>,
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
    pub(super) ty: VulkanImageMemory,
    pub(super) render_ops: CloneCell<Vec<GfxApiOpt>>,
}

pub enum VulkanImageMemory {
    DmaBuf(VulkanDmaBufImage),
    Internal(VulkanShmImage),
}

pub struct VulkanDmaBufImage {
    pub(super) template: Rc<VulkanDmaBufImageTemplate>,
    pub(super) mems: PlaneVec<DeviceMemory>,
}

pub struct VulkanShmImage {
    pub(super) to_flush: RefCell<Option<Vec<u8>>>,
    pub(super) size: DeviceSize,
    pub(super) stride: u32,
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
        }
    }
}

impl VulkanShmImage {
    pub fn upload(&self, buffer: &[Cell<u8>]) -> Result<(), VulkanError> {
        let buffer = unsafe {
            std::slice::from_raw_parts(buffer.as_ptr() as *const u8, buffer.len()).to_vec()
        };
        *self.to_flush.borrow_mut() = Some(buffer);
        Ok(())
    }
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
    ) -> Result<Rc<VulkanImage>, VulkanError> {
        if width <= 0 || height <= 0 || stride <= 0 {
            return Err(VulkanError::NonPositiveImageSize);
        }
        let width = width as u32;
        let height = height as u32;
        let stride = stride as u32;
        if stride % format.bpp != 0 || stride / format.bpp < width {
            return Err(VulkanError::InvalidStride);
        }
        let vk_format = self
            .device
            .formats
            .get(&format.drm)
            .ok_or(VulkanError::FormatNotSupported)?;
        let shm = vk_format.shm.as_ref().ok_or(VulkanError::ShmNotSupported)?;
        if width > shm.max_extents.width || height > shm.max_extents.height {
            return Err(VulkanError::ImageTooLarge);
        }
        let size = stride.checked_mul(height).ok_or(VulkanError::ShmOverflow)?;
        let usage = ImageUsageFlags::TRANSFER_SRC
            | match for_download {
                true => ImageUsageFlags::COLOR_ATTACHMENT,
                false => ImageUsageFlags::SAMPLED | ImageUsageFlags::TRANSFER_DST,
            };
        let create_info = ImageCreateInfo::builder()
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
            .usage(usage)
            .build();
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
        let image_view_create_info = ImageViewCreateInfo::builder()
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
        let shm = VulkanShmImage {
            to_flush: Default::default(),
            size: size as u64,
            stride,
            _allocation: allocation,
        };
        shm.upload(data)?;
        destroy_image.forget();
        Ok(Rc::new(VulkanImage {
            renderer: self.clone(),
            format,
            width,
            height,
            stride,
            texture_view: view,
            render_view: None,
            image,
            is_undefined: Cell::new(true),
            ty: VulkanImageMemory::Internal(shm),
            render_ops: Default::default(),
        }))
    }

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
        let can_render = match &modifier.render_max_extents {
            None => false,
            Some(t) => width <= t.width && height <= t.height,
        };
        let can_texture = match &modifier.texture_max_extents {
            None => false,
            Some(t) => width <= t.width && height <= t.height,
        };
        if !can_render && !can_texture {
            if modifier.render_max_extents.is_none() && modifier.texture_max_extents.is_none() {
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
            render_max_extents: modifier.render_max_extents,
            texture_max_extents: modifier.texture_max_extents,
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
        let create_info = ImageViewCreateInfo::builder()
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

impl VulkanDmaBufImageTemplate {
    pub fn create_framebuffer(self: &Rc<Self>) -> Result<Rc<VulkanImage>, VulkanError> {
        self.create_image(true, None)
    }

    pub fn create_texture(
        self: &Rc<Self>,
        shm: Option<VulkanShmImage>,
    ) -> Result<Rc<VulkanImage>, VulkanError> {
        self.create_image(false, shm)
    }

    fn create_image(
        self: &Rc<Self>,
        for_rendering: bool,
        shm: Option<VulkanShmImage>,
    ) -> Result<Rc<VulkanImage>, VulkanError> {
        let device = &self.renderer.device;
        let max_extents = match for_rendering {
            true => self.render_max_extents,
            false => self.texture_max_extents,
        };
        let max_extents = max_extents.ok_or(VulkanError::ModifierUseNotSupported)?;
        if self.width > max_extents.width || self.height > max_extents.height {
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
            let mut mod_info = ImageDrmFormatModifierExplicitCreateInfoEXT::builder()
                .drm_format_modifier(self.dmabuf.modifier)
                .plane_layouts(&plane_layouts)
                .build();
            let mut memory_image_create_info = ExternalMemoryImageCreateInfo::builder()
                .handle_types(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
                .build();
            let flags = match self.disjoint {
                true => ImageCreateFlags::DISJOINT,
                false => ImageCreateFlags::empty(),
            };
            let usage = ImageUsageFlags::TRANSFER_SRC
                | match (for_rendering, shm.is_some()) {
                    (true, _) => ImageUsageFlags::COLOR_ATTACHMENT,
                    (false, false) => ImageUsageFlags::SAMPLED,
                    (false, true) => ImageUsageFlags::SAMPLED | ImageUsageFlags::TRANSFER_DST,
                };
            let create_info = ImageCreateInfo::builder()
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
                .push_next(&mut mod_info)
                .build();
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
            let memory_fd_properties = unsafe {
                device.external_memory_fd.get_memory_fd_properties(
                    ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
                    dma_buf_plane.fd.raw(),
                )
            };
            let memory_fd_properties =
                memory_fd_properties.map_err(VulkanError::MemoryFdProperties)?;
            let mut image_memory_requirements_info =
                ImageMemoryRequirementsInfo2::builder().image(image);
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
                    ImagePlaneMemoryRequirementsInfo::builder().plane_aspect(plane_aspect);
                image_memory_requirements_info = image_memory_requirements_info
                    .push_next(&mut image_plane_memory_requirements_info);
                bind_image_plane_memory_infos.push(
                    BindImagePlaneMemoryInfo::builder()
                        .plane_aspect(plane_aspect)
                        .build(),
                );
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
                MemoryDedicatedAllocateInfo::builder().image(image).build();
            let mut import_memory_fd_info = ImportMemoryFdInfoKHR::builder()
                .fd(fd.raw())
                .handle_type(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
                .build();
            let memory_allocate_info = MemoryAllocateInfo::builder()
                .allocation_size(memory_requirements.memory_requirements.size)
                .memory_type_index(memory_type_index)
                .push_next(&mut import_memory_fd_info)
                .push_next(&mut memory_dedicated_allocate_info)
                .build();
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
        for (i, mem) in device_memories.iter().copied().enumerate() {
            let mut info = BindImageMemoryInfo::builder().image(image).memory(mem);
            if self.disjoint {
                info = info.push_next(&mut bind_image_plane_memory_infos[i]);
            }
            bind_image_memory_infos.push(info.build());
        }
        let res = unsafe { device.device.bind_image_memory2(&bind_image_memory_infos) };
        res.map_err(VulkanError::BindImageMemory)?;
        let texture_view = device.create_image_view(image, self.dmabuf.format, false)?;
        let render_view = device.create_image_view(image, self.dmabuf.format, true)?;
        free_device_memories.drain(..).for_each(mem::forget);
        mem::forget(destroy_image);
        Ok(Rc::new(VulkanImage {
            renderer: self.renderer.clone(),
            texture_view,
            render_view: Some(render_view),
            image,
            width: self.width,
            height: self.height,
            stride: 0,
            render_ops: Default::default(),
            ty: VulkanImageMemory::DmaBuf(VulkanDmaBufImage {
                template: self.clone(),
                mems: device_memories,
            }),
            format: self.dmabuf.format,
            is_undefined: Cell::new(true),
        }))
    }
}

impl GfxImage for VulkanDmaBufImageTemplate {
    fn to_framebuffer(self: Rc<Self>) -> Result<Rc<dyn GfxFramebuffer>, GfxError> {
        self.create_framebuffer()
            .map(|v| v as _)
            .map_err(|e| e.into())
    }

    fn to_texture(self: Rc<Self>) -> Result<Rc<dyn GfxTexture>, GfxError> {
        self.create_texture(None)
            .map(|v| v as _)
            .map_err(|e| e.into())
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
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn take_render_ops(&self) -> Vec<GfxApiOpt> {
        self.render_ops.take()
    }

    fn physical_size(&self) -> (i32, i32) {
        (self.width as _, self.height as _)
    }

    fn render(
        &self,
        ops: Vec<GfxApiOpt>,
        clear: Option<&Color>,
    ) -> Result<Option<SyncFile>, GfxError> {
        self.renderer
            .execute(self, &ops, clear)
            .map_err(|e| e.into())
    }

    fn copy_to_shm(
        self: Rc<Self>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
        shm: &[Cell<u8>],
    ) -> Result<(), GfxError> {
        self.renderer
            .read_pixels(&self, x, y, width, height, stride, format, shm)
            .map_err(|e| e.into())
    }

    fn format(&self) -> &'static Format {
        self.format
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

    fn read_pixels(
        self: Rc<Self>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
        shm: &[Cell<u8>],
    ) -> Result<(), GfxError> {
        self.renderer
            .read_pixels(&self, x, y, width, height, stride, format, shm)
            .map_err(|e| e.into())
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
