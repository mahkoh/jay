use {
    crate::gfx_apis::vulkan::{
        VulkanError,
        allocator::{VulkanAllocation, VulkanAllocator},
        device::VulkanDevice,
    },
    ash::vk::{
        Buffer, BufferCreateInfo, BufferDeviceAddressInfo, BufferUsageFlags, DeviceAddress,
        DeviceSize,
    },
    gpu_alloc::UsageFlags,
    run_on_drop::on_drop,
    std::{cell::RefCell, mem::ManuallyDrop, ops::Deref, rc::Rc},
    uapi::Packed,
};

pub struct VulkanBufferCache {
    device: Rc<VulkanDevice>,
    allocator: Rc<VulkanAllocator>,
    buffers: RefCell<Vec<VulkanBufferUnused>>,
    usage: BufferUsageFlags,
    min_alignment: DeviceSize,
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
        min_alignment: DeviceSize,
    ) -> Rc<Self> {
        Rc::new(Self {
            device: device.clone(),
            allocator: allocator.clone(),
            buffers: Default::default(),
            usage,
            min_alignment,
        })
    }

    pub fn for_descriptor_buffer(
        device: &Rc<VulkanDevice>,
        allocator: &Rc<VulkanAllocator>,
        for_sampler: bool,
    ) -> Rc<Self> {
        let mut usage = BufferUsageFlags::SHADER_DEVICE_ADDRESS;
        let mut min_alignment = 1;
        if for_sampler {
            usage |= BufferUsageFlags::SAMPLER_DESCRIPTOR_BUFFER_EXT;
        } else {
            usage |= BufferUsageFlags::RESOURCE_DESCRIPTOR_BUFFER_EXT;
            if device.is_anv {
                // https://gitlab.freedesktop.org/mesa/mesa/-/merge_requests/33903
                min_alignment = 4096;
            }
        }
        Self::new(device, allocator, usage, min_alignment)
    }

    pub fn usage(&self) -> BufferUsageFlags {
        self.usage
    }

    pub fn allocate(self: &Rc<Self>, capacity: DeviceSize) -> Result<VulkanBuffer, VulkanError> {
        const MIN_ALLOCATION: DeviceSize = 1024;
        let align_mask = self.min_alignment - 1;
        let capacity = (capacity.max(MIN_ALLOCATION) + align_mask) & !align_mask;
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
        let destroy_buffer = on_drop(|| unsafe { self.device.device.destroy_buffer(buffer, None) });
        let mut memory_requirements =
            unsafe { self.device.device.get_buffer_memory_requirements(buffer) };
        memory_requirements.alignment = memory_requirements.alignment.max(self.min_alignment);
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

#[derive(Default)]
pub struct GenericBufferWriter {
    buf: Vec<u8>,
}

impl GenericBufferWriter {
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn write(&mut self, offset_mask: DeviceSize, data: &(impl Packed + ?Sized)) -> DeviceSize {
        let mut offset = self.buf.len() as DeviceSize;
        let mask = offset_mask | (align_of_val(data) as DeviceSize - 1);
        offset = (offset + mask) & !mask;
        self.buf.resize(offset as usize, 0);
        self.buf.extend_from_slice(uapi::as_bytes(data));
        offset
    }
}

impl Deref for GenericBufferWriter {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.buf
    }
}
