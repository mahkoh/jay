use {
    crate::gfx_apis::vulkan::{VulkanError, device::VulkanDevice, sampler::VulkanSampler},
    arrayvec::ArrayVec,
    ash::vk::{
        DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateFlags,
        DescriptorSetLayoutCreateInfo, DescriptorType, DeviceSize, ShaderStageFlags,
    },
    std::{rc::Rc, slice},
};

pub(super) struct VulkanDescriptorSetLayout {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) layout: DescriptorSetLayout,
    pub(super) size: DeviceSize,
    pub(super) offsets: ArrayVec<DeviceSize, 1>,
    pub(super) _sampler: Option<Rc<VulkanSampler>>,
    pub(super) has_sampler: bool,
}

impl Drop for VulkanDescriptorSetLayout {
    fn drop(&mut self) {
        unsafe {
            self.device
                .device
                .destroy_descriptor_set_layout(self.layout, None);
        }
    }
}

impl VulkanDevice {
    pub(super) fn create_descriptor_set_layout(
        self: &Rc<Self>,
        sampler: &Rc<VulkanSampler>,
    ) -> Result<Rc<VulkanDescriptorSetLayout>, VulkanError> {
        let immutable_sampler = [sampler.sampler];
        let binding = DescriptorSetLayoutBinding::default()
            .stage_flags(ShaderStageFlags::FRAGMENT)
            .immutable_samplers(&immutable_sampler)
            .descriptor_count(1)
            .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER);
        let mut flags = DescriptorSetLayoutCreateFlags::empty();
        if self.descriptor_buffer.is_some() {
            flags |= DescriptorSetLayoutCreateFlags::DESCRIPTOR_BUFFER_EXT;
        } else {
            flags |= DescriptorSetLayoutCreateFlags::PUSH_DESCRIPTOR_KHR;
        }
        let create_info = DescriptorSetLayoutCreateInfo::default()
            .bindings(slice::from_ref(&binding))
            .flags(flags);
        let layout = unsafe { self.device.create_descriptor_set_layout(&create_info, None) };
        let layout = layout.map_err(VulkanError::CreateDescriptorSetLayout)?;
        let mut size = 0;
        let mut offsets = ArrayVec::new();
        if let Some(db) = &self.descriptor_buffer {
            size = unsafe { db.get_descriptor_set_layout_size(layout) };
            size =
                (size + self.descriptor_buffer_offset_mask) & !self.descriptor_buffer_offset_mask;
            unsafe {
                offsets.push(db.get_descriptor_set_layout_binding_offset(layout, 0));
            }
        }
        Ok(Rc::new(VulkanDescriptorSetLayout {
            device: self.clone(),
            layout,
            size,
            offsets,
            _sampler: Some(sampler.clone()),
            has_sampler: true,
        }))
    }
}
