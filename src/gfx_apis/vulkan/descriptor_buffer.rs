use {
    crate::{
        gfx_apis::vulkan::{
            allocator::{VulkanAllocation, VulkanAllocator},
            descriptor::VulkanDescriptorSetLayout,
            device::VulkanDevice,
            VulkanError,
        },
        utils::on_drop::OnDrop,
    },
    ash::vk::{
        Buffer, BufferCreateInfo, BufferDeviceAddressInfo, BufferUsageFlags, DeviceAddress,
        DeviceSize,
    },
    gpu_alloc::UsageFlags,
    std::{cell::RefCell, mem::ManuallyDrop, ops::Deref, rc::Rc},
};

pub struct VulkanDescriptorBufferCache {
    device: Rc<VulkanDevice>,
    allocator: Rc<VulkanAllocator>,
    buffers: RefCell<Vec<VulkanDescriptorBufferUnused>>,
    has_sampler: bool,
}

pub struct VulkanDescriptorBuffer {
    cache: Rc<VulkanDescriptorBufferCache>,
    pub buffer: ManuallyDrop<VulkanDescriptorBufferUnused>,
}

pub struct VulkanDescriptorBufferUnused {
    device: Rc<VulkanDevice>,
    pub size: DeviceSize,
    pub buffer: Buffer,
    pub allocation: VulkanAllocation,
    pub address: DeviceAddress,
}

pub struct VulkanDescriptorBufferWriter {
    set_size: usize,
    buffer: Vec<u8>,
}

pub struct VulkanDescriptorBufferSetWriter<'a> {
    set: &'a mut [u8],
}

impl VulkanDescriptorBufferCache {
    pub fn new(
        device: &Rc<VulkanDevice>,
        allocator: &Rc<VulkanAllocator>,
        layout: &VulkanDescriptorSetLayout,
    ) -> Self {
        Self {
            device: device.clone(),
            allocator: allocator.clone(),
            buffers: Default::default(),
            has_sampler: layout.has_sampler,
        }
    }

    pub fn allocate(
        self: &Rc<Self>,
        capacity: DeviceSize,
    ) -> Result<VulkanDescriptorBuffer, VulkanError> {
        const MIN_ALLOCATION: DeviceSize = 2048;
        let capacity = capacity.max(MIN_ALLOCATION);
        let mut smallest = None;
        let mut smallest_size = DeviceSize::MAX;
        let mut fitting = None;
        let mut fitting_size = DeviceSize::MAX;
        let buffers = &mut *self.buffers.borrow_mut();
        for (idx, buffer) in buffers.iter().enumerate() {
            if buffer.size >= capacity {
                if buffer.size < fitting_size {
                    fitting = Some(idx);
                    fitting_size = buffer.size;
                }
            } else {
                if buffer.size < smallest_size {
                    smallest = Some(idx);
                    smallest_size = buffer.size;
                }
            }
        }
        if let Some(idx) = fitting {
            return Ok(VulkanDescriptorBuffer {
                cache: self.clone(),
                buffer: ManuallyDrop::new(buffers.swap_remove(idx)),
            });
        }
        if let Some(idx) = smallest {
            log::debug!("discarding size {}", smallest_size);
            buffers.swap_remove(idx);
        }
        let size = capacity.checked_next_power_of_two().unwrap();
        log::debug!("allocating size {}", size);
        let buffer = {
            let usage = self.usage();
            let info = BufferCreateInfo::default().size(size).usage(usage);
            unsafe {
                self.device
                    .device
                    .create_buffer(&info, None)
                    .map_err(VulkanError::CreateBuffer)?
            }
        };
        let destroy_buffer = OnDrop(|| unsafe { self.device.device.destroy_buffer(buffer, None) });
        let memory_requirements =
            unsafe { self.device.device.get_buffer_memory_requirements(buffer) };
        let allocation = {
            let flags = UsageFlags::UPLOAD
                | UsageFlags::FAST_DEVICE_ACCESS
                | UsageFlags::HOST_ACCESS
                | UsageFlags::DEVICE_ADDRESS;
            self.allocator.alloc(&memory_requirements, flags, true)?
        };
        log::info!("memory_requirements: {:?}", memory_requirements);
        log::info!("block: {}", allocation.block_debug);
        unsafe {
            self.device
                .device
                .bind_buffer_memory(buffer, allocation.memory, allocation.offset)
                .map_err(VulkanError::BindBufferMemory)?;
        }
        destroy_buffer.forget();
        let address = {
            let info = BufferDeviceAddressInfo::default().buffer(buffer);
            unsafe { self.device.device.get_buffer_device_address(&info) }
        };
        Ok(VulkanDescriptorBuffer {
            cache: self.clone(),
            buffer: ManuallyDrop::new(VulkanDescriptorBufferUnused {
                device: self.device.clone(),
                size,
                buffer,
                allocation,
                address,
            }),
        })
    }

    pub fn usage(&self) -> BufferUsageFlags {
        let mut usage = BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT
            | BufferUsageFlags::SHADER_DEVICE_ADDRESS;
        if self.has_sampler {
            usage |= BufferUsageFlags::SAMPLER_DESCRIPTOR_BUFFER_EXT;
        }
        usage
    }
}

impl Drop for VulkanDescriptorBuffer {
    fn drop(&mut self) {
        let buffer = unsafe { ManuallyDrop::take(&mut self.buffer) };
        self.cache.buffers.borrow_mut().push(buffer);
    }
}

impl Drop for VulkanDescriptorBufferUnused {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_buffer(self.buffer, None);
        }
    }
}

impl VulkanDescriptorBufferWriter {
    pub fn new(layout: &VulkanDescriptorSetLayout) -> Self {
        Self {
            set_size: layout.size as usize,
            buffer: Default::default(),
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn next_offset(&self) -> DeviceSize {
        self.buffer.len() as DeviceSize
    }

    pub fn add_set(&mut self) -> VulkanDescriptorBufferSetWriter<'_> {
        let buffer = &mut self.buffer;
        let lo = buffer.len();
        buffer.resize(lo + self.set_size, 0);
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
