use {
    crate::{
        format::Format,
        gfx_api::GfxBuffer,
        gfx_apis::vulkan::{VulkanError, device::VulkanDevice},
    },
    ash::{
        Device,
        vk::{
            self, BufferCreateInfo, BufferUsageFlags, ExternalMemoryBufferCreateInfo,
            ExternalMemoryHandleTypeFlags, ImportMemoryFdInfoKHR, MemoryAllocateInfo,
            MemoryDedicatedAllocateInfo, MemoryFdPropertiesKHR, MemoryPropertyFlags,
        },
    },
    run_on_drop::on_drop,
    std::{any::Any, rc::Rc},
    uapi::OwnedFd,
};

pub(super) const TRANSFER_QUEUE_BUFFER_ALIGNMENT: u64 = 4;

pub struct VulkanDmabufBuffer {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) size: u64,
    pub(super) offset: u64,
    pub(super) buffer: vk::Buffer,
    pub(super) memory: vk::DeviceMemory,
}

impl VulkanDevice {
    pub fn create_dmabuf_buffer(
        self: &Rc<Self>,
        dmabuf: &OwnedFd,
        offset: u64,
        size: u64,
        format: &'static Format,
    ) -> Result<Rc<VulkanDmabufBuffer>, VulkanError> {
        if offset % TRANSFER_QUEUE_BUFFER_ALIGNMENT != 0 || offset % format.bpp as u64 != 0 {
            return Err(VulkanError::DmaBufBufferOffsetAlignment);
        }
        let mut memory_fd_properties = MemoryFdPropertiesKHR::default();
        unsafe {
            self.external_memory_fd
                .get_memory_fd_properties(
                    ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
                    dmabuf.raw(),
                    &mut memory_fd_properties,
                )
                .map_err(VulkanError::MemoryFdProperties)?
        }
        let buffer = {
            let mut external_info = ExternalMemoryBufferCreateInfo::default()
                .handle_types(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let create_info = BufferCreateInfo::default()
                .size(size)
                .usage(BufferUsageFlags::TRANSFER_SRC)
                .push_next(&mut external_info);
            unsafe {
                self.device
                    .create_buffer(&create_info, None)
                    .map_err(VulkanError::CreateBuffer)?
            }
        };
        let destroy_buffer = on_drop(|| unsafe { self.device.destroy_buffer(buffer, None) });
        let requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        let memory_type = self.find_memory_type(
            MemoryPropertyFlags::HOST_VISIBLE,
            requirements.memory_type_bits & memory_fd_properties.memory_type_bits,
        );
        let Some(memory_type) = memory_type else {
            return Err(VulkanError::MemoryType);
        };
        let fd =
            uapi::fcntl_dupfd_cloexec(dmabuf.raw(), 0).map_err(|e| VulkanError::Dupfd(e.into()))?;
        let memory = {
            let mut dedicated = MemoryDedicatedAllocateInfo::default().buffer(buffer);
            let mut import_info = ImportMemoryFdInfoKHR::default()
                .fd(fd.raw())
                .handle_type(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let allocate_info = MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(memory_type)
                .push_next(&mut import_info)
                .push_next(&mut dedicated);
            unsafe {
                self.device
                    .allocate_memory(&allocate_info, None)
                    .map_err(VulkanError::AllocateMemory)?
            }
        };
        fd.unwrap();
        let free_memory = on_drop(|| unsafe { self.device.free_memory(memory, None) });
        unsafe {
            self.device
                .bind_buffer_memory(buffer, memory, 0)
                .map_err(VulkanError::BindBufferMemory)?;
        }
        free_memory.forget();
        destroy_buffer.forget();
        Ok(Rc::new(VulkanDmabufBuffer {
            device: self.clone(),
            size,
            offset,
            buffer,
            memory,
        }))
    }
}

impl Drop for VulkanDmabufBuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.device.free_memory(self.memory, None);
            self.device.device.destroy_buffer(self.buffer, None);
        }
    }
}

impl VulkanDmabufBuffer {
    fn assert_device(&self, device: &Device) {
        assert_eq!(
            self.device.device.handle(),
            device.handle(),
            "Mixed vulkan device use"
        );
    }
}

impl GfxBuffer for VulkanDmabufBuffer {}

impl dyn GfxBuffer {
    pub(super) fn into_vk(self: Rc<Self>, device: &Device) -> Rc<VulkanDmabufBuffer> {
        let buffer: Rc<VulkanDmabufBuffer> = (self as Rc<dyn Any>)
            .downcast()
            .expect("Non-vulkan buffer passed into vulkan");
        buffer.assert_device(device);
        buffer
    }
}
