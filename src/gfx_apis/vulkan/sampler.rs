use {
    crate::gfx_apis::vulkan::{VulkanError, device::VulkanDevice},
    ash::vk::{
        BorderColor, Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode,
    },
    std::rc::Rc,
};

pub struct VulkanSampler {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) sampler: Sampler,
}

impl VulkanDevice {
    pub(super) fn create_sampler(self: &Rc<Self>) -> Result<Rc<VulkanSampler>, VulkanError> {
        let create_info = SamplerCreateInfo::default()
            .mag_filter(Filter::LINEAR)
            .min_filter(Filter::LINEAR)
            .mipmap_mode(SamplerMipmapMode::NEAREST)
            .address_mode_u(SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(SamplerAddressMode::CLAMP_TO_EDGE)
            .max_anisotropy(1.0)
            .min_lod(0.0)
            .max_lod(0.25)
            .border_color(BorderColor::FLOAT_TRANSPARENT_BLACK);
        let sampler = unsafe { self.device.create_sampler(&create_info, None) };
        let sampler = sampler.map_err(VulkanError::CreateSampler)?;
        Ok(Rc::new(VulkanSampler {
            device: self.clone(),
            sampler,
        }))
    }
}

impl Drop for VulkanSampler {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_sampler(self.sampler, None);
        }
    }
}
