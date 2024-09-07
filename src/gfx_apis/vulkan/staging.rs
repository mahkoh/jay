use {
    crate::{
        cpu_worker::CpuWorker,
        gfx_apis::vulkan::{
            allocator::{VulkanAllocation, VulkanAllocator},
            device::VulkanDevice,
            renderer::VulkanRenderer,
            VulkanError,
        },
        utils::on_drop::{OnDrop, OnDrop2},
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
        let (vk_usage, usage) = get_usage(upload, download, transient);
        let buffer = self.create_buffer(size, vk_usage)?;
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

    #[expect(dead_code)]
    pub(super) fn create_shm_staging(
        self: &Rc<Self>,
        renderer: &Rc<VulkanRenderer>,
        cpu: &Rc<CpuWorker>,
        size: u64,
        upload: bool,
        download: bool,
        cb: impl FnOnce(Result<VulkanStagingBuffer, VulkanError>) + 'static,
    ) -> Result<(), VulkanError> {
        let (vk_usage, usage) = get_usage(upload, download, false);
        let buffer = self.create_buffer(size, vk_usage)?;
        let memory_requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        let slf = self.clone();
        let destroy_buffer =
            OnDrop2::new(move || unsafe { slf.device.destroy_buffer(buffer, None) });
        let slf = self.clone();
        let finish_allocation = move |res| {
            let allocation: VulkanAllocation = res?;
            {
                let res = unsafe {
                    slf.device
                        .bind_buffer_memory(buffer, allocation.memory, allocation.offset)
                };
                res.map_err(VulkanError::BindBufferMemory)?;
            }
            destroy_buffer.forget();
            Ok(VulkanStagingBuffer {
                device: slf.clone(),
                allocation,
                buffer,
                size,
            })
        };
        renderer.shm_allocator.async_alloc(
            renderer,
            cpu,
            memory_requirements,
            usage,
            true,
            move |res| cb(finish_allocation(res)),
        )
    }

    fn create_buffer(&self, size: u64, usage: BufferUsageFlags) -> Result<Buffer, VulkanError> {
        let create_info = BufferCreateInfo::default().size(size).usage(usage);
        let buffer = unsafe { self.device.create_buffer(&create_info, None) };
        buffer.map_err(VulkanError::CreateBuffer)
    }
}

fn get_usage(upload: bool, download: bool, transient: bool) -> (BufferUsageFlags, UsageFlags) {
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
    (vk_usage, usage)
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
