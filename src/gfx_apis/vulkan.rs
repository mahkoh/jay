mod allocator;
mod alpha_modes;
mod blend_buffer;
mod bo_allocator;
mod buffer_cache;
mod command;
mod descriptor;
mod descriptor_buffer;
mod descriptor_heap;
mod device;
mod dmabuf_buffer;
mod eotfs;
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

use crate::allocator::Allocator;
use crate::allocator::AllocatorError;
use crate::async_engine::AsyncEngine;
use crate::backend::DrmDeviceId;
use crate::cpu_worker::CpuWorker;
use crate::cpu_worker::jobs::read_write::ReadWriteJobError;
use crate::eventfd_cache::EventfdCache;
use crate::format::Format;
use crate::gfx_api::AsyncShmGfxTexture;
use crate::gfx_api::GfxApi;
use crate::gfx_api::GfxBlendBuffer;
use crate::gfx_api::GfxBuffer;
use crate::gfx_api::GfxContext;
use crate::gfx_api::GfxError;
use crate::gfx_api::GfxFormat;
use crate::gfx_api::GfxFramebuffer;
use crate::gfx_api::GfxInternalFramebuffer;
use crate::gfx_api::GfxStagingBuffer;
use crate::gfx_api::GfxTexture;
use crate::gfx_api::ResetStatus;
use crate::gfx_api::STAGING_DOWNLOAD;
use crate::gfx_api::STAGING_UPLOAD;
use crate::gfx_api::ShmGfxTexture;
use crate::gfx_api::StagingBufferUsecase;
use crate::gfx_apis::vulkan::device::VulkanDevice;
use crate::gfx_apis::vulkan::image::VulkanImageMemory;
use crate::gfx_apis::vulkan::instance::VulkanInstance;
use crate::gfx_apis::vulkan::renderer::VulkanRenderer;
use crate::io_uring::IoUring;
use crate::pr_caps::PrCapsThread;
use crate::rect::Rect;
use crate::syncobj::SyncobjCtx;
use crate::utils::bhash::BHashMap;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::oserror::OsError;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmaBufIds;
use crate::video::drm::Drm;
use crate::video::drm::DrmError;
use crate::video::gbm::GbmError;
use crate::vulkan_core::VulkanCoreError;
use crate::vulkan_core::{self};
use ash::vk;
use gpu_alloc::AllocationError;
use gpu_alloc::MapError;
use log::Level;
use std::cell::Cell;
use std::ffi::CStr;
use std::ffi::CString;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c::dev_t;

#[derive(Debug, Error)]
pub enum VulkanError {
    #[error(transparent)]
    Core(#[from] VulkanCoreError),
    #[error("Could not create a GBM device")]
    Gbm(#[source] GbmError),
    #[error("Could not list device extensions")]
    DeviceExtensions(#[source] vk::Result),
    #[error("Could not create the device")]
    CreateDevice(#[source] vk::Result),
    #[error("Could not create a semaphore")]
    CreateSemaphore(#[source] vk::Result),
    #[error("Could not create the buffer")]
    CreateBuffer(#[source] vk::Result),
    #[error("Could not create a shader module")]
    CreateShaderModule(#[source] vk::Result),
    #[error("Could not allocate a command pool")]
    AllocateCommandPool(#[source] vk::Result),
    #[error("Could not allocate a command buffer")]
    AllocateCommandBuffer(#[source] vk::Result),
    #[error("Device does not have a graphics queue")]
    NoGraphicsQueue,
    #[error("Missing required device extension {0:?}")]
    MissingDeviceExtension(&'static CStr),
    #[error("Could not enumerate the physical devices")]
    EnumeratePhysicalDevices(#[source] vk::Result),
    #[error("Could not find a vulkan device that matches dev_t {0}")]
    NoDeviceFound(dev_t),
    #[error("There is no vulkan software renderer")]
    NoSoftwareRenderer,
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
    #[error("The format does not support read-write images")]
    RwNotSupported,
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
    #[error("Driver does not support descriptor buffers/heaps")]
    NoBlendBuffers,
    #[error("A non-vulkan buffer was passed into the vulkan renderer")]
    NonVulkanBuffer,
    #[error("Mixed vulkan device use")]
    MixedVulkanDeviceUse,
    #[error("Could not allocate GBM BO")]
    AllocGbm(#[source] GbmError),
    #[error("Could not retrieve file description flags")]
    GetFl(#[source] OsError),
    #[error("GBM implementation cannot be used with software renderer")]
    SoftwareRendererNotUsable,
    #[error("DMABUF buffer offsets must be aligned to 4 bytes and the pixel size")]
    DmaBufBufferOffsetAlignment,
    #[error("Framebuffer has no image view")]
    FbNoImageView,
    #[error("Blend buffer has no image view")]
    BbNoImageView,
    #[error("Could not write descriptor")]
    WriteDescriptor(#[source] vk::Result),
    #[error("Requested heap size exceeds maximum size")]
    MaximumHeapSize,
}

type VulkanSync = vulkan_core::sync::VulkanSync<VulkanDevice>;
type VulkanTimelineSemaphore =
    vulkan_core::timeline_semaphore::VulkanTimelineSemaphore<VulkanDevice>;

impl From<VulkanError> for GfxError {
    fn from(value: VulkanError) -> Self {
        Self(Box::new(value))
    }
}

pub fn create_graphics_context(
    eng: &Rc<AsyncEngine>,
    ring: &Rc<IoUring>,
    eventfd_cache: &Rc<EventfdCache>,
    drm_device_id: Option<DrmDeviceId>,
    drm: &Drm,
    caps_thread: Option<&PrCapsThread>,
    software: bool,
) -> Result<Rc<dyn GfxContext>, GfxError> {
    let instance = VulkanInstance::new(Level::Info)?;
    let device = 'device: {
        if let Some(t) = caps_thread {
            match unsafe {
                t.run(|| instance.create_device(drm_device_id, drm, eventfd_cache, true, software))
            } {
                Ok(d) => break 'device d,
                Err(e) => {
                    log::warn!("Could not create high-priority device: {}", ErrorFmt(e));
                }
            }
        }
        instance.create_device(drm_device_id, drm, eventfd_cache, false, software)?
    };
    let renderer = device.create_renderer(eng, ring)?;
    Ok(Rc::new(Context(renderer)))
}

pub fn create_vulkan_allocator(
    drm: &Drm,
    eventfd_cache: &Rc<EventfdCache>,
) -> Result<Rc<dyn Allocator>, AllocatorError> {
    let instance = VulkanInstance::new(Level::Debug)?;
    let device = instance.create_device(None, drm, eventfd_cache, false, false)?;
    let allocator = device.create_bo_allocator(drm)?;
    Ok(Rc::new(allocator))
}

#[derive(Debug)]
struct Context(Rc<VulkanRenderer>);

impl GfxContext for Context {
    fn reset_status(&self) -> Option<ResetStatus> {
        self.0.device.lost.get().then_some(ResetStatus::Unknown)
    }

    fn drm_device_id(&self) -> Option<DrmDeviceId> {
        self.0.device.drm_device_id
    }

    fn render_node(&self) -> Option<Rc<CString>> {
        Some(self.0.device.render_node.clone())
    }

    fn formats(&self) -> &Rc<BHashMap<u32, GfxFormat>> {
        &self.0.formats
    }

    fn fast_ram_access(&self) -> bool {
        self.0.device.fast_ram_access
    }

    fn dmabuf_fb(self: Rc<Self>, buf: &Rc<DmaBuf>) -> Result<Rc<dyn GfxFramebuffer>, GfxError> {
        self.0
            .import_dmabuf(buf)?
            .create_framebuffer()
            .map(|v| v as _)
            .map_err(|e| e.into())
    }

    fn dmabuf_tex(self: Rc<Self>, buf: &Rc<DmaBuf>) -> Result<Rc<dyn GfxTexture>, GfxError> {
        self.0
            .import_dmabuf(buf)?
            .create_texture()
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
                VulkanImageMemory::Rw(_) => unreachable!(),
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

    fn create_read_write_img(
        self: Rc<Self>,
        _dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
    ) -> Result<(Rc<dyn GfxFramebuffer>, Rc<dyn GfxTexture>), GfxError> {
        let img = self.0.create_rw_image(format, width, height)?;
        Ok((img.clone(), img))
    }

    fn syncobj_ctx(&self) -> Option<&Rc<SyncobjCtx>> {
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
        self.0.device.uses_descriptor_memory()
    }

    fn supports_alpha_modes(&self) -> bool {
        self.0.device.uses_descriptor_memory()
    }

    fn create_dmabuf_buffer(
        &self,
        dmabuf: &OwnedFd,
        offset: usize,
        size: usize,
        format: &'static Format,
    ) -> Result<Rc<dyn GfxBuffer>, GfxError> {
        self.0.check_defunct()?;
        let buffer =
            self.0
                .device
                .create_dmabuf_buffer(dmabuf, offset as u64, size as u64, format)?;
        Ok(buffer)
    }

    fn supports_wait_sync(&self) -> bool {
        true
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        self.0.on_drop();
    }
}
