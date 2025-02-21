use {
    crate::{
        cpu_worker::{AsyncCpuWork, CpuJob, CpuWork, CpuWorker},
        gfx_apis::vulkan::{
            VulkanError, device::VulkanDevice, instance::API_VERSION, renderer::VulkanRenderer,
        },
        utils::{numcell::NumCell, ptr_ext::MutPtrExt},
    },
    ash::{
        Device,
        vk::{DeviceMemory, DeviceSize, MappedMemoryRange, MemoryRequirements},
    },
    gpu_alloc::{Config, GpuAllocator, MemoryBlock, MemoryPropertyFlags, Request, UsageFlags},
    gpu_alloc_ash::AshMemoryDevice,
    parking_lot::Mutex,
    std::{
        cell::{Cell, UnsafeCell},
        rc::Rc,
        sync::Arc,
    },
};

pub struct SyncAllocatorStorage {
    allocator: Arc<Mutex<GpuAllocator<DeviceMemory>>>,
    device: Rc<VulkanDevice>,
}

pub struct UnsyncAllocatorStorage {
    allocator: UnsafeCell<GpuAllocator<DeviceMemory>>,
    device: Rc<VulkanDevice>,
}

pub struct VulkanAllocatorType<T> {
    storage: T,
    non_coherent_atom_mask: u64,
    total: NumCell<u64>,
}

pub type VulkanAllocator = VulkanAllocatorType<UnsyncAllocatorStorage>;
pub type VulkanThreadedAllocator = VulkanAllocatorType<SyncAllocatorStorage>;

enum AllocatorType {
    Local(Rc<VulkanAllocator>),
    Threaded {
        allocator: Rc<VulkanThreadedAllocator>,
        renderer: Rc<VulkanRenderer>,
        cpu: Rc<CpuWorker>,
    },
}

pub struct VulkanAllocation {
    allocator: AllocatorType,
    pub(super) memory: DeviceMemory,
    pub(super) offset: DeviceSize,
    pub(super) mem: Option<*mut u8>,
    pub(super) size: DeviceSize,
    pub(super) coherency_mask: Option<u64>,
    block: Cell<Option<MemoryBlock<DeviceMemory>>>,
}

impl VulkanAllocation {
    unsafe fn free_locally<T>(
        &self,
        allocator: &VulkanAllocatorType<T>,
        device: &VulkanDevice,
        gpu: &mut GpuAllocator<DeviceMemory>,
    ) {
        allocator.total.fetch_sub(self.size);
        let block = self.block.take().unwrap();
        unsafe {
            do_free(gpu, &device.device, block, self.mem);
        }
    }

    pub fn upload<T, F>(&self, f: F) -> Result<T, VulkanError>
    where
        F: FnOnce(*mut u8, usize) -> T,
    {
        let t = f(self.mem.unwrap(), self.size as usize);
        if let Some(mask) = self.coherency_mask {
            let range = self.incoherent_range(mask);
            let res = unsafe { self.device().device.flush_mapped_memory_ranges(&[range]) };
            res.map_err(VulkanError::FlushMemory)?;
        }
        Ok(t)
    }

    pub fn download<T, F>(&self, f: F) -> Result<T, VulkanError>
    where
        F: FnOnce(*const u8, usize) -> T,
    {
        if let Some(mask) = self.coherency_mask {
            let range = self.incoherent_range(mask);
            let res = unsafe {
                self.device()
                    .device
                    .invalidate_mapped_memory_ranges(&[range])
            };
            res.map_err(VulkanError::FlushMemory)?;
        }
        Ok(f(self.mem.unwrap(), self.size as usize))
    }

    fn incoherent_range(&self, mask: u64) -> MappedMemoryRange {
        MappedMemoryRange::default()
            .memory(self.memory)
            .offset(self.offset & !mask)
            .size((self.size + mask) & !mask)
    }

    fn device(&self) -> &VulkanDevice {
        match &self.allocator {
            AllocatorType::Local(l) => &l.storage.device,
            AllocatorType::Threaded { allocator, .. } => &allocator.storage.device,
        }
    }
}

impl Drop for VulkanAllocation {
    fn drop(&mut self) {
        unsafe {
            match &self.allocator {
                AllocatorType::Local(a) => self.free_locally(a, &a.storage.device, a.allocator()),
                AllocatorType::Threaded {
                    allocator,
                    renderer,
                    cpu,
                } => {
                    if renderer.defunct.get() {
                        self.free_locally(
                            allocator,
                            &allocator.storage.device,
                            &mut allocator.storage.allocator.lock(),
                        );
                    } else {
                        let id = renderer.allocate_point();
                        let job = FreeJob {
                            id,
                            renderer: renderer.clone(),
                            allocator: allocator.clone(),
                            size: self.size,
                            work: FreeWork {
                                device: allocator.storage.device.device.clone(),
                                allocator: allocator.storage.allocator.clone(),
                                allocation: Some(UnsafeAllocation {
                                    block: self.block.take().unwrap(),
                                    ptr: self.mem,
                                }),
                            },
                        };
                        let pending = cpu.submit(Box::new(job));
                        renderer.pending_cpu_jobs.set(id, pending);
                    }
                }
            }
        }
    }
}

impl VulkanDevice {
    fn create_allocator_<T>(
        self: &Rc<Self>,
        map: impl FnOnce(GpuAllocator<DeviceMemory>) -> T,
    ) -> Result<Rc<VulkanAllocatorType<T>>, VulkanError> {
        let config = Config::i_am_prototyping();
        let props = unsafe {
            gpu_alloc_ash::device_properties(
                &self.instance.instance,
                API_VERSION,
                self.physical_device,
            )
        };
        let mut props = props.map_err(VulkanError::GetDeviceProperties)?;
        props.buffer_device_address = self.descriptor_buffer.is_some();
        let non_coherent_atom_size = props.non_coherent_atom_size;
        let allocator = GpuAllocator::new(config, props);
        Ok(Rc::new(VulkanAllocatorType {
            non_coherent_atom_mask: non_coherent_atom_size - 1,
            storage: map(allocator),
            total: Default::default(),
        }))
    }

    pub fn create_allocator(self: &Rc<Self>) -> Result<Rc<VulkanAllocator>, VulkanError> {
        self.create_allocator_(|a| UnsyncAllocatorStorage {
            allocator: UnsafeCell::new(a),
            device: self.clone(),
        })
    }

    pub fn create_threaded_allocator(
        self: &Rc<Self>,
    ) -> Result<Rc<VulkanThreadedAllocator>, VulkanError> {
        self.create_allocator_(|a| SyncAllocatorStorage {
            allocator: Arc::new(Mutex::new(a)),
            device: self.clone(),
        })
    }
}

impl<T> VulkanAllocatorType<T> {
    fn commit_allocation(
        self: &Rc<Self>,
        ua: UnsafeAllocation,
        allocator: AllocatorType,
    ) -> VulkanAllocation {
        let UnsafeAllocation { block, ptr } = ua;
        self.total.fetch_add(block.size());
        VulkanAllocation {
            allocator,
            memory: *block.memory(),
            offset: block.offset(),
            mem: ptr,
            size: block.size(),
            coherency_mask: match block.props().contains(MemoryPropertyFlags::HOST_COHERENT) {
                true => None,
                false => Some(self.non_coherent_atom_mask),
            },
            block: Cell::new(Some(block)),
        }
    }
}

impl VulkanAllocator {
    fn allocator(&self) -> &mut GpuAllocator<DeviceMemory> {
        unsafe { self.storage.allocator.get().deref_mut() }
    }

    pub fn alloc(
        self: &Rc<Self>,
        req: &MemoryRequirements,
        usage: UsageFlags,
        map: bool,
    ) -> Result<VulkanAllocation, VulkanError> {
        let ua = do_alloc(
            self.allocator(),
            &self.storage.device.device,
            req,
            usage,
            map,
        )?;
        Ok(self.commit_allocation(ua, AllocatorType::Local(self.clone())))
    }
}

impl VulkanThreadedAllocator {
    pub fn async_alloc(
        self: &Rc<Self>,
        renderer: &Rc<VulkanRenderer>,
        cpu: &Rc<CpuWorker>,
        req: MemoryRequirements,
        usage: UsageFlags,
        map: bool,
        cb: impl FnOnce(Result<VulkanAllocation, VulkanError>) + 'static,
    ) -> Result<(), VulkanError> {
        renderer.check_defunct()?;
        let id = renderer.allocate_point();
        let job = AllocJob {
            id,
            renderer: renderer.clone(),
            cpu: cpu.clone(),
            allocator: self.clone(),
            cb: Some(cb),
            work: AllocWork {
                req,
                usage,
                map,
                device: self.storage.device.device.clone(),
                allocator: self.storage.allocator.clone(),
                res: None,
            },
        };
        let pending = cpu.submit(Box::new(job));
        renderer.pending_cpu_jobs.set(id, pending);
        Ok(())
    }
}

struct AllocJob<T> {
    id: u64,
    renderer: Rc<VulkanRenderer>,
    cpu: Rc<CpuWorker>,
    allocator: Rc<VulkanThreadedAllocator>,
    cb: Option<T>,
    work: AllocWork,
}

struct AllocWork {
    req: MemoryRequirements,
    usage: UsageFlags,
    map: bool,
    device: Arc<Device>,
    allocator: Arc<Mutex<GpuAllocator<DeviceMemory>>>,
    res: Option<Result<UnsafeAllocation, VulkanError>>,
}

impl CpuWork for AllocWork {
    fn run(&mut self) -> Option<Box<dyn AsyncCpuWork>> {
        zone!("AllocWork");
        let r = do_alloc(
            &mut self.allocator.lock(),
            &self.device,
            &self.req,
            self.usage,
            self.map,
        );
        self.res = Some(r);
        None
    }
}

impl<T> CpuJob for AllocJob<T>
where
    T: FnOnce(Result<VulkanAllocation, VulkanError>),
{
    fn work(&mut self) -> &mut dyn CpuWork {
        &mut self.work
    }

    fn completed(mut self: Box<Self>) {
        self.renderer.pending_cpu_jobs.remove(&self.id);
        let res = self.work.res.take().unwrap().map(|ua| {
            self.allocator.commit_allocation(
                ua,
                AllocatorType::Threaded {
                    allocator: self.allocator.clone(),
                    renderer: self.renderer.clone(),
                    cpu: self.cpu.clone(),
                },
            )
        });
        self.cb.take().unwrap()(res);
    }
}

struct FreeJob {
    id: u64,
    renderer: Rc<VulkanRenderer>,
    allocator: Rc<VulkanThreadedAllocator>,
    size: u64,
    work: FreeWork,
}

struct FreeWork {
    device: Arc<Device>,
    allocator: Arc<Mutex<GpuAllocator<DeviceMemory>>>,
    allocation: Option<UnsafeAllocation>,
}

impl CpuWork for FreeWork {
    fn run(&mut self) -> Option<Box<dyn AsyncCpuWork>> {
        zone!("FreeWork");
        let ua = self.allocation.take().unwrap();
        unsafe {
            do_free(&mut self.allocator.lock(), &self.device, ua.block, ua.ptr);
        }
        None
    }
}

impl CpuJob for FreeJob {
    fn work(&mut self) -> &mut dyn CpuWork {
        &mut self.work
    }

    fn completed(self: Box<Self>) {
        self.renderer.pending_cpu_jobs.remove(&self.id);
        self.allocator.total.fetch_sub(self.size);
    }
}

pub struct UnsafeAllocation {
    block: MemoryBlock<DeviceMemory>,
    ptr: Option<*mut u8>,
}

unsafe impl Send for UnsafeAllocation {}

fn do_alloc(
    allocator: &mut GpuAllocator<DeviceMemory>,
    device: &Device,
    req: &MemoryRequirements,
    usage: UsageFlags,
    map: bool,
) -> Result<UnsafeAllocation, VulkanError> {
    let request = Request {
        size: req.size,
        align_mask: req.alignment - 1,
        usage,
        memory_types: req.memory_type_bits,
    };
    let device = AshMemoryDevice::wrap(device);
    let block = unsafe { allocator.alloc(device, request) };
    let mut block = block.map_err(VulkanError::AllocateMemory2)?;
    let ptr = match map {
        true => {
            let ptr = unsafe { block.map(device, 0, block.size() as usize) };
            Some(ptr.map_err(VulkanError::MapMemory)?.as_ptr())
        }
        false => None,
    };
    Ok(UnsafeAllocation { block, ptr })
}

unsafe fn do_free(
    gpu: &mut GpuAllocator<DeviceMemory>,
    device: &Device,
    mut block: MemoryBlock<DeviceMemory>,
    ptr: Option<*mut u8>,
) {
    unsafe {
        let device = AshMemoryDevice::wrap(device);
        if let Some(_ptr) = ptr {
            // log::info!("free = {:?} - {:?} ({})", ptr, ptr.add(block.size() as usize), block.size());
            block.unmap(device);
        }
        gpu.dealloc(device, block);
    }
}

impl Drop for UnsyncAllocatorStorage {
    fn drop(&mut self) {
        let device = AshMemoryDevice::wrap(&self.device.device);
        unsafe {
            self.allocator.get_mut().cleanup(device);
        }
    }
}

impl Drop for SyncAllocatorStorage {
    fn drop(&mut self) {
        let device = AshMemoryDevice::wrap(&self.device.device);
        unsafe {
            self.allocator.lock().cleanup(device);
        }
    }
}
