use {
    crate::gfx_apis::vulkan::{device::VulkanDevice, sampler::VulkanSampler, VulkanError},
    ash::vk::{
        DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateFlags,
        DescriptorSetLayoutCreateInfo, DescriptorType, ShaderStageFlags,
    },
    std::{rc::Rc, slice},
};

pub(super) struct VulkanDescriptorSetLayout {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) layout: DescriptorSetLayout,
    pub(super) _sampler: Rc<VulkanSampler>,
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
        &self,
        sampler: &Rc<VulkanSampler>,
    ) -> Result<Rc<VulkanDescriptorSetLayout>, VulkanError> {
        let immutable_sampler = [sampler.sampler];
        let binding = DescriptorSetLayoutBinding::default()
            .stage_flags(ShaderStageFlags::FRAGMENT)
            .immutable_samplers(&immutable_sampler)
            .descriptor_count(1)
            .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER);
        let create_info = DescriptorSetLayoutCreateInfo::default()
            .bindings(slice::from_ref(&binding))
            .flags(DescriptorSetLayoutCreateFlags::PUSH_DESCRIPTOR_KHR);
        let layout = unsafe { self.device.create_descriptor_set_layout(&create_info, None) };
        let layout = layout.map_err(VulkanError::CreateDescriptorSetLayout)?;
        Ok(Rc::new(VulkanDescriptorSetLayout {
            device: sampler.device.clone(),
            layout,
            _sampler: sampler.clone(),
        }))
    }
}
