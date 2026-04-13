use {
    crate::{
        gfx_apis::vulkan::{VulkanError, device::VulkanDevice},
        utils::{errorfmt::ErrorFmt, numcell::NumCell, stack::Stack},
    },
    ash::vk::{
        CommandBuffer, CommandBufferAllocateInfo, CommandBufferLevel, CommandBufferResetFlags,
        CommandPool, CommandPoolCreateFlags, CommandPoolCreateInfo,
    },
    std::rc::Rc,
};

pub struct VulkanCommandPool {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) pool: CommandPool,
}

pub struct VulkanCommandBuffer {
    pub(super) pool: Rc<VulkanCommandPool>,
    pub(super) buffer: CommandBuffer,
}

pub(super) struct CachedCommandBuffers {
    pool: Rc<VulkanCommandPool>,
    buffers: Stack<Rc<VulkanCommandBuffer>>,
    total_buffers: NumCell<usize>,
}

impl Drop for VulkanCommandPool {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_command_pool(self.pool, None);
        }
    }
}

impl Drop for VulkanCommandBuffer {
    fn drop(&mut self) {
        unsafe {
            self.pool
                .device
                .device
                .free_command_buffers(self.pool.pool, &[self.buffer]);
        }
    }
}

impl VulkanCommandPool {
    pub fn allocate_buffer(self: &Rc<Self>) -> Result<Rc<VulkanCommandBuffer>, VulkanError> {
        let create_info = CommandBufferAllocateInfo::default()
            .command_pool(self.pool)
            .command_buffer_count(1)
            .level(CommandBufferLevel::PRIMARY);
        let buffer = unsafe { self.device.device.allocate_command_buffers(&create_info) };
        let mut buffer = buffer.map_err(VulkanError::AllocateCommandBuffer)?;
        assert_eq!(buffer.len(), 1);
        Ok(Rc::new(VulkanCommandBuffer {
            pool: self.clone(),
            buffer: buffer.pop().unwrap(),
        }))
    }
}

impl VulkanDevice {
    pub fn create_command_pool(
        self: &Rc<Self>,
        queue: u32,
    ) -> Result<CachedCommandBuffers, VulkanError> {
        let info = CommandPoolCreateInfo::default()
            .queue_family_index(queue)
            .flags(
                CommandPoolCreateFlags::TRANSIENT | CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            );
        let pool = unsafe { self.device.create_command_pool(&info, None) };
        let pool = pool.map_err(VulkanError::AllocateCommandPool)?;
        Ok(CachedCommandBuffers {
            pool: Rc::new(VulkanCommandPool {
                device: self.clone(),
                pool,
            }),
            buffers: Default::default(),
            total_buffers: Default::default(),
        })
    }
}

impl CachedCommandBuffers {
    pub(super) fn allocate(&self) -> Result<Rc<VulkanCommandBuffer>, VulkanError> {
        zone!("allocate_command_buffer");
        let buf = match self.buffers.pop() {
            Some(b) => b,
            _ => {
                self.total_buffers.fetch_add(1);
                self.pool.allocate_buffer()?
            }
        };
        Ok(buf)
    }

    pub(super) fn release(&self, buffer: Rc<VulkanCommandBuffer>) {
        let res = unsafe {
            buffer
                .pool
                .device
                .device
                .reset_command_buffer(buffer.buffer, CommandBufferResetFlags::empty())
        };
        if let Err(e) = res {
            log::error!("Could not reset command buffer: {}", ErrorFmt(e));
            return;
        }
        self.buffers.push(buffer);
    }
}
