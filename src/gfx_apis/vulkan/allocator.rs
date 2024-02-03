use {
    crate::{
        gfx_apis::vulkan::{device::VulkanDevice, instance::API_VERSION, VulkanError},
        utils::{numcell::NumCell, ptr_ext::MutPtrExt},
    },
    ash::vk::{DeviceMemory, DeviceSize, MemoryRequirements},
    gpu_alloc::{Config, GpuAllocator, MemoryBlock, Request, UsageFlags},
    gpu_alloc_ash::AshMemoryDevice,
    std::{
        cell::{Cell, UnsafeCell},
        rc::Rc,
    },
};

pub struct VulkanAllocator {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) non_coherent_atom_mask: u64,
    allocator: UnsafeCell<GpuAllocator<DeviceMemory>>,
    total: NumCell<u64>,
}

pub struct VulkanAllocation {
    pub(super) allocator: Rc<VulkanAllocator>,
    pub(super) memory: DeviceMemory,
    pub(super) offset: DeviceSize,
    pub(super) mem: Option<*mut u8>,
    pub(super) size: DeviceSize,
    block: Cell<Option<MemoryBlock<DeviceMemory>>>,
}

impl Drop for VulkanAllocation {
    fn drop(&mut self) {
        unsafe {
            self.allocator.total.fetch_sub(self.size);
            let mut block = self.block.take().unwrap();
            if let Some(_ptr) = self.mem {
                // log::info!("free = {:?} - {:?} ({})", ptr, ptr.add(block.size() as usize), block.size());
                block.unmap(AshMemoryDevice::wrap(&self.allocator.device.device));
            }
            self.allocator
                .allocator
                .get()
                .deref_mut()
                .dealloc(AshMemoryDevice::wrap(&self.allocator.device.device), block);
        }
    }
}

impl VulkanDevice {
    pub fn create_allocator(self: &Rc<Self>) -> Result<Rc<VulkanAllocator>, VulkanError> {
        let config = Config::i_am_prototyping();
        let props = unsafe {
            gpu_alloc_ash::device_properties(
                &self.instance.instance,
                API_VERSION,
                self.physical_device,
            )
        };
        let mut props = props.map_err(VulkanError::GetDeviceProperties)?;
        props.buffer_device_address = false;
        let non_coherent_atom_size = props.non_coherent_atom_size;
        let allocator = GpuAllocator::new(config, props);
        Ok(Rc::new(VulkanAllocator {
            device: self.clone(),
            non_coherent_atom_mask: non_coherent_atom_size - 1,
            allocator: UnsafeCell::new(allocator),
            total: Default::default(),
        }))
    }
}

impl VulkanAllocator {
    fn allocator(&self) -> &mut GpuAllocator<DeviceMemory> {
        unsafe { self.allocator.get().deref_mut() }
    }

    pub fn alloc(
        self: &Rc<Self>,
        req: &MemoryRequirements,
        usage: UsageFlags,
        map: bool,
    ) -> Result<VulkanAllocation, VulkanError> {
        let request = Request {
            size: req.size,
            align_mask: req.alignment - 1,
            usage,
            memory_types: req.memory_type_bits,
        };
        let block = unsafe {
            self.allocator()
                .alloc(AshMemoryDevice::wrap(&self.device.device), request)
        };
        let mut block = block.map_err(VulkanError::AllocateMemory2)?;
        let ptr = match map {
            true => {
                let ptr = unsafe {
                    block.map(
                        AshMemoryDevice::wrap(&self.device.device),
                        0,
                        block.size() as usize,
                    )
                };
                Some(ptr.map_err(VulkanError::MapMemory)?.as_ptr())
            }
            false => None,
        };
        self.total.fetch_add(block.size());
        Ok(VulkanAllocation {
            allocator: self.clone(),
            memory: *block.memory(),
            offset: block.offset(),
            mem: ptr,
            size: block.size(),
            block: Cell::new(Some(block)),
        })
    }
}

impl Drop for VulkanAllocator {
    fn drop(&mut self) {
        unsafe {
            self.allocator
                .get()
                .deref_mut()
                .cleanup(AshMemoryDevice::wrap(&self.device.device));
        }
    }
}
