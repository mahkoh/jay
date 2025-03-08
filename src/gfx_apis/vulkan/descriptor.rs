use {
    crate::gfx_apis::vulkan::{VulkanError, device::VulkanDevice, sampler::VulkanSampler},
    arrayvec::ArrayVec,
    ash::{
        ext::descriptor_buffer,
        vk::{
            DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateFlags,
            DescriptorSetLayoutCreateInfo, DescriptorType, DeviceSize, ShaderStageFlags,
        },
    },
    std::{rc::Rc, slice},
};

pub(super) struct VulkanDescriptorSetLayout {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) layout: DescriptorSetLayout,
    pub(super) size: DeviceSize,
    pub(super) offsets: ArrayVec<DeviceSize, 2>,
    pub(super) _sampler: Option<Rc<VulkanSampler>>,
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
    pub(super) fn create_tex_legacy_descriptor_set_layout(
        self: &Rc<Self>,
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
            device: self.clone(),
            layout,
            size: 0,
            offsets: Default::default(),
            _sampler: Some(sampler.clone()),
        }))
    }

    pub(super) fn create_tex_sampler_descriptor_set_layout(
        self: &Rc<Self>,
        sampler: &Rc<VulkanSampler>,
    ) -> Result<Rc<VulkanDescriptorSetLayout>, VulkanError> {
        let immutable_sampler = [sampler.sampler];
        let binding = DescriptorSetLayoutBinding::default()
            .stage_flags(ShaderStageFlags::FRAGMENT)
            .immutable_samplers(&immutable_sampler)
            .descriptor_count(1)
            .descriptor_type(DescriptorType::SAMPLER);
        let create_info = DescriptorSetLayoutCreateInfo::default()
            .bindings(slice::from_ref(&binding))
            .flags(DescriptorSetLayoutCreateFlags::DESCRIPTOR_BUFFER_EXT);
        let layout = unsafe { self.device.create_descriptor_set_layout(&create_info, None) };
        let layout = layout.map_err(VulkanError::CreateDescriptorSetLayout)?;
        let db = self.descriptor_buffer.as_ref().unwrap();
        let size = self.get_descriptor_set_size(db, layout);
        let mut offsets = ArrayVec::new();
        unsafe {
            offsets.push(db.get_descriptor_set_layout_binding_offset(layout, 0));
        }
        Ok(Rc::new(VulkanDescriptorSetLayout {
            device: self.clone(),
            layout,
            size,
            offsets,
            _sampler: Some(sampler.clone()),
        }))
    }

    pub(super) fn create_tex_resource_descriptor_set_layout(
        self: &Rc<Self>,
    ) -> Result<Rc<VulkanDescriptorSetLayout>, VulkanError> {
        let bindings = [
            DescriptorSetLayoutBinding::default()
                .binding(0)
                .stage_flags(ShaderStageFlags::FRAGMENT)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::SAMPLED_IMAGE),
            DescriptorSetLayoutBinding::default()
                .binding(1)
                .stage_flags(ShaderStageFlags::FRAGMENT)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER),
        ];
        let create_info = DescriptorSetLayoutCreateInfo::default()
            .bindings(&bindings)
            .flags(DescriptorSetLayoutCreateFlags::DESCRIPTOR_BUFFER_EXT);
        let layout = unsafe { self.device.create_descriptor_set_layout(&create_info, None) };
        let layout = layout.map_err(VulkanError::CreateDescriptorSetLayout)?;
        let db = self.descriptor_buffer.as_ref().unwrap();
        let size = self.get_descriptor_set_size(db, layout);
        let mut offsets = ArrayVec::new();
        unsafe {
            offsets.push(db.get_descriptor_set_layout_binding_offset(layout, 0));
            offsets.push(db.get_descriptor_set_layout_binding_offset(layout, 1));
        }
        Ok(Rc::new(VulkanDescriptorSetLayout {
            device: self.clone(),
            layout,
            size,
            offsets,
            _sampler: None,
        }))
    }

    pub(super) fn create_out_descriptor_set_layout(
        self: &Rc<Self>,
        db: &descriptor_buffer::Device,
    ) -> Result<Rc<VulkanDescriptorSetLayout>, VulkanError> {
        let binding = DescriptorSetLayoutBinding::default()
            .stage_flags(ShaderStageFlags::FRAGMENT)
            .descriptor_count(1)
            .descriptor_type(DescriptorType::SAMPLED_IMAGE);
        let create_info = DescriptorSetLayoutCreateInfo::default()
            .bindings(slice::from_ref(&binding))
            .flags(DescriptorSetLayoutCreateFlags::DESCRIPTOR_BUFFER_EXT);
        let layout = unsafe { self.device.create_descriptor_set_layout(&create_info, None) };
        let layout = layout.map_err(VulkanError::CreateDescriptorSetLayout)?;
        let size = self.get_descriptor_set_size(db, layout);
        let mut offsets = ArrayVec::new();
        unsafe {
            offsets.push(db.get_descriptor_set_layout_binding_offset(layout, 0));
        }
        Ok(Rc::new(VulkanDescriptorSetLayout {
            device: self.clone(),
            layout,
            size,
            offsets,
            _sampler: None,
        }))
    }

    fn get_descriptor_set_size(
        &self,
        db: &descriptor_buffer::Device,
        layout: DescriptorSetLayout,
    ) -> DeviceSize {
        let mut size = unsafe { db.get_descriptor_set_layout_size(layout) };
        size = (size + self.descriptor_buffer_offset_mask) & !self.descriptor_buffer_offset_mask;
        size
    }
}
