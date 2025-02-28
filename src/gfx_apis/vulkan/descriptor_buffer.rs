use {
    crate::gfx_apis::vulkan::descriptor::VulkanDescriptorSetLayout, ash::vk::DeviceSize,
    std::ops::Deref,
};

#[derive(Default)]
pub struct VulkanDescriptorBufferWriter {
    buffer: Vec<u8>,
}

pub struct VulkanDescriptorBufferSetWriter<'a> {
    set: &'a mut [u8],
}

impl VulkanDescriptorBufferWriter {
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn next_offset(&self) -> DeviceSize {
        self.buffer.len() as DeviceSize
    }

    pub fn add_set(
        &mut self,
        layout: &VulkanDescriptorSetLayout,
    ) -> VulkanDescriptorBufferSetWriter<'_> {
        let buffer = &mut self.buffer;
        let lo = buffer.len();
        buffer.resize(lo + layout.size as usize, 0);
        VulkanDescriptorBufferSetWriter {
            set: &mut buffer[lo..],
        }
    }
}

impl VulkanDescriptorBufferSetWriter<'_> {
    pub fn write(&mut self, offset: DeviceSize, data: &[u8]) {
        let offset = offset as usize;
        let set = &mut *self.set;
        assert!(offset <= set.len());
        assert!(data.len() <= set.len() - offset);
        set[offset..offset + data.len()].copy_from_slice(data);
    }
}

impl Deref for VulkanDescriptorBufferWriter {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}
