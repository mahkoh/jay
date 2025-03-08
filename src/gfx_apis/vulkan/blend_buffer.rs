use {
    crate::{
        gfx_api::GfxBlendBuffer,
        gfx_apis::vulkan::{
            VulkanError,
            format::{BLEND_FORMAT, BLEND_USAGE},
            image::{QueueFamily, QueueState, VulkanImage, VulkanImageMemory},
            renderer::VulkanRenderer,
        },
        utils::on_drop::OnDrop,
    },
    ash::vk::{
        Extent3D, ImageAspectFlags, ImageCreateInfo, ImageLayout, ImageSubresourceRange,
        ImageTiling, ImageType, ImageViewCreateInfo, ImageViewType, SampleCountFlags, SharingMode,
    },
    gpu_alloc::UsageFlags,
    std::{any::Any, cell::Cell, collections::hash_map::Entry, rc::Rc},
};

impl VulkanRenderer {
    pub fn acquire_blend_buffer(
        self: &Rc<Self>,
        width: i32,
        height: i32,
    ) -> Result<Rc<VulkanImage>, VulkanError> {
        if self.device.descriptor_buffer.is_none() {
            return Err(VulkanError::NoDescriptorBuffer);
        }
        if width <= 0 || height <= 0 {
            return Err(VulkanError::NonPositiveImageSize);
        }
        let width = width as u32;
        let height = height as u32;
        let cached = &mut *self.blend_buffers.borrow_mut();
        let cached = cached.entry((width, height));
        if let Entry::Occupied(entry) = &cached {
            if let Some(buffer) = entry.get().upgrade() {
                return Ok(buffer);
            }
        }
        let limits = self.device.blend_limits;
        if width > limits.max_width || height > limits.max_height {
            return Err(VulkanError::ImageTooLarge);
        }
        let create_info = ImageCreateInfo::default()
            .image_type(ImageType::TYPE_2D)
            .format(BLEND_FORMAT.vk_format)
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
            .usage(BLEND_USAGE);
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
            .format(BLEND_FORMAT.vk_format)
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
        destroy_image.forget();
        let img = Rc::new(VulkanImage {
            renderer: self.clone(),
            format: BLEND_FORMAT,
            width,
            height,
            stride: 0,
            texture_view: view,
            render_view: None,
            image,
            is_undefined: Cell::new(true),
            contents_are_undefined: Cell::new(true),
            queue_state: Cell::new(QueueState::Acquired {
                family: QueueFamily::Gfx,
            }),
            ty: VulkanImageMemory::Blend(allocation),
            bridge: None,
            sampled_image_descriptor: self.sampled_image_descriptor(view),
            execution_version: Default::default(),
        });
        cached.insert_entry(Rc::downgrade(&img));
        Ok(img)
    }
}

impl GfxBlendBuffer for VulkanImage {
    fn into_any(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }
}
