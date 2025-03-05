use {
    crate::{
        gfx_apis::vulkan::{
            VulkanError,
            allocator::{VulkanAllocation, VulkanAllocator},
            device::VulkanDevice,
        },
        utils::on_drop::OnDrop,
    },
    ash::vk::{
        Buffer, BufferCreateInfo, BufferDeviceAddressInfo, BufferUsageFlags, DeviceAddress,
        DeviceSize,
    },
    gpu_alloc::UsageFlags,
    std::{cell::RefCell, mem::ManuallyDrop, rc::Rc},
};

pub struct VulkanBufferCache {
    device: Rc<VulkanDevice>,
    allocator: Rc<VulkanAllocator>,
    buffers: RefCell<Vec<VulkanBufferUnused>>,
    usage: BufferUsageFlags,
}

pub struct VulkanBuffer {
    cache: Rc<VulkanBufferCache>,
    pub buffer: ManuallyDrop<VulkanBufferUnused>,
}

pub struct VulkanBufferUnused {
    device: Rc<VulkanDevice>,
    pub size: DeviceSize,
    pub buffer: Buffer,
    pub allocation: VulkanAllocation,
    pub address: DeviceAddress,
}

impl VulkanBufferCache {
    pub fn new(
        device: &Rc<VulkanDevice>,
        allocator: &Rc<VulkanAllocator>,
        usage: BufferUsageFlags,
    ) -> Rc<Self> {
        Rc::new(Self {
            device: device.clone(),
            allocator: allocator.clone(),
            buffers: Default::default(),
            usage,
        })
    }

    pub fn for_descriptor_buffer(
        device: &Rc<VulkanDevice>,
        allocator: &Rc<VulkanAllocator>,
        for_sampler: bool,
    ) -> Rc<Self> {
        let mut usage = BufferUsageFlags::SHADER_DEVICE_ADDRESS;
        if for_sampler {
            usage |= BufferUsageFlags::SAMPLER_DESCRIPTOR_BUFFER_EXT;
        } else {
            usage |= BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT;
        }
        Self::new(device, allocator, usage)
    }

    pub fn usage(&self) -> BufferUsageFlags {
        self.usage
    }

    pub fn allocate(
        self: &Rc<Self>,
        capacity: DeviceSize,
        align: DeviceSize,
    ) -> Result<VulkanBuffer, VulkanError> {
        const MIN_ALLOCATION: DeviceSize = 1024;
        let capacity = (capacity.max(MIN_ALLOCATION) + align - 1) & !(align - 1);
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
            return Ok(VulkanBuffer {
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
            let info = BufferCreateInfo::default().size(size).usage(self.usage);
            unsafe {
                self.device
                    .device
                    .create_buffer(&info, None)
                    .map_err(VulkanError::CreateBuffer)?
            }
        };
        let destroy_buffer = OnDrop(|| unsafe { self.device.device.destroy_buffer(buffer, None) });
        let mut memory_requirements =
            unsafe { self.device.device.get_buffer_memory_requirements(buffer) };
        memory_requirements.alignment = memory_requirements.alignment.max(align);
        let allocation = {
            let flags = UsageFlags::UPLOAD
                | UsageFlags::FAST_DEVICE_ACCESS
                | UsageFlags::HOST_ACCESS
                | UsageFlags::DEVICE_ADDRESS;
            self.allocator.alloc(&memory_requirements, flags, true)?
        };
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
        Ok(VulkanBuffer {
            cache: self.clone(),
            buffer: ManuallyDrop::new(VulkanBufferUnused {
                device: self.device.clone(),
                size,
                buffer,
                allocation,
                address,
            }),
        })
    }
}

impl Drop for VulkanBuffer {
    fn drop(&mut self) {
        let buffer = unsafe { ManuallyDrop::take(&mut self.buffer) };
        self.cache.buffers.borrow_mut().push(buffer);
    }
}

impl Drop for VulkanBufferUnused {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_buffer(self.buffer, None);
        }
    }
}
