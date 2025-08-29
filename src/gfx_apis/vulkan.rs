mod allocator;
mod blend_buffer;
mod bo_allocator;
mod buffer_cache;
mod command;
mod descriptor;
mod descriptor_buffer;
mod device;
mod fence;
mod format;
mod image;
mod instance;
mod pipeline;
mod renderer;
mod sampler;
mod semaphore;
mod shaders;
mod shm_image;
mod staging;
mod transfer;
mod transfer_functions;

use {
    crate::{
        allocator::{Allocator, AllocatorError},
        async_engine::AsyncEngine,
        cpu_worker::{CpuWorker, jobs::read_write::ReadWriteJobError},
        format::Format,
        gfx_api::{
            AsyncShmGfxTexture, GfxBlendBuffer, GfxContext, GfxError, GfxFormat, GfxImage,
            GfxInternalFramebuffer, GfxStagingBuffer, GfxTexture, ResetStatus, STAGING_DOWNLOAD,
            STAGING_UPLOAD, ShmGfxTexture, StagingBufferUsecase,
        },
        gfx_apis::vulkan::{
            image::VulkanImageMemory, instance::VulkanInstance, renderer::VulkanRenderer,
        },
        io_uring::IoUring,
        pr_caps::PrCapsThread,
        rect::Rect,
        utils::{errorfmt::ErrorFmt, oserror::OsError},
        video::{
            dmabuf::DmaBuf,
            drm::{Drm, DrmError, sync_obj::SyncObjCtx},
            gbm::GbmError,
        },
    },
    ahash::AHashMap,
    ash::{LoadingError, vk},
    gpu_alloc::{AllocationError, MapError},
    jay_config::video::GfxApi,
    log::Level,
    once_cell::sync::Lazy,
    std::{
        cell::Cell,
        ffi::{CStr, CString},
        rc::Rc,
        sync::Arc,
    },
    thiserror::Error,
    uapi::c::dev_t,
};

#[derive(Debug, Error)]
pub enum VulkanError {
    #[error("Could not create a GBM device")]
    Gbm(#[source] GbmError),
    #[error("Could not load libvulkan.so")]
    Load(#[source] Arc<LoadingError>),
    #[error("Could not list instance extensions")]
    InstanceExtensions(#[source] vk::Result),
    #[error("Could not list instance layers")]
    InstanceLayers(#[source] vk::Result),
    #[error("Could not list device extensions")]
    DeviceExtensions(#[source] vk::Result),
    #[error("Could not create the device")]
    CreateDevice(#[source] vk::Result),
    #[error("Could not create a semaphore")]
    CreateSemaphore(#[source] vk::Result),
    #[error("Could not create a fence")]
    CreateFence(#[source] vk::Result),
    #[error("Could not create the buffer")]
    CreateBuffer(#[source] vk::Result),
    #[error("Could not create a shader module")]
    CreateShaderModule(#[source] vk::Result),
    #[error("Missing required instance extension {0:?}")]
    MissingInstanceExtension(&'static CStr),
    #[error("Could not allocate a command pool")]
    AllocateCommandPool(#[source] vk::Result),
    #[error("Could not allocate a command buffer")]
    AllocateCommandBuffer(#[source] vk::Result),
    #[error("Device does not have a graphics queue")]
    NoGraphicsQueue,
    #[error("Missing required device extension {0:?}")]
    MissingDeviceExtension(&'static CStr),
    #[error("Could not create an instance")]
    CreateInstance(#[source] vk::Result),
    #[error("Could not create a debug-utils messenger")]
    Messenger(#[source] vk::Result),
    #[error("Could not enumerate the physical devices")]
    EnumeratePhysicalDevices(#[source] vk::Result),
    #[error("Could not find a vulkan device that matches dev_t {0}")]
    NoDeviceFound(dev_t),
    #[error("Could not load image properties")]
    LoadImageProperties(#[source] vk::Result),
    #[error("Device does not support rending and texturing from the XRGB8888 format")]
    XRGB8888,
    #[error("Device does not support sync file import")]
    SyncFileImport,
    #[error("Could not start a command buffer")]
    BeginCommandBuffer(vk::Result),
    #[error("Could not end a command buffer")]
    EndCommandBuffer(vk::Result),
    #[error("Could not submit a command buffer")]
    Submit(vk::Result),
    #[error("Could not create a sampler")]
    CreateSampler(#[source] vk::Result),
    #[error("Could not create a pipeline layout")]
    CreatePipelineLayout(#[source] vk::Result),
    #[error("Could not create a descriptor set layout")]
    CreateDescriptorSetLayout(#[source] vk::Result),
    #[error("Could not create a pipeline")]
    CreatePipeline(#[source] vk::Result),
    #[error("The format is not supported")]
    FormatNotSupported,
    #[error("The modifier is not supported")]
    ModifierNotSupported,
    #[error("The modifier does not support this use-case")]
    ModifierUseNotSupported,
    #[error("The image has a non-positive size")]
    NonPositiveImageSize,
    #[error("The image is too large")]
    ImageTooLarge,
    #[error("Could not retrieve device properties")]
    GetDeviceProperties(#[source] vk::Result),
    #[error("The dmabuf has an incorrect number of planes")]
    BadPlaneCount,
    #[error("The dmabuf is disjoint but the modifier does not support disjoint buffers")]
    DisjointNotSupported,
    #[error("Could not create the image")]
    CreateImage(#[source] vk::Result),
    #[error("Could not create an image view")]
    CreateImageView(#[source] vk::Result),
    #[error("Could not query the memory fd properties")]
    MemoryFdProperties(#[source] vk::Result),
    #[error("There is no matching memory type")]
    MemoryType,
    #[error("Could not duplicate the DRM fd")]
    Dupfd(#[source] OsError),
    #[error("Could not allocate memory")]
    AllocateMemory(#[source] vk::Result),
    #[error("Could not allocate memory")]
    AllocateMemory2(#[source] AllocationError),
    #[error("Could not bind memory to the image")]
    BindImageMemory(#[source] vk::Result),
    #[error("The format does not support shared memory images")]
    ShmNotSupported,
    #[error("Could not bind memory to the buffer")]
    BindBufferMemory(#[source] vk::Result),
    #[error("Could not map the memory")]
    MapMemory(#[source] MapError),
    #[error("Could not flush modified memory")]
    FlushMemory(#[source] vk::Result),
    #[error("Could not export a sync file from a dma-buf")]
    IoctlExportSyncFile(#[source] OsError),
    #[error("Could not import a sync file into a semaphore")]
    ImportSyncFile(#[source] vk::Result),
    #[error("Could not export a sync file from a semaphore")]
    ExportSyncFile(#[source] vk::Result),
    #[error("Could not fetch the render node of the device")]
    FetchRenderNode(#[source] DrmError),
    #[error("Device has no render node")]
    NoRenderNode,
    #[error("Overflow while calculating shm buffer size")]
    ShmOverflow,
    #[error("Shm stride does not match format or width")]
    InvalidStride,
    #[error("Shm stride and height do not match buffer size")]
    InvalidBufferSize,
    #[error("Buffer format {0} is not supported for shm buffers in Vulkan context")]
    UnsupportedShmFormat(&'static str),
    #[error("Only BO_USE_RENDERING and BO_USE_WRITE are supported")]
    UnsupportedBufferUsage,
    #[error("None of the supplied modifiers are supported")]
    NoSupportedModifiers,
    #[error("Could not retrieve the image modifier")]
    GetModifier(#[source] vk::Result),
    #[error("Vulkan allocated the image with an invalid modifier")]
    InvalidModifier,
    #[error("Could not export the DmaBuf")]
    GetDmaBuf(#[source] vk::Result),
    #[error("Could not wait for the device to become idle")]
    WaitIdle(#[source] vk::Result),
    #[error("Could not dup a DRM device")]
    DupDrm(#[source] DrmError),
    #[error("Graphics context has already been dropped")]
    Defunct,
    #[error("Could not perform an async copy to the staging buffer")]
    AsyncCopyToStaging(#[source] ReadWriteJobError),
    #[error("The async shm texture is busy")]
    AsyncCopyBusy,
    #[error("The staging buffer is busy")]
    StagingBufferBusy,
    #[error("The staging buffer does not support uploads")]
    StagingBufferNoUpload,
    #[error("The staging buffer does not support downloads")]
    StagingBufferNoDownload,
    #[error("Image contents are undefined")]
    UndefinedContents,
    #[error("The framebuffer is being used by the transfer queue")]
    BusyInTransfer,
    #[error("Driver does not support descriptor buffers")]
    NoDescriptorBuffer,
    #[error("A non-vulkan buffer was passed into the vulkan renderer")]
    NonVulkanBuffer,
    #[error("Mixed vulkan device use")]
    MixedVulkanDeviceUse,
}

impl From<VulkanError> for GfxError {
    fn from(value: VulkanError) -> Self {
        Self(Box::new(value))
    }
}

pub static VULKAN_VALIDATION: Lazy<bool> =
    Lazy::new(|| std::env::var("JAY_VULKAN_VALIDATION").ok().as_deref() == Some("1"));

pub fn create_graphics_context(
    eng: &Rc<AsyncEngine>,
    ring: &Rc<IoUring>,
    drm: &Drm,
    caps_thread: Option<&PrCapsThread>,
) -> Result<Rc<dyn GfxContext>, GfxError> {
    let instance = VulkanInstance::new(Level::Info, *VULKAN_VALIDATION)?;
    let device = 'device: {
        if let Some(t) = caps_thread {
            match unsafe { t.run(|| instance.create_device(drm, true)) } {
                Ok(d) => break 'device d,
                Err(e) => {
                    log::warn!("Could not create high-priority device: {}", ErrorFmt(e));
                }
            }
        }
        instance.create_device(drm, false)?
    };
    let renderer = device.create_renderer(eng, ring)?;
    Ok(Rc::new(Context(renderer)))
}

pub fn create_vulkan_allocator(drm: &Drm) -> Result<Rc<dyn Allocator>, AllocatorError> {
    let instance = VulkanInstance::new(Level::Debug, *VULKAN_VALIDATION)?;
    let device = instance.create_device(drm, false)?;
    let allocator = device.create_bo_allocator(drm)?;
    Ok(Rc::new(allocator))
}

#[derive(Debug)]
struct Context(Rc<VulkanRenderer>);

impl GfxContext for Context {
    fn reset_status(&self) -> Option<ResetStatus> {
        self.0.device.lost.get().then_some(ResetStatus::Unknown)
    }

    fn render_node(&self) -> Option<Rc<CString>> {
        Some(self.0.device.render_node.clone())
    }

    fn formats(&self) -> Rc<AHashMap<u32, GfxFormat>> {
        self.0.formats.clone()
    }

    fn dmabuf_img(self: Rc<Self>, buf: &DmaBuf) -> Result<Rc<dyn GfxImage>, GfxError> {
        self.0
            .import_dmabuf(buf)
            .map(|v| v as _)
            .map_err(|e| e.into())
    }

    fn shmem_texture(
        self: Rc<Self>,
        old: Option<Rc<dyn ShmGfxTexture>>,
        data: &[Cell<u8>],
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
        damage: Option<&[Rect]>,
    ) -> Result<Rc<dyn ShmGfxTexture>, GfxError> {
        if let Some(old) = old {
            let old = (old as Rc<dyn GfxTexture>).into_vk(&self.0.device.device)?;
            let shm = match &old.ty {
                VulkanImageMemory::DmaBuf(_) => unreachable!(),
                VulkanImageMemory::Blend(_) => unreachable!(),
                VulkanImageMemory::Internal(shm) => shm,
            };
            if old.width as i32 == width
                && old.height as i32 == height
                && shm.stride as i32 == stride
                && old.format.vk_format == format.vk_format
            {
                shm.upload(&old, data, damage)?;
                return Ok(old);
            }
        }
        let tex = self
            .0
            .create_shm_texture(format, width, height, stride, data, false, None)?;
        Ok(tex as _)
    }

    fn async_shmem_texture(
        self: Rc<Self>,
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
        cpu_worker: &Rc<CpuWorker>,
    ) -> Result<Rc<dyn AsyncShmGfxTexture>, GfxError> {
        let tex = self.0.create_shm_texture(
            format,
            width,
            height,
            stride,
            &[],
            false,
            Some(cpu_worker),
        )?;
        Ok(tex)
    }

    fn allocator(&self) -> Rc<dyn Allocator> {
        self.0.device.gbm.clone()
    }

    fn gfx_api(&self) -> GfxApi {
        GfxApi::Vulkan
    }

    fn create_internal_fb(
        self: Rc<Self>,
        cpu_worker: &Rc<CpuWorker>,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
    ) -> Result<Rc<dyn GfxInternalFramebuffer>, GfxError> {
        let fb = self.0.create_shm_texture(
            format,
            width,
            height,
            stride,
            &[],
            true,
            Some(cpu_worker),
        )?;
        Ok(fb)
    }

    fn sync_obj_ctx(&self) -> Option<&Rc<SyncObjCtx>> {
        Some(&self.0.device.sync_ctx)
    }

    fn create_staging_buffer(
        &self,
        size: usize,
        usecase: StagingBufferUsecase,
    ) -> Rc<dyn GfxStagingBuffer> {
        let upload = usecase.contains(STAGING_UPLOAD);
        let download = usecase.contains(STAGING_DOWNLOAD);
        self.0
            .device
            .create_staging_shell(size as u64, upload, download)
    }

    fn acquire_blend_buffer(
        &self,
        width: i32,
        height: i32,
    ) -> Result<Rc<dyn GfxBlendBuffer>, GfxError> {
        let buffer = self.0.acquire_blend_buffer(width, height)?;
        Ok(buffer)
    }

    fn supports_color_management(&self) -> bool {
        self.0.device.descriptor_buffer.is_some()
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        self.0.on_drop();
    }
}
