use crate::gfx_api::ScalingFilter;
use crate::gfx_apis::vulkan::VulkanError;
use crate::gfx_apis::vulkan::device::DescriptorHeapDevice;
use crate::gfx_apis::vulkan::device::VulkanDevice;
use ash::vk::BorderColor;
use ash::vk::Filter;
use ash::vk::HostAddressRangeEXT;
use ash::vk::Sampler;
use ash::vk::SamplerAddressMode;
use ash::vk::SamplerCreateInfo;
use ash::vk::SamplerMipmapMode;
use std::rc::Rc;
use std::slice;

pub struct VulkanSampler {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) create_info: SamplerCreateInfo<'static>,
    pub(super) sampler: Sampler,
}

impl VulkanDevice {
    pub(super) fn create_sampler(
        self: &Rc<Self>,
        filter: ScalingFilter,
    ) -> Result<Rc<VulkanSampler>, VulkanError> {
        let filter = match filter {
            ScalingFilter::Linear => Filter::LINEAR,
            ScalingFilter::Nearest => Filter::NEAREST,
        };
        let create_info = SamplerCreateInfo::default()
            .mag_filter(filter)
            .min_filter(filter)
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
            create_info,
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

impl DescriptorHeapDevice {
    pub(super) fn create_sampler_descriptor(
        &self,
        sampler: &SamplerCreateInfo,
    ) -> Result<Box<[u8]>, VulkanError> {
        let mut buf = vec![0; self.sampler_descriptor_size].into_boxed_slice();
        let descriptor = HostAddressRangeEXT::default().address(&mut buf);
        unsafe {
            self.device
                .write_sampler_descriptors(slice::from_ref(sampler), slice::from_ref(&descriptor))
                .map_err(VulkanError::WriteDescriptor)?;
        }
        Ok(buf)
    }
}
