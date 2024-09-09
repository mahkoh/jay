use {
    crate::{
        allocator::{
            Allocator, AllocatorError, BufferObject, BufferUsage, MappedBuffer, BO_USE_RENDERING,
            BO_USE_WRITE,
        },
        format::Format,
        gfx_apis::vulkan::{
            allocator::VulkanAllocator, command::VulkanCommandBuffer, device::VulkanDevice,
            format::VulkanFormat, renderer::image_barrier, staging::VulkanStagingBuffer,
            VulkanError,
        },
        utils::{errorfmt::ErrorFmt, on_drop::OnDrop},
        video::{
            dmabuf::{DmaBuf, DmaBufIds, DmaBufPlane, PlaneVec},
            drm::Drm,
            Modifier,
        },
    },
    arrayvec::ArrayVec,
    ash::vk::{
        AccessFlags2, BindImageMemoryInfo, BindImagePlaneMemoryInfo, BufferImageCopy2,
        BufferMemoryBarrier2, CommandBuffer, CommandBufferBeginInfo, CommandBufferSubmitInfo,
        CommandBufferUsageFlags, CopyBufferToImageInfo2, CopyImageToBufferInfo2, DependencyInfo,
        DeviceMemory, ExportMemoryAllocateInfo, Extent3D, ExternalMemoryHandleTypeFlags,
        ExternalMemoryImageCreateInfo, Fence, FormatFeatureFlags, Image, ImageAspectFlags,
        ImageCreateInfo, ImageDrmFormatModifierExplicitCreateInfoEXT,
        ImageDrmFormatModifierListCreateInfoEXT, ImageDrmFormatModifierPropertiesEXT, ImageLayout,
        ImageMemoryBarrier2, ImageMemoryRequirementsInfo2, ImagePlaneMemoryRequirementsInfo,
        ImageSubresource, ImageSubresourceLayers, ImageTiling, ImageType, ImageUsageFlags,
        ImportMemoryFdInfoKHR, MemoryAllocateInfo, MemoryDedicatedAllocateInfo,
        MemoryFdPropertiesKHR, MemoryGetFdInfoKHR, MemoryPropertyFlags, MemoryRequirements2,
        PipelineStageFlags2, SampleCountFlags, SharingMode, SubmitInfo2, SubresourceLayout,
        QUEUE_FAMILY_FOREIGN_EXT,
    },
    std::{rc::Rc, slice},
    uapi::OwnedFd,
};

impl From<VulkanError> for AllocatorError {
    fn from(value: VulkanError) -> Self {
        Self(Box::new(value))
    }
}

pub(super) struct VulkanBoAllocator {
    data: Rc<VulkanBoAllocatorData>,
}

struct VulkanBoAllocatorData {
    drm: Drm,
    device: Rc<VulkanDevice>,
    allocator: Rc<VulkanAllocator>,
    command_buffer: Rc<VulkanCommandBuffer>,
}

struct VulkanBo {
    allocator: Rc<VulkanBoAllocatorData>,
    image: Image,
    memory: PlaneVec<DeviceMemory>,
    buf: DmaBuf,
}

struct VulkanBoMapping {
    bo: Rc<VulkanBo>,
    upload: bool,
    stride: i32,
    staging: VulkanStagingBuffer,
    data: *mut [u8],
}

impl Drop for VulkanBo {
    fn drop(&mut self) {
        unsafe {
            self.allocator.device.device.destroy_image(self.image, None);
            for &memory in &self.memory {
                self.allocator.device.device.free_memory(memory, None);
            }
        }
    }
}

impl VulkanDevice {
    pub(super) fn create_bo_allocator(
        self: &Rc<Self>,
        drm: &Drm,
    ) -> Result<VulkanBoAllocator, VulkanError> {
        let allocator = self.create_allocator()?;
        let pool = self.create_command_pool()?;
        let command_buffer = pool.allocate_buffer()?;
        let drm = drm.dup_render().map_err(VulkanError::DupDrm)?;
        Ok(VulkanBoAllocator {
            data: Rc::new(VulkanBoAllocatorData {
                drm,
                device: self.clone(),
                allocator,
                command_buffer,
            }),
        })
    }
}

impl VulkanBoAllocator {
    fn create_bo(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
        modifiers: &[Modifier],
        usage: BufferUsage,
    ) -> Result<Rc<VulkanBo>, VulkanError> {
        validate_usage(usage)?;
        let data = &self.data;
        let Some(format) = data.device.formats.get(&format.drm) else {
            return Err(VulkanError::FormatNotSupported);
        };
        if width < 0 || height < 0 {
            return Err(VulkanError::NonPositiveImageSize);
        }
        let width = width as u32;
        let height = height as u32;
        let image = {
            let mut mods = vec![];
            for &modifier in modifiers {
                if validate_modifier(width, height, usage, false, None, format, modifier) {
                    mods.push(modifier);
                }
            }
            if mods.is_empty() {
                return Err(VulkanError::NoSupportedModifiers);
            }
            let mut mod_list =
                ImageDrmFormatModifierListCreateInfoEXT::default().drm_format_modifiers(&mods);
            let mut memory_image_create_info = ExternalMemoryImageCreateInfo::default()
                .handle_types(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let create_info = image_create_info(width, height, format.format, usage)
                .push_next(&mut memory_image_create_info)
                .push_next(&mut mod_list);
            let res = unsafe { data.device.device.create_image(&create_info, None) };
            res.map_err(VulkanError::CreateImage)?
        };
        let destroy_image = OnDrop(|| unsafe { data.device.device.destroy_image(image, None) });
        let modifier = {
            let mut props = ImageDrmFormatModifierPropertiesEXT::default();
            unsafe {
                data.device
                    .image_drm_format_modifier
                    .get_image_drm_format_modifier_properties(image, &mut props)
                    .map_err(VulkanError::GetModifier)?
            }
            props.drm_format_modifier
        };
        let Some(modifier) = format.modifiers.get(&modifier) else {
            return Err(VulkanError::InvalidModifier)?;
        };
        let memory = {
            let image_memory_requirements_info =
                ImageMemoryRequirementsInfo2::default().image(image);
            let mut memory_requirements = MemoryRequirements2::default();
            unsafe {
                data.device.device.get_image_memory_requirements2(
                    &image_memory_requirements_info,
                    &mut memory_requirements,
                );
            }
            let memory_type_index = data
                .device
                .find_memory_type(
                    MemoryPropertyFlags::DEVICE_LOCAL,
                    memory_requirements.memory_requirements.memory_type_bits,
                )
                .ok_or(VulkanError::MemoryType)?;
            let mut memory_dedicated_allocate_info =
                MemoryDedicatedAllocateInfo::default().image(image);
            let mut export_info = ExportMemoryAllocateInfo::default()
                .handle_types(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let memory_allocate_info = MemoryAllocateInfo::default()
                .allocation_size(memory_requirements.memory_requirements.size)
                .memory_type_index(memory_type_index)
                .push_next(&mut memory_dedicated_allocate_info)
                .push_next(&mut export_info);
            let memory = unsafe {
                data.device
                    .device
                    .allocate_memory(&memory_allocate_info, None)
            };
            memory.map_err(VulkanError::AllocateMemory)?
        };
        let destroy_memory = OnDrop(|| unsafe { data.device.device.free_memory(memory, None) });
        unsafe {
            data.device
                .device
                .bind_image_memory(image, memory, 0)
                .map_err(VulkanError::BindImageMemory)?;
        }
        let fd = {
            let get_info = MemoryGetFdInfoKHR::default()
                .handle_type(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
                .memory(memory);
            let fd = unsafe { data.device.external_memory_fd.get_memory_fd(&get_info) };
            fd.map_err(VulkanError::GetDmaBuf)
                .map(OwnedFd::new)
                .map(Rc::new)?
        };
        let mut planes = PlaneVec::new();
        for i in 0..modifier.planes {
            let flag = [
                ImageAspectFlags::MEMORY_PLANE_0_EXT,
                ImageAspectFlags::MEMORY_PLANE_1_EXT,
                ImageAspectFlags::MEMORY_PLANE_2_EXT,
                ImageAspectFlags::MEMORY_PLANE_3_EXT,
            ][i];
            let layout = unsafe {
                data.device.device.get_image_subresource_layout(
                    image,
                    ImageSubresource::default().aspect_mask(flag),
                )
            };
            planes.push(DmaBufPlane {
                offset: layout.offset as _,
                stride: layout.row_pitch as _,
                fd: fd.clone(),
            });
        }
        let buf = DmaBuf {
            id: dma_buf_ids.next(),
            width: width as _,
            height: height as _,
            format: format.format,
            modifier: modifier.modifier,
            planes,
        };
        unsafe {
            let cmd = data.command_buffer.buffer;
            let device = &data.device.device;
            let begin =
                CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            let barrier = image_barrier()
                .src_queue_family_index(data.device.graphics_queue_idx)
                .dst_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                .old_layout(ImageLayout::UNDEFINED)
                .new_layout(ImageLayout::GENERAL)
                .image(image);
            let dependency_info =
                DependencyInfo::default().image_memory_barriers(slice::from_ref(&barrier));
            let cmd_buffer_submit_info = CommandBufferSubmitInfo::default().command_buffer(cmd);
            let submit_info = SubmitInfo2::default()
                .command_buffer_infos(slice::from_ref(&cmd_buffer_submit_info));
            device
                .begin_command_buffer(cmd, &begin)
                .map_err(VulkanError::BeginCommandBuffer)?;
            device.cmd_pipeline_barrier2(cmd, &dependency_info);
            device
                .end_command_buffer(cmd)
                .map_err(VulkanError::EndCommandBuffer)?;
            device
                .queue_submit2(
                    data.device.graphics_queue,
                    slice::from_ref(&submit_info),
                    Fence::null(),
                )
                .map_err(VulkanError::Submit)?;
            device.device_wait_idle().map_err(VulkanError::WaitIdle)?;
        }
        destroy_image.forget();
        destroy_memory.forget();
        Ok(Rc::new(VulkanBo {
            allocator: self.data.clone(),
            image,
            memory: [memory].into_iter().collect(),
            buf,
        }))
    }

    fn import_dmabuf(
        &self,
        dmabuf: &DmaBuf,
        usage: BufferUsage,
    ) -> Result<Rc<VulkanBo>, VulkanError> {
        validate_usage(usage)?;
        let data = &self.data;
        let Some(format) = data.device.formats.get(&dmabuf.format.drm) else {
            return Err(VulkanError::FormatNotSupported);
        };
        if dmabuf.width < 0 || dmabuf.height < 0 {
            return Err(VulkanError::NonPositiveImageSize);
        }
        let width = dmabuf.width as u32;
        let height = dmabuf.height as u32;
        let disjoint = dmabuf.is_disjoint();
        let image = {
            if !validate_modifier(
                width,
                height,
                usage,
                disjoint,
                Some(dmabuf.planes.len()),
                format,
                dmabuf.modifier,
            ) {
                return Err(VulkanError::ModifierNotSupported);
            }
            let plane_layouts: PlaneVec<_> = dmabuf
                .planes
                .iter()
                .map(|p| {
                    SubresourceLayout::default()
                        .offset(p.offset as _)
                        .row_pitch(p.stride as _)
                })
                .collect();
            let mut modifier_info = ImageDrmFormatModifierExplicitCreateInfoEXT::default()
                .plane_layouts(&plane_layouts)
                .drm_format_modifier(dmabuf.modifier);
            let mut memory_image_create_info = ExternalMemoryImageCreateInfo::default()
                .handle_types(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let create_info = image_create_info(width, height, format.format, usage)
                .push_next(&mut memory_image_create_info)
                .push_next(&mut modifier_info);
            let res = unsafe { data.device.device.create_image(&create_info, None) };
            res.map_err(VulkanError::CreateImage)?
        };
        let destroy_image = OnDrop(|| unsafe { data.device.device.destroy_image(image, None) });
        let num_device_memories = match disjoint {
            true => dmabuf.planes.len(),
            false => 1,
        };
        let mut device_memories = PlaneVec::new();
        let mut free_device_memories = PlaneVec::new();
        let mut bind_image_plane_memory_infos = PlaneVec::new();
        for plane_idx in 0..num_device_memories {
            let dma_buf_plane = &dmabuf.planes[plane_idx];
            let mut memory_fd_properties = MemoryFdPropertiesKHR::default();
            unsafe {
                data.device
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
            if disjoint {
                let plane_aspect = [
                    ImageAspectFlags::MEMORY_PLANE_0_EXT,
                    ImageAspectFlags::MEMORY_PLANE_1_EXT,
                    ImageAspectFlags::MEMORY_PLANE_2_EXT,
                    ImageAspectFlags::MEMORY_PLANE_3_EXT,
                ][plane_idx];
                image_plane_memory_requirements_info =
                    ImagePlaneMemoryRequirementsInfo::default().plane_aspect(plane_aspect);
                image_memory_requirements_info = image_memory_requirements_info
                    .push_next(&mut image_plane_memory_requirements_info);
                bind_image_plane_memory_infos
                    .push(BindImagePlaneMemoryInfo::default().plane_aspect(plane_aspect));
            }
            let mut memory_requirements = MemoryRequirements2::default();
            unsafe {
                data.device.device.get_image_memory_requirements2(
                    &image_memory_requirements_info,
                    &mut memory_requirements,
                );
            }
            let memory_type_bits = memory_requirements.memory_requirements.memory_type_bits
                & memory_fd_properties.memory_type_bits;
            let memory_type_index = data
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
            let device_memory = unsafe {
                data.device
                    .device
                    .allocate_memory(&memory_allocate_info, None)
            };
            let device_memory = device_memory.map_err(VulkanError::AllocateMemory)?;
            fd.unwrap();
            device_memories.push(device_memory);
            free_device_memories.push(OnDrop(move || unsafe {
                data.device.device.free_memory(device_memory, None)
            }));
        }
        let mut bind_image_memory_infos = Vec::with_capacity(num_device_memories);
        let mut bind_image_plane_memory_infos = bind_image_plane_memory_infos.iter_mut();
        for mem in device_memories.iter().copied() {
            let mut info = BindImageMemoryInfo::default().image(image).memory(mem);
            if disjoint {
                info = info.push_next(bind_image_plane_memory_infos.next().unwrap());
            }
            bind_image_memory_infos.push(info);
        }
        let res = unsafe {
            data.device
                .device
                .bind_image_memory2(&bind_image_memory_infos)
        };
        res.map_err(VulkanError::BindImageMemory)?;
        destroy_image.forget();
        free_device_memories.drain(..).for_each(|m| m.forget());
        Ok(Rc::new(VulkanBo {
            allocator: data.clone(),
            image,
            memory: device_memories,
            buf: dmabuf.clone(),
        }))
    }
}

impl Allocator for VulkanBoAllocator {
    fn drm(&self) -> Option<&Drm> {
        Some(&self.data.drm)
    }

    fn create_bo(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
        modifiers: &[Modifier],
        usage: BufferUsage,
    ) -> Result<Rc<dyn BufferObject>, AllocatorError> {
        let bo = self.create_bo(dma_buf_ids, width, height, format, modifiers, usage)?;
        Ok(bo)
    }

    fn import_dmabuf(
        &self,
        dmabuf: &DmaBuf,
        usage: BufferUsage,
    ) -> Result<Rc<dyn BufferObject>, AllocatorError> {
        let bo = self.import_dmabuf(dmabuf, usage)?;
        Ok(bo)
    }
}

impl VulkanBo {
    fn map(self: &Rc<Self>, write: bool) -> Result<VulkanBoMapping, VulkanError> {
        let format = self.buf.format;
        let Some(shm_info) = &format.shm_info else {
            return Err(VulkanError::ShmNotSupported);
        };
        let stride = self.buf.width as u32 * shm_info.bpp;
        let size = self.buf.height as u32 * stride;
        let data = &self.allocator;
        let staging =
            data.device
                .create_staging_buffer(&data.allocator, size as _, write, true, true)?;
        self.transfer(&staging, false, |cmd| {
            let region = BufferImageCopy2::default()
                .image_subresource(
                    ImageSubresourceLayers::default()
                        .aspect_mask(ImageAspectFlags::COLOR)
                        .layer_count(1),
                )
                .image_extent(Extent3D {
                    width: self.buf.width as _,
                    height: self.buf.height as _,
                    depth: 1,
                });
            let copy_info = CopyImageToBufferInfo2::default()
                .src_image(self.image)
                .src_image_layout(ImageLayout::TRANSFER_SRC_OPTIMAL)
                .regions(slice::from_ref(&region))
                .dst_buffer(staging.buffer);
            unsafe {
                data.device
                    .device
                    .cmd_copy_image_to_buffer2(cmd, &copy_info);
            }
        })?;
        staging.download(|_, _| ())?;
        let data = unsafe {
            slice::from_raw_parts_mut(
                staging.allocation.mem.unwrap(),
                staging.allocation.size as _,
            )
        };
        Ok(VulkanBoMapping {
            bo: self.clone(),
            upload: write,
            stride: stride as _,
            staging,
            data,
        })
    }

    fn transfer<F>(
        &self,
        staging: &VulkanStagingBuffer,
        write: bool,
        f: F,
    ) -> Result<(), VulkanError>
    where
        F: FnOnce(CommandBuffer),
    {
        let data = &self.allocator;
        let cmd = data.command_buffer.buffer;
        let device = &data.device.device;
        let begin =
            CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        let initial_image_barrier = self.initial_image_barrier(write);
        let final_image_barrier = self.final_image_barrier(write);
        let mut initial_buffer_barrier = ArrayVec::<_, 1>::new();
        let mut final_buffer_barrier = ArrayVec::<_, 1>::new();
        if write {
            initial_buffer_barrier.push(
                BufferMemoryBarrier2::default()
                    .src_access_mask(AccessFlags2::HOST_WRITE)
                    .src_stage_mask(PipelineStageFlags2::HOST)
                    .dst_access_mask(AccessFlags2::TRANSFER_READ)
                    .dst_stage_mask(PipelineStageFlags2::TRANSFER)
                    .buffer(staging.buffer)
                    .size(staging.size),
            );
        } else {
            final_buffer_barrier.push(
                BufferMemoryBarrier2::default()
                    .src_access_mask(AccessFlags2::TRANSFER_WRITE)
                    .src_stage_mask(PipelineStageFlags2::TRANSFER)
                    .dst_access_mask(AccessFlags2::HOST_READ | AccessFlags2::HOST_WRITE)
                    .dst_stage_mask(PipelineStageFlags2::HOST)
                    .buffer(staging.buffer)
                    .size(staging.size),
            );
        }
        let initial_dependency_info = DependencyInfo::default()
            .image_memory_barriers(slice::from_ref(&initial_image_barrier))
            .buffer_memory_barriers(&initial_buffer_barrier);
        let final_dependency_info = DependencyInfo::default()
            .image_memory_barriers(slice::from_ref(&final_image_barrier))
            .buffer_memory_barriers(&final_buffer_barrier);
        let cmd_buffer_submit_info = CommandBufferSubmitInfo::default().command_buffer(cmd);
        let submit_info =
            SubmitInfo2::default().command_buffer_infos(slice::from_ref(&cmd_buffer_submit_info));
        unsafe {
            device
                .begin_command_buffer(cmd, &begin)
                .map_err(VulkanError::BeginCommandBuffer)?;
            device.cmd_pipeline_barrier2(cmd, &initial_dependency_info);
            f(cmd);
            device.cmd_pipeline_barrier2(cmd, &final_dependency_info);
            device
                .end_command_buffer(cmd)
                .map_err(VulkanError::EndCommandBuffer)?;
            device
                .queue_submit2(
                    data.device.graphics_queue,
                    slice::from_ref(&submit_info),
                    Fence::null(),
                )
                .map_err(VulkanError::Submit)?;
            device.device_wait_idle().map_err(VulkanError::WaitIdle)?;
        }
        Ok(())
    }

    fn get_image_barrier_flags(&self, write: bool) -> (ImageLayout, AccessFlags2) {
        let layout;
        let access_mask;
        match write {
            false => {
                layout = ImageLayout::TRANSFER_SRC_OPTIMAL;
                access_mask = AccessFlags2::TRANSFER_READ;
            }
            true => {
                layout = ImageLayout::TRANSFER_DST_OPTIMAL;
                access_mask = AccessFlags2::TRANSFER_WRITE;
            }
        }
        (layout, access_mask)
    }

    fn initial_image_barrier(&self, write: bool) -> ImageMemoryBarrier2<'static> {
        let (new_layout, dst_access_mask) = self.get_image_barrier_flags(write);
        image_barrier()
            .src_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
            .dst_queue_family_index(self.allocator.device.graphics_queue_idx)
            .old_layout(ImageLayout::GENERAL)
            .new_layout(new_layout)
            .dst_access_mask(dst_access_mask)
            .dst_stage_mask(PipelineStageFlags2::TRANSFER)
            .image(self.image)
    }

    fn final_image_barrier(&self, write: bool) -> ImageMemoryBarrier2<'static> {
        let (old_layout, src_access_mask) = self.get_image_barrier_flags(write);
        image_barrier()
            .src_queue_family_index(self.allocator.device.graphics_queue_idx)
            .dst_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
            .old_layout(old_layout)
            .new_layout(ImageLayout::GENERAL)
            .src_access_mask(src_access_mask)
            .src_stage_mask(PipelineStageFlags2::TRANSFER)
            .image(self.image)
    }
}

impl BufferObject for VulkanBo {
    fn dmabuf(&self) -> &DmaBuf {
        &self.buf
    }

    fn map_read(self: Rc<Self>) -> Result<Box<dyn MappedBuffer>, AllocatorError> {
        let m = self.map(false)?;
        Ok(Box::new(m))
    }

    fn map_write(self: Rc<Self>) -> Result<Box<dyn MappedBuffer>, AllocatorError> {
        let m = self.map(true)?;
        Ok(Box::new(m))
    }
}

impl VulkanBoMapping {
    fn upload(&self) -> Result<(), VulkanError> {
        let data = &self.bo.allocator;
        self.staging.upload(|_, _| ())?;
        self.bo.transfer(&self.staging, true, |cmd| {
            let region = BufferImageCopy2::default()
                .image_subresource(
                    ImageSubresourceLayers::default()
                        .aspect_mask(ImageAspectFlags::COLOR)
                        .layer_count(1),
                )
                .image_extent(Extent3D {
                    width: self.bo.buf.width as _,
                    height: self.bo.buf.height as _,
                    depth: 1,
                });
            let copy_info = CopyBufferToImageInfo2::default()
                .dst_image(self.bo.image)
                .dst_image_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                .regions(slice::from_ref(&region))
                .src_buffer(self.staging.buffer);
            unsafe {
                data.device
                    .device
                    .cmd_copy_buffer_to_image2(cmd, &copy_info);
            }
        })?;
        Ok(())
    }
}

impl Drop for VulkanBoMapping {
    fn drop(&mut self) {
        if self.upload {
            if let Err(e) = self.upload() {
                log::error!("Could not upload to image: {}", ErrorFmt(e));
            }
        }
    }
}

impl MappedBuffer for VulkanBoMapping {
    unsafe fn data(&self) -> &[u8] {
        &*self.data
    }

    fn data_ptr(&self) -> *mut u8 {
        self.data as _
    }

    fn stride(&self) -> i32 {
        self.stride
    }
}

fn validate_usage(usage: BufferUsage) -> Result<(), VulkanError> {
    if usage.contains(!(BO_USE_WRITE | BO_USE_RENDERING)) {
        return Err(VulkanError::UnsupportedBufferUsage);
    }
    Ok(())
}

fn map_usage(usage: BufferUsage) -> ImageUsageFlags {
    let mut vk_usage = ImageUsageFlags::TRANSFER_SRC | ImageUsageFlags::TRANSFER_DST;
    if usage.contains(BO_USE_RENDERING) {
        vk_usage |= ImageUsageFlags::COLOR_ATTACHMENT;
    }
    vk_usage
}

fn validate_modifier(
    width: u32,
    height: u32,
    usage: BufferUsage,
    disjoint: bool,
    plane_count: Option<usize>,
    format: &VulkanFormat,
    modifier: Modifier,
) -> bool {
    let Some(modifier) = format.modifiers.get(&modifier) else {
        return false;
    };
    if disjoint && !modifier.features.contains(FormatFeatureFlags::DISJOINT) {
        return false;
    }
    if let Some(plane_count) = plane_count {
        if plane_count != modifier.planes {
            return false;
        }
    }
    let Some(limits) = modifier.transfer_limits else {
        return false;
    };
    let mut max_width = limits.max_width;
    let mut max_height = limits.max_height;
    let mut exportable = limits.exportable;
    if usage.contains(BO_USE_RENDERING) {
        let Some(limits) = modifier.render_limits else {
            return false;
        };
        max_width = max_width.min(limits.max_width);
        max_height = max_height.min(limits.max_height);
        exportable &= limits.exportable;
    }
    if !exportable || width > max_width || height > max_height {
        return false;
    }
    true
}

fn image_create_info(
    width: u32,
    height: u32,
    format: &Format,
    usage: BufferUsage,
) -> ImageCreateInfo {
    let usage = map_usage(usage);
    ImageCreateInfo::default()
        .image_type(ImageType::TYPE_2D)
        .format(format.vk_format)
        .mip_levels(1)
        .array_layers(1)
        .tiling(ImageTiling::DRM_FORMAT_MODIFIER_EXT)
        .samples(SampleCountFlags::TYPE_1)
        .sharing_mode(SharingMode::EXCLUSIVE)
        .initial_layout(ImageLayout::UNDEFINED)
        .extent(Extent3D {
            width,
            height,
            depth: 1,
        })
        .usage(usage)
}
