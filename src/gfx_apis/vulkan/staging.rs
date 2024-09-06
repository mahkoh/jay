use {
    crate::{
        gfx_apis::vulkan::{
            allocator::{VulkanAllocation, VulkanAllocator},
            device::VulkanDevice,
            VulkanError,
        },
        utils::on_drop::OnDrop,
    },
    ash::vk::{Buffer, BufferCreateInfo, BufferUsageFlags, MappedMemoryRange},
    gpu_alloc::UsageFlags,
    std::rc::Rc,
};

pub struct VulkanStagingBuffer {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) allocation: VulkanAllocation,
    pub(super) buffer: Buffer,
    pub(super) size: u64,
}

impl VulkanDevice {
    pub(super) fn create_staging_buffer(
        self: &Rc<Self>,
        allocator: &Rc<VulkanAllocator>,
        size: u64,
        upload: bool,
        download: bool,
        transient: bool,
    ) -> Result<VulkanStagingBuffer, VulkanError> {
        let mut vk_usage = BufferUsageFlags::empty();
        let mut usage = UsageFlags::empty();
        if upload {
            vk_usage |= BufferUsageFlags::TRANSFER_SRC;
            usage |= UsageFlags::UPLOAD;
        }
        if download {
            vk_usage |= BufferUsageFlags::TRANSFER_DST;
            usage |= UsageFlags::DOWNLOAD;
        }
        if transient {
            usage |= UsageFlags::TRANSIENT;
        }
        let buffer = {
            let create_info = BufferCreateInfo::default().size(size).usage(vk_usage);
            let buffer = unsafe { self.device.create_buffer(&create_info, None) };
            buffer.map_err(VulkanError::CreateBuffer)?
        };
        let destroy_buffer = OnDrop(|| unsafe { self.device.destroy_buffer(buffer, None) });
        let memory_requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        let allocation = allocator.alloc(&memory_requirements, usage, true)?;
        {
            let res = unsafe {
                self.device
                    .bind_buffer_memory(buffer, allocation.memory, allocation.offset)
            };
            res.map_err(VulkanError::BindBufferMemory)?;
        }
        destroy_buffer.forget();
        Ok(VulkanStagingBuffer {
            device: self.clone(),
            allocation,
            buffer,
            size,
        })
    }
}

impl VulkanStagingBuffer {
    pub fn upload<T, F>(&self, f: F) -> Result<T, VulkanError>
    where
        F: FnOnce(*mut u8, usize) -> T,
    {
        let t = f(self.allocation.mem.unwrap(), self.size as usize);
        if let Some(mask) = self.allocation.coherency_mask {
            let range = self.incoherent_range(mask);
            let res = unsafe { self.device.device.flush_mapped_memory_ranges(&[range]) };
            res.map_err(VulkanError::FlushMemory)?;
        }
        Ok(t)
    }

    pub fn download<T, F>(&self, f: F) -> Result<T, VulkanError>
    where
        F: FnOnce(*const u8, usize) -> T,
    {
        if let Some(mask) = self.allocation.coherency_mask {
            let range = self.incoherent_range(mask);
            let res = unsafe { self.device.device.invalidate_mapped_memory_ranges(&[range]) };
            res.map_err(VulkanError::FlushMemory)?;
        }
        Ok(f(self.allocation.mem.unwrap(), self.size as usize))
    }

    fn incoherent_range(&self, mask: u64) -> MappedMemoryRange {
        MappedMemoryRange::default()
            .memory(self.allocation.memory)
            .offset(self.allocation.offset & !mask)
            .size((self.allocation.size + mask) & !mask)
    }
}

impl Drop for VulkanStagingBuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_buffer(self.buffer, None);
        }
    }
}
