use {
    crate::{
        gfx_apis::vulkan::{
            VulkanError,
            allocator::VulkanAllocator,
            buffer_cache::VulkanBufferUncached,
            device::{DescriptorHeapDevice, VulkanDevice},
        },
        utils::page_alloc::{PAGE_ALLOC_PAGE_SIZE, PageAlloc, PageAllocEntry},
    },
    ash::vk::{
        BindHeapInfoEXT, BufferUsageFlags, CommandBuffer, DeviceAddressRangeEXT, DeviceSize,
    },
    linearize::Linearize,
    std::rc::Rc,
};

pub struct DescriptorHeap {
    ty: DescriptorHeapType,
    page_alloc: Rc<PageAlloc>,
    buffer: VulkanBufferUncached,
    size: usize,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Linearize)]
pub enum DescriptorHeapType {
    Sampler,
    Resource,
}

impl VulkanDevice {
    pub fn allocate_descriptor_heap(
        self: &Rc<Self>,
        dh: &DescriptorHeapDevice,
        allocator: &Rc<VulkanAllocator>,
        size: usize,
        ty: DescriptorHeapType,
    ) -> Result<Rc<DescriptorHeap>, VulkanError> {
        let size = size.next_multiple_of(PAGE_ALLOC_PAGE_SIZE as usize);
        let full_size = size as DeviceSize + dh.min_reserved_range[ty];
        if full_size > dh.max_size[ty] {
            return Err(VulkanError::MaximumHeapSize);
        }
        let buffer = self.allocate_uncached_buffer(
            full_size,
            (PAGE_ALLOC_PAGE_SIZE as DeviceSize).max(dh.alignment[ty]),
            allocator,
            BufferUsageFlags::DESCRIPTOR_HEAP_EXT | BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        )?;
        let pages = size / PAGE_ALLOC_PAGE_SIZE as usize;
        Ok(Rc::new(DescriptorHeap {
            ty,
            size,
            page_alloc: dh.alloc.create_alloc(pages),
            buffer,
        }))
    }
}

impl DescriptorHeap {
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn not_contains(&self, entry: &PageAllocEntry) -> bool {
        entry.is_not_in_alloc(&self.page_alloc)
    }

    pub fn bind(&self, dh: &DescriptorHeapDevice, buf: CommandBuffer) {
        let info = BindHeapInfoEXT::default()
            .heap_range(DeviceAddressRangeEXT {
                address: self.buffer.address,
                size: self.buffer.size,
            })
            .reserved_range_offset(self.size as _)
            .reserved_range_size(dh.min_reserved_range[self.ty]);
        unsafe {
            match self.ty {
                DescriptorHeapType::Sampler => dh.device.cmd_bind_sampler_heap(buf, &info),
                DescriptorHeapType::Resource => dh.device.cmd_bind_resource_heap(buf, &info),
            }
        }
    }

    pub fn allocate(
        self: &Rc<Self>,
        data: &[u8],
    ) -> Result<Option<Rc<PageAllocEntry>>, VulkanError> {
        let Some(res) = self.page_alloc.allocate(data.len()) else {
            return Ok(None);
        };
        unsafe {
            self.buffer
                .allocation
                .upload_range(res.offset() as usize, data.len(), |p| {
                    std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
                })?;
        }
        Ok(Some(res))
    }
}
