use {
    crate::{
        cpu_worker::CpuWorker,
        gfx_api::GfxStagingBuffer,
        gfx_apis::vulkan::{
            VulkanError,
            allocator::{VulkanAllocation, VulkanAllocator},
            device::VulkanDevice,
            renderer::VulkanRenderer,
        },
        utils::clonecell::CloneCell,
    },
    ash::{
        Device,
        vk::{Buffer, BufferCreateInfo, BufferUsageFlags},
    },
    gpu_alloc::UsageFlags,
    run_on_drop::on_drop,
    std::{any::Any, cell::Cell, rc::Rc},
};

pub struct VulkanStagingBuffer {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) allocation: VulkanAllocation,
    pub(super) buffer: Buffer,
    pub(super) size: u64,
}

impl VulkanDevice {
    pub(super) fn create_staging_shell(
        self: &Rc<Self>,
        size: u64,
        upload: bool,
        download: bool,
    ) -> Rc<VulkanStagingShell> {
        Rc::new(VulkanStagingShell {
            device: self.clone(),
            staging: Default::default(),
            size,
            download,
            upload,
            busy: Cell::new(false),
        })
    }

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
        let destroy_buffer = on_drop(|| unsafe { self.device.destroy_buffer(buffer, None) });
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

    pub(super) fn fill_staging_shell(
        self: &Rc<Self>,
        renderer: &Rc<VulkanRenderer>,
        cpu: &Rc<CpuWorker>,
        shell: Rc<VulkanStagingShell>,
        cb: impl FnOnce(Result<Rc<VulkanStagingBuffer>, VulkanError>) + 'static,
    ) -> Result<(), VulkanError> {
        let (vk_usage, usage) = get_usage(shell.upload, shell.download, false);
        let buffer = self.create_buffer(shell.size, vk_usage)?;
        let memory_requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        let slf = self.clone();
        let destroy_buffer = on_drop(move || unsafe { slf.device.destroy_buffer(buffer, None) });
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
            let buffer = Rc::new(VulkanStagingBuffer {
                device: slf.clone(),
                allocation,
                buffer,
                size: shell.size,
            });
            shell.staging.set(Some(buffer.clone()));
            Ok(buffer)
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
        self.allocation.upload(f)
    }

    pub fn download<T, F>(&self, f: F) -> Result<T, VulkanError>
    where
        F: FnOnce(*const u8, usize) -> T,
    {
        self.allocation.download(f)
    }
}

impl Drop for VulkanStagingBuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.device.destroy_buffer(self.buffer, None);
        }
    }
}

pub(super) struct VulkanStagingShell {
    pub(super) device: Rc<VulkanDevice>,
    pub(super) staging: CloneCell<Option<Rc<VulkanStagingBuffer>>>,
    pub(super) size: u64,
    pub(super) download: bool,
    pub(super) upload: bool,
    pub(super) busy: Cell<bool>,
}

impl GfxStagingBuffer for VulkanStagingShell {
    fn size(&self) -> usize {
        self.size as _
    }
}

impl VulkanStagingShell {
    fn assert_device(&self, device: &Device) {
        assert_eq!(
            self.device.device.handle(),
            device.handle(),
            "Mixed vulkan device use"
        );
    }
}

impl dyn GfxStagingBuffer {
    pub(super) fn into_vk(self: Rc<Self>, device: &Device) -> Rc<VulkanStagingShell> {
        let shell: Rc<VulkanStagingShell> = (self as Rc<dyn Any>)
            .downcast()
            .expect("Non-vulkan staging buffer passed into vulkan");
        shell.assert_device(device);
        shell
    }
}
