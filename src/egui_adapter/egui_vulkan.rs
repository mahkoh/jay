use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        format::XRGB8888,
        gfx_api::SyncFile,
        io_uring::IoUring,
        utils::{errorfmt::ErrorFmt, numcell::NumCell, queue::AsyncQueue},
        video::{
            Modifier,
            dmabuf::{DmaBuf, PlaneVec},
        },
        vulkan_core::{
            VULKAN_API_VERSION, VulkanCoreError, VulkanCoreInstance,
            gpu_alloc_ash::AshMemoryDevice, map_extension_properties,
        },
    },
    ahash::AHashMap,
    arrayvec::ArrayVec,
    ash::{
        Device,
        ext::{
            external_memory_dma_buf, image_drm_format_modifier, physical_device_drm,
            queue_family_foreign,
        },
        khr::{external_fence_fd, external_memory_fd, external_semaphore_fd, push_descriptor},
        util::read_spv,
        vk::{
            self, AccessFlags2, AttachmentLoadOp, AttachmentStoreOp, BindImageMemoryInfo,
            BindImagePlaneMemoryInfo, BlendFactor, BlendOp, BorderColor, Buffer, BufferCreateInfo,
            BufferImageCopy2, BufferMemoryBarrier2, BufferUsageFlags, ColorComponentFlags,
            CommandBuffer, CommandBufferAllocateInfo, CommandBufferBeginInfo, CommandBufferLevel,
            CommandBufferSubmitInfo, CommandBufferUsageFlags, CommandPool, CommandPoolCreateFlags,
            CommandPoolCreateInfo, ComponentMapping, ComponentSwizzle, CopyBufferToImageInfo2,
            CullModeFlags, DependencyInfo, DescriptorImageInfo, DescriptorSetLayout,
            DescriptorSetLayoutBinding, DescriptorSetLayoutCreateFlags,
            DescriptorSetLayoutCreateInfo, DescriptorType, DeviceCreateInfo, DeviceMemory,
            DeviceQueueCreateInfo, DrmFormatModifierPropertiesEXT,
            DrmFormatModifierPropertiesListEXT, DynamicState, ExportFenceCreateInfo, Extent2D,
            Extent3D, ExternalFenceFeatureFlags, ExternalFenceHandleTypeFlags,
            ExternalFenceProperties, ExternalImageFormatPropertiesKHR, ExternalMemoryFeatureFlags,
            ExternalMemoryHandleTypeFlags, ExternalMemoryImageCreateInfo,
            ExternalSemaphoreFeatureFlags, ExternalSemaphoreHandleTypeFlags,
            ExternalSemaphoreProperties, Fence, FenceCreateInfo, FenceGetFdInfoKHR, Filter, Format,
            FormatFeatureFlags, FormatProperties2, FrontFace, GraphicsPipelineCreateInfo, Image,
            ImageAspectFlags, ImageCreateFlags, ImageCreateInfo,
            ImageDrmFormatModifierExplicitCreateInfoEXT, ImageFormatProperties2, ImageLayout,
            ImageMemoryBarrier2, ImageMemoryRequirementsInfo2, ImagePlaneMemoryRequirementsInfo,
            ImageSubresourceLayers, ImageSubresourceRange, ImageTiling, ImageType, ImageUsageFlags,
            ImageView, ImageViewCreateInfo, ImageViewType, ImportMemoryFdInfoKHR,
            ImportSemaphoreFdInfoKHR, IndexType, MappedMemoryRange, MemoryAllocateInfo,
            MemoryDedicatedAllocateInfo, MemoryFdPropertiesKHR, MemoryRequirements,
            MemoryRequirements2, Offset2D, Offset3D, PhysicalDeviceDrmPropertiesEXT,
            PhysicalDeviceDynamicRenderingFeatures, PhysicalDeviceExternalFenceInfo,
            PhysicalDeviceExternalImageFormatInfoKHR, PhysicalDeviceExternalSemaphoreInfo,
            PhysicalDeviceFeatures2, PhysicalDeviceImageDrmFormatModifierInfoEXT,
            PhysicalDeviceImageFormatInfo2, PhysicalDeviceProperties2,
            PhysicalDeviceSynchronization2Features, PhysicalDeviceType,
            PhysicalDeviceVulkan13Properties, Pipeline, PipelineBindPoint, PipelineCache,
            PipelineColorBlendAttachmentState, PipelineColorBlendStateCreateInfo,
            PipelineDynamicStateCreateInfo, PipelineInputAssemblyStateCreateInfo, PipelineLayout,
            PipelineLayoutCreateInfo, PipelineMultisampleStateCreateInfo,
            PipelineRasterizationStateCreateInfo, PipelineRenderingCreateInfo,
            PipelineShaderStageCreateInfo, PipelineStageFlags2, PipelineVertexInputStateCreateInfo,
            PipelineViewportStateCreateInfo, PolygonMode, PrimitiveTopology,
            QUEUE_FAMILY_FOREIGN_EXT, Queue, QueueFlags, Rect2D, RenderingAttachmentInfo,
            RenderingInfoKHR, SampleCountFlags, Sampler, SamplerAddressMode, SamplerCreateInfo,
            SamplerMipmapMode, Semaphore, SemaphoreCreateInfo, SemaphoreImportFlags,
            SemaphoreSubmitInfo, ShaderModule, ShaderModuleCreateInfo, ShaderStageFlags,
            SharingMode, SubmitInfo2, SubresourceLayout, VertexInputAttributeDescription,
            VertexInputBindingDescription, VertexInputRate, Viewport, WHOLE_SIZE,
            WriteDescriptorSet,
        },
    },
    bstr::ByteSlice,
    egui::epaint::{
        ClippedPrimitive, ImageData, ImageDelta, Primitive, TextureId, Vertex,
        textures::{TextureFilter, TextureOptions, TextureWrapMode, TexturesDelta},
    },
    gpu_alloc::{
        AllocationError, Config, GpuAllocator, MapError, MemoryBlock, Request, UsageFlags,
    },
    gpu_alloc_types::MemoryPropertyFlags,
    isnt::std_1::{collections::IsntHashMapExt, primitive::IsntSliceExt},
    log::Level,
    run_on_drop::on_drop,
    std::{
        cell::{Cell, RefCell},
        collections::hash_map::Entry,
        ffi::CStr,
        io::{self, Cursor},
        mem::{ManuallyDrop, offset_of},
        ptr,
        rc::Rc,
        slice,
    },
    thiserror::Error,
    uapi::{AsUstr, AssertPacked, OwnedFd, Packed, Pod, c},
};

#[derive(Debug, Error)]
pub enum EgvError {
    #[error(transparent)]
    Core(#[from] VulkanCoreError),
    #[error("could not read spv source")]
    ReadSpv(#[source] io::Error),
    #[error("could not create a shader module")]
    CreateShaderModule(#[source] vk::Result),
    #[error("could not create a sampler")]
    CreateSampler(#[source] vk::Result),
    #[error("could not allocate GPU memory")]
    AllocateMemory(#[source] AllocationError),
    #[error("could not map GPU memory")]
    MapMemory(#[source] MapError),
    #[error("could not create a buffer")]
    CreateBuffer(#[source] vk::Result),
    #[error("could not bind memory to buffer")]
    BindBufferMemory(#[source] vk::Result),
    #[error("could not bind memory to image")]
    BindImageMemory(#[source] vk::Result),
    #[error("could not flush GPU memory")]
    FlushMemory(#[source] vk::Result),
    #[error("could not create an image")]
    CreateImage(#[source] vk::Result),
    #[error("could not create an image view")]
    CreateImageView(#[source] vk::Result),
    #[error("tried to render an unknown texture {0:?}")]
    UnknownTexture(TextureId),
    #[error("could not create a descriptor set layout")]
    CreateDescriptorSetLayout(#[source] vk::Result),
    #[error("could not create a pipeline layout")]
    CreatePipelineLayout(#[source] vk::Result),
    #[error("could not create a pipeline")]
    CreatePipeline(#[source] vk::Result),
    #[error("cannot perform a partial update of unknown texture {0:?}")]
    PartialTextureUpdateForUnknownTexture(TextureId),
    #[error("cannot perform out-of-bounds texture update for {0:?}")]
    TextureUpdateOutOfBounds(TextureId),
    #[error("could not allocate a command buffer")]
    AllocateCommandBuffer(#[source] vk::Result),
    #[error("could not begin a command buffer")]
    BeginCommandBuffer(#[source] vk::Result),
    #[error("could not end a command buffer")]
    EndCommandBuffer(#[source] vk::Result),
    #[error("could not create a fence")]
    CreateFence(#[source] vk::Result),
    #[error("could not create a semaphore")]
    CreateSemaphore(#[source] vk::Result),
    #[error("could not submit a command buffer")]
    Submit(#[source] vk::Result),
    #[error("could not get device properties")]
    GetDeviceProperties(#[source] vk::Result),
    #[error("could not create a command pool")]
    CreateCommandPool(#[source] vk::Result),
    #[error("driver does not support all required format features")]
    MissingFormatFeatures,
    #[error("could not get image format properties")]
    GetImageFormatProperties(#[source] vk::Result),
    #[error("texture is empty")]
    EmptyImage,
    #[error("texture is too large")]
    TexTooLarge,
    #[error("driver does not support sufficiently-large buffers")]
    BufferTooLarge,
    #[error("Could not enumerate the physical devices")]
    EnumeratePhysicalDevice(#[source] vk::Result),
    #[error("Could not find a corresponding vulkan device")]
    NoVulkanDevice,
    #[error("Device does not support vulkan 1.3")]
    NoVulkan13,
    #[error("Device does not support the synchronization2 feature")]
    NoSynchronization2,
    #[error("Device does not support the dynamic rendering feature")]
    NoDynamicRendering,
    #[error("Device does not support the device extension {}", .0.as_ustr().as_bytes().as_bstr())]
    MissingDeviceExtensions(&'static CStr),
    #[error("Device does not support importing sync files")]
    NoSyncFileImport,
    #[error("Device does not support exporting sync files")]
    NoSyncFileExport,
    #[error("Device does not have a graphics queue family")]
    NoGfxQueueFamily,
    #[error("Could not create the device")]
    CreateDevice(#[source] vk::Result),
    #[error("Only XRGB8888 is supported as the framebuffer format")]
    WrongFbFormat,
    #[error("The size of FB must be > 0")]
    NonPositiveFbSize,
    #[error("The modifier is not supported")]
    UnsupportedModifier,
    #[error("The number of planes is incorrect")]
    WrongPlaneCount,
    #[error("The FB is too large")]
    TooLarge,
    #[error("Could not query memory fd properties")]
    GetMemoryFdProperties(#[source] vk::Result),
    #[error("Could not find a memory type for import")]
    NoMemoryTypeForImport,
    #[error("Could not dup a dma buf")]
    DupDmaBuf(#[source] io::Error),
    #[error("Could not import memory")]
    ImportMemory(#[source] vk::Result),
    #[error("Could not dup a sync file")]
    DupSyncFile(#[source] io::Error),
    #[error("Could not import a sync file")]
    ImportSyncFile(#[source] vk::Result),
    #[error("Could not export a sync file")]
    ExportSyncFile(#[source] vk::Result),
}

pub struct EgvRenderer {
    ri: Rc<EgvRendererInner>,
    _task: SpawnedFuture<()>,
}

pub struct EgvContext {
    renderer: Rc<EgvRenderer>,
    id: u64,
}

pub struct EgvFramebuffer {
    renderer: Rc<EgvRenderer>,
    ctx: Rc<EgvContext>,
    image: Rc<EgvImage<EgvImportedMemory>>,
}

pub struct Support {
    pub modifier: Modifier,
    pub planes: usize,
    pub max_width: u32,
    pub max_height: u32,
}

struct EgvRendererInner {
    _instance: VulkanCoreInstance,
    device: Device,
    queue: Queue,
    queue_family: u32,
    external_fence_fd: external_fence_fd::Device,
    external_semaphore_fd: external_semaphore_fd::Device,
    external_memory_fd: external_memory_fd::Device,
    push_descriptor: push_descriptor::Device,
    vert: ShaderModule,
    frag: ShaderModule,
    non_coherent_atom_size: u64,
    descriptor_set_layout: DescriptorSetLayout,
    pipeline_layout: PipelineLayout,
    max_tex_width: u32,
    max_tex_height: u32,
    max_buffer_size: u64,
    allocator: RefCell<GpuAllocator<DeviceMemory>>,
    pool: CommandPool,
    cache: RefCell<EgvRendererCache>,
    submissions: Rc<PendingSubmissions>,
    pipeline: Pipeline,
    dmabuf_support: Vec<Support>,
    next_context_id: NumCell<u64>,
}

#[derive(Default)]
struct EgvRendererCache {
    device_local_buffers: Vec<EgvBuffer>,
    samplers: AHashMap<TextureOptions, Rc<VkSampler>>,
    images: AHashMap<(u64, TextureId), EgvSampledImage>,
    upload_todos: Vec<(Rc<EgvImage<EgvAllocatedMemory>>, EgvBuffer, ImageDelta)>,
    buffer_memory_barriers: Vec<BufferMemoryBarrier2<'static>>,
    initial_image_memory_barriers: Vec<ImageMemoryBarrier2<'static>>,
    final_image_memory_barriers: Vec<ImageMemoryBarrier2<'static>>,
    fences: Vec<EgvFence>,
    semaphores: Vec<EgvSemaphore>,
}

struct EgvBuffer {
    ri: Rc<EgvRendererInner>,
    memory: EgvAllocatedMemory,
    buffer: Buffer,
    size: u64,
    usage: BufferUsageFlags,
    mapping: *mut [u8],
    host_coherent: bool,
}

struct EgvImportedMemory {
    ri: Rc<EgvRendererInner>,
    memories: PlaneVec<DeviceMemory>,
}

struct EgvAllocatedMemory {
    ri: Rc<EgvRendererInner>,
    block: ManuallyDrop<MemoryBlock<DeviceMemory>>,
    mapping: Option<*mut [u8]>,
}

struct EgvCommandBuffer {
    ri: Rc<EgvRendererInner>,
    buf: CommandBuffer,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct VkVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [u8; 4],
}

unsafe impl Pod for VkVertex {}
unsafe impl Packed for VkVertex {}

struct VkSampler {
    ri: Rc<EgvRendererInner>,
    options: TextureOptions,
    sampler: Sampler,
}

#[derive(Clone)]
struct EgvSampledImage {
    image: Rc<EgvImage<EgvAllocatedMemory>>,
    sampler: Rc<VkSampler>,
}

struct EgvImage<M> {
    ri: Rc<EgvRendererInner>,
    width: u32,
    height: u32,
    image: Image,
    image_view: ImageView,
    _memory: M,
    layout: Cell<ImageLayout>,
}

#[derive(Default)]
struct PendingSubmissions {
    task_has_pending: Cell<bool>,
    pending: AsyncQueue<Pending>,
}

struct Pending {
    ri: Rc<EgvRendererInner>,
    sync_file: Option<SyncFile>,
    semaphore: Option<EgvSemaphore>,
    fence: Option<EgvFence>,
    _cmd: EgvCommandBuffer,
    _uploads: Vec<(Rc<EgvImage<EgvAllocatedMemory>>, EgvBuffer)>,
    _sampled: Vec<EgvSampledImage>,
    _fb: Rc<EgvImage<EgvImportedMemory>>,
    index_buffer: Option<EgvBuffer>,
    vertex_buffer: Option<EgvBuffer>,
}

struct EgvFence {
    ri: Rc<EgvRendererInner>,
    fence: Fence,
}

struct EgvSemaphore {
    ri: Rc<EgvRendererInner>,
    semaphore: Semaphore,
}

const SRGB_FORMAT: Format = Format::R8G8B8A8_SRGB;
const SRGB_FORMAT_BPP: u64 = 4;
pub const EGV_FORMAT: &crate::format::Format = XRGB8888;
const VK_FB_FORMAT: Format = Format::B8G8R8A8_SRGB;

const DEVICE_EXTENSIONS: [&CStr; 7] = [
    external_fence_fd::NAME,
    external_semaphore_fd::NAME,
    external_memory_fd::NAME,
    external_memory_dma_buf::NAME,
    image_drm_format_modifier::NAME,
    queue_family_foreign::NAME,
    push_descriptor::NAME,
];

const VERT: &[u8] = include_bytes!("shaders_bin/shader.vert.spv");
const FRAG: &[u8] = include_bytes!("shaders_bin/shader.frag.spv");

const IMAGE_SUBRESOURCE_RANGE: ImageSubresourceRange = ImageSubresourceRange {
    aspect_mask: ImageAspectFlags::COLOR,
    base_mip_level: 0,
    level_count: 1,
    base_array_layer: 0,
    layer_count: 1,
};

const IMAGE_SUBRESOURCE_LAYERS: ImageSubresourceLayers = ImageSubresourceLayers {
    aspect_mask: ImageAspectFlags::COLOR,
    mip_level: 0,
    base_array_layer: 0,
    layer_count: 1,
};

impl EgvRenderer {
    pub fn new(
        eng: &Rc<AsyncEngine>,
        ring: &Rc<IoUring>,
        dev: Option<c::dev_t>,
    ) -> Result<Rc<Self>, EgvError> {
        let core_instance = VulkanCoreInstance::new(Level::Debug)?;
        let instance = &core_instance.instance;
        let mut physical_device;
        let mut device_extensions;
        let mut device_properties;
        'find_device: {
            let devices = unsafe {
                instance
                    .enumerate_physical_devices()
                    .map_err(EgvError::EnumeratePhysicalDevice)?
            };
            'outer: for phy in devices {
                let res = unsafe { instance.enumerate_device_extension_properties(phy) };
                let exts = match res {
                    Ok(res) => map_extension_properties(res),
                    Err(e) => {
                        log::error!(
                            "Could not enumerate extensions of physical device: {}",
                            ErrorFmt(e),
                        );
                        continue;
                    }
                };
                let mut drm_props = PhysicalDeviceDrmPropertiesEXT::default();
                let mut props = PhysicalDeviceProperties2::default().push_next(&mut drm_props);
                unsafe {
                    instance.get_physical_device_properties2(phy, &mut props);
                }
                let props = props.properties;
                physical_device = phy;
                device_extensions = exts;
                device_properties = props;
                if let Some(dev) = dev {
                    if device_extensions.not_contains_key(physical_device_drm::NAME) {
                        continue 'outer;
                    }
                    let major = uapi::major(dev) as i64;
                    let minor = uapi::minor(dev) as i64;
                    let matches = (drm_props.has_primary == vk::TRUE
                        && drm_props.primary_major == major
                        && drm_props.primary_minor == minor)
                        || (drm_props.has_render == vk::TRUE
                            && drm_props.render_major == major
                            && drm_props.render_minor == minor);
                    if matches {
                        break 'find_device;
                    }
                } else {
                    if device_properties.device_type == PhysicalDeviceType::CPU {
                        break 'find_device;
                    }
                }
            }
            return Err(EgvError::NoVulkanDevice);
        }
        if device_properties.api_version < VULKAN_API_VERSION {
            return Err(EgvError::NoVulkan13);
        }
        for ext in DEVICE_EXTENSIONS {
            if device_extensions.not_contains_key(ext) {
                return Err(EgvError::MissingDeviceExtensions(ext));
            }
        }
        {
            let mut synchronization2_features = PhysicalDeviceSynchronization2Features::default();
            let mut dynamic_rendering_features = PhysicalDeviceDynamicRenderingFeatures::default();
            let mut physical_device_features = PhysicalDeviceFeatures2::default()
                .push_next(&mut synchronization2_features)
                .push_next(&mut dynamic_rendering_features);
            unsafe {
                instance
                    .get_physical_device_features2(physical_device, &mut physical_device_features);
            }
            if synchronization2_features.synchronization2 != vk::TRUE {
                return Err(EgvError::NoSynchronization2);
            }
            if dynamic_rendering_features.dynamic_rendering != vk::TRUE {
                return Err(EgvError::NoDynamicRendering);
            }
        }
        {
            let info = PhysicalDeviceExternalSemaphoreInfo::default()
                .handle_type(ExternalSemaphoreHandleTypeFlags::SYNC_FD);
            let mut props = ExternalSemaphoreProperties::default();
            unsafe {
                instance.get_physical_device_external_semaphore_properties(
                    physical_device,
                    &info,
                    &mut props,
                );
            }
            let supported = props
                .external_semaphore_features
                .contains(ExternalSemaphoreFeatureFlags::IMPORTABLE);
            if !supported {
                return Err(EgvError::NoSyncFileImport);
            }
        }
        {
            let info = PhysicalDeviceExternalFenceInfo::default()
                .handle_type(ExternalFenceHandleTypeFlags::SYNC_FD);
            let mut props = ExternalFenceProperties::default();
            unsafe {
                instance.get_physical_device_external_fence_properties(
                    physical_device,
                    &info,
                    &mut props,
                );
            }
            let supported = props
                .external_fence_features
                .contains(ExternalFenceFeatureFlags::EXPORTABLE);
            if !supported {
                return Err(EgvError::NoSyncFileExport);
            }
        }
        let queue_family = 'queue_family: {
            let families =
                unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
            for (idx, family) in families.iter().enumerate() {
                if family.queue_count > 0 && family.queue_flags.contains(QueueFlags::GRAPHICS) {
                    break 'queue_family idx as u32;
                }
            }
            return Err(EgvError::NoGfxQueueFamily);
        };
        let dmabuf_support = {
            let mut list = vec![];
            for attach in [false, true] {
                let mut modifiers = DrmFormatModifierPropertiesListEXT::default();
                if attach {
                    modifiers = modifiers.drm_format_modifier_properties(&mut list);
                }
                let mut out = FormatProperties2::default().push_next(&mut modifiers);
                unsafe {
                    instance.get_physical_device_format_properties2(
                        physical_device,
                        VK_FB_FORMAT,
                        &mut out,
                    );
                }
                if !attach {
                    list = vec![
                        DrmFormatModifierPropertiesEXT::default();
                        modifiers.drm_format_modifier_count as usize
                    ];
                }
            }
            let mut support = vec![];
            for modifier in list {
                let image_features = modifier.drm_format_modifier_tiling_features;
                if !image_features.contains(
                    FormatFeatureFlags::COLOR_ATTACHMENT
                        | FormatFeatureFlags::COLOR_ATTACHMENT_BLEND,
                ) {
                    continue;
                }
                let mut modifier_info = PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
                    .drm_format_modifier(modifier.drm_format_modifier);
                let mut external_memory_info = PhysicalDeviceExternalImageFormatInfoKHR::default()
                    .handle_type(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
                let info = PhysicalDeviceImageFormatInfo2::default()
                    .format(VK_FB_FORMAT)
                    .ty(ImageType::TYPE_2D)
                    .tiling(ImageTiling::DRM_FORMAT_MODIFIER_EXT)
                    .usage(ImageUsageFlags::COLOR_ATTACHMENT)
                    .push_next(&mut external_memory_info)
                    .push_next(&mut modifier_info);
                let mut external_memory_prop = ExternalImageFormatPropertiesKHR::default();
                let mut prop =
                    ImageFormatProperties2::default().push_next(&mut external_memory_prop);
                let res = unsafe {
                    instance.get_physical_device_image_format_properties2(
                        physical_device,
                        &info,
                        &mut prop,
                    )
                };
                if res.is_err() {
                    continue;
                }
                let prop = prop.image_format_properties;
                let memory_features = external_memory_prop
                    .external_memory_properties
                    .external_memory_features;
                if !memory_features.contains(ExternalMemoryFeatureFlags::IMPORTABLE) {
                    continue;
                }
                let me = prop.max_extent;
                if me.width > 0 && me.height > 0 && me.depth > 0 {
                    support.push(Support {
                        modifier: modifier.drm_format_modifier,
                        planes: modifier.drm_format_modifier_plane_count as usize,
                        max_width: me.width,
                        max_height: me.height,
                    });
                }
            }
            support
        };
        let device = {
            let queue_create_info = DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family)
                .queue_priorities(&[1.0]);
            let extensions = DEVICE_EXTENSIONS.map(|e| e.as_ptr());
            let mut dynamic_rendering_features =
                PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);
            let mut synchronization2_features =
                PhysicalDeviceSynchronization2Features::default().synchronization2(true);
            let info = DeviceCreateInfo::default()
                .queue_create_infos(slice::from_ref(&queue_create_info))
                .enabled_extension_names(&extensions)
                .push_next(&mut synchronization2_features)
                .push_next(&mut dynamic_rendering_features);
            unsafe {
                instance
                    .create_device(physical_device, &info, None)
                    .map_err(EgvError::CreateDevice)?
            }
        };
        let destroy_device = on_drop(|| unsafe { device.destroy_device(None) });
        let external_fence_fd = external_fence_fd::Device::new(instance, &device);
        let external_semaphore_fd = external_semaphore_fd::Device::new(instance, &device);
        let external_memory_fd = external_memory_fd::Device::new(instance, &device);
        let push_descriptor = push_descriptor::Device::new(instance, &device);
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        let pool = {
            let info = CommandPoolCreateInfo::default()
                .queue_family_index(queue_family)
                .flags(CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
            unsafe {
                device
                    .create_command_pool(&info, None)
                    .map_err(EgvError::CreateCommandPool)?
            }
        };
        let destroy_pool = on_drop(|| unsafe { device.destroy_command_pool(pool, None) });
        let format_properties =
            unsafe { instance.get_physical_device_format_properties(physical_device, SRGB_FORMAT) };
        let required_features = FormatFeatureFlags::SAMPLED_IMAGE
            | FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR
            | FormatFeatureFlags::TRANSFER_DST;
        if !format_properties
            .optimal_tiling_features
            .contains(required_features)
        {
            return Err(EgvError::MissingFormatFeatures);
        }
        let format_properties = unsafe {
            instance
                .get_physical_device_image_format_properties(
                    physical_device,
                    SRGB_FORMAT,
                    ImageType::TYPE_2D,
                    ImageTiling::OPTIMAL,
                    ImageUsageFlags::SAMPLED | ImageUsageFlags::TRANSFER_SRC,
                    ImageCreateFlags::empty(),
                )
                .map_err(EgvError::GetImageFormatProperties)?
        };
        let max_buffer_size;
        {
            let mut prop13 = PhysicalDeviceVulkan13Properties::default();
            let mut prop = PhysicalDeviceProperties2::default().push_next(&mut prop13);
            unsafe {
                instance.get_physical_device_properties2(physical_device, &mut prop);
            }
            max_buffer_size = prop13.max_buffer_size;
        }
        let create_shader = |src: &[u8]| {
            let mut cursor = Cursor::new(src);
            let spv = read_spv(&mut cursor).map_err(EgvError::ReadSpv)?;
            let create_info = ShaderModuleCreateInfo::default().code(&spv);
            unsafe {
                device
                    .create_shader_module(&create_info, None)
                    .map_err(EgvError::CreateShaderModule)
            }
        };
        let vert = create_shader(VERT)?;
        let destroy_vert = on_drop(|| unsafe { device.destroy_shader_module(vert, None) });
        let frag = create_shader(FRAG)?;
        let destroy_frag = on_drop(|| unsafe { device.destroy_shader_module(frag, None) });
        let descriptor_set_layout = {
            let binding = DescriptorSetLayoutBinding::default()
                .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(ShaderStageFlags::FRAGMENT);
            let create_info = DescriptorSetLayoutCreateInfo::default()
                .flags(DescriptorSetLayoutCreateFlags::PUSH_DESCRIPTOR_KHR)
                .bindings(slice::from_ref(&binding));
            unsafe {
                device
                    .create_descriptor_set_layout(&create_info, None)
                    .map_err(EgvError::CreateDescriptorSetLayout)?
            }
        };
        let destroy_descriptor_set_layout = on_drop(|| unsafe {
            device.destroy_descriptor_set_layout(descriptor_set_layout, None)
        });
        let pipeline_layout = {
            let create_info = PipelineLayoutCreateInfo::default()
                .set_layouts(slice::from_ref(&descriptor_set_layout));
            unsafe {
                device
                    .create_pipeline_layout(&create_info, None)
                    .map_err(EgvError::CreatePipelineLayout)?
            }
        };
        let destroy_pipeline_layout =
            on_drop(|| unsafe { device.destroy_pipeline_layout(pipeline_layout, None) });
        let mut device_properties = unsafe {
            crate::vulkan_core::gpu_alloc_ash::device_properties(instance, physical_device)
                .map_err(EgvError::GetDeviceProperties)?
        };
        device_properties.buffer_device_address = false;
        let non_coherent_atom_size = device_properties.non_coherent_atom_size;
        let allocator = GpuAllocator::new(Config::i_am_potato(), device_properties);
        let pipeline = {
            let stages = [
                PipelineShaderStageCreateInfo::default()
                    .stage(ShaderStageFlags::VERTEX)
                    .module(vert)
                    .name(c"main"),
                PipelineShaderStageCreateInfo::default()
                    .stage(ShaderStageFlags::FRAGMENT)
                    .module(frag)
                    .name(c"main"),
            ];
            let vertex_input_binding_description = VertexInputBindingDescription {
                binding: 0,
                stride: size_of::<Vertex>() as _,
                input_rate: VertexInputRate::VERTEX,
            };
            let vertex_attribute_descriptions = [
                VertexInputAttributeDescription::default()
                    .location(0)
                    .format(Format::R32G32_SFLOAT)
                    .offset(offset_of!(Vertex, pos) as u32),
                VertexInputAttributeDescription::default()
                    .location(1)
                    .format(Format::R32G32_SFLOAT)
                    .offset(offset_of!(Vertex, uv) as u32),
                VertexInputAttributeDescription::default()
                    .location(2)
                    .format(Format::R8G8B8A8_UNORM)
                    .offset(offset_of!(Vertex, color) as u32),
            ];
            let vertex_input_state = PipelineVertexInputStateCreateInfo::default()
                .vertex_binding_descriptions(slice::from_ref(&vertex_input_binding_description))
                .vertex_attribute_descriptions(&vertex_attribute_descriptions);
            let input_assembly_info = PipelineInputAssemblyStateCreateInfo::default()
                .topology(PrimitiveTopology::TRIANGLE_LIST);
            let viewport_state = PipelineViewportStateCreateInfo::default()
                .viewport_count(1)
                .scissor_count(1);
            let rasterization_state = PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(PolygonMode::FILL)
                .cull_mode(CullModeFlags::NONE)
                .front_face(FrontFace::CLOCKWISE)
                .line_width(1.0);
            let multisampling_state = PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(SampleCountFlags::TYPE_1)
                .min_sample_shading(1.0);
            let color_blend_attachment_state = PipelineColorBlendAttachmentState::default()
                .blend_enable(true)
                .src_color_blend_factor(BlendFactor::ONE)
                .dst_color_blend_factor(BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_blend_op(BlendOp::ADD)
                .src_alpha_blend_factor(BlendFactor::ONE)
                .dst_alpha_blend_factor(BlendFactor::ONE_MINUS_SRC_ALPHA)
                .alpha_blend_op(BlendOp::ADD)
                .color_write_mask(ColorComponentFlags::RGBA);
            let color_blend_state = PipelineColorBlendStateCreateInfo::default()
                .attachments(slice::from_ref(&color_blend_attachment_state));
            let dynamic_state = PipelineDynamicStateCreateInfo::default()
                .dynamic_states(&[DynamicState::VIEWPORT, DynamicState::SCISSOR]);
            let mut rendering_create_info = PipelineRenderingCreateInfo::default()
                .color_attachment_formats(slice::from_ref(&VK_FB_FORMAT));
            let create_info = GraphicsPipelineCreateInfo::default()
                .stages(&stages)
                .vertex_input_state(&vertex_input_state)
                .input_assembly_state(&input_assembly_info)
                .viewport_state(&viewport_state)
                .rasterization_state(&rasterization_state)
                .multisample_state(&multisampling_state)
                .color_blend_state(&color_blend_state)
                .dynamic_state(&dynamic_state)
                .layout(pipeline_layout)
                .push_next(&mut rendering_create_info);
            let mut pipelines = unsafe {
                device
                    .create_graphics_pipelines(
                        PipelineCache::null(),
                        slice::from_ref(&create_info),
                        None,
                    )
                    .map_err(|e| EgvError::CreatePipeline(e.1))?
            };
            pipelines.pop().unwrap()
        };
        let destroy_pipeline = on_drop(|| unsafe { device.destroy_pipeline(pipeline, None) });
        let submissions = Rc::new(PendingSubmissions::default());
        destroy_pipeline.forget();
        destroy_pool.forget();
        destroy_pipeline_layout.forget();
        destroy_descriptor_set_layout.forget();
        destroy_frag.forget();
        destroy_vert.forget();
        destroy_device.forget();
        let renderer = Rc::new(EgvRendererInner {
            push_descriptor,
            device,
            queue,
            queue_family,
            external_fence_fd,
            external_semaphore_fd,
            vert,
            frag,
            non_coherent_atom_size,
            descriptor_set_layout,
            pipeline_layout,
            max_tex_width: format_properties.max_extent.width,
            max_tex_height: format_properties.max_extent.height,
            max_buffer_size,
            allocator: RefCell::new(allocator),
            pool,
            cache: Default::default(),
            submissions: submissions.clone(),
            pipeline,
            _instance: core_instance,
            external_memory_fd,
            dmabuf_support,
            next_context_id: Default::default(),
        });
        let task = {
            let future = wait_for_submissions(submissions, renderer.clone(), ring.clone());
            eng.spawn("egui-vulkan-await-pending", future)
        };
        let renderer = Self {
            ri: renderer,
            _task: task,
        };
        Ok(Rc::new(renderer))
    }

    fn create_fence(&self) -> Result<EgvFence, EgvError> {
        let ri = &self.ri;
        let mut export_info =
            ExportFenceCreateInfo::default().handle_types(ExternalFenceHandleTypeFlags::SYNC_FD);
        let create_info = FenceCreateInfo::default().push_next(&mut export_info);
        let fence = unsafe {
            ri.device
                .create_fence(&create_info, None)
                .map_err(EgvError::CreateFence)?
        };
        Ok(EgvFence {
            ri: ri.clone(),
            fence,
        })
    }

    fn create_semaphore(&self) -> Result<EgvSemaphore, EgvError> {
        let ri = &self.ri;
        let create_info = SemaphoreCreateInfo::default();
        let semaphore = unsafe {
            ri.device
                .create_semaphore(&create_info, None)
                .map_err(EgvError::CreateSemaphore)?
        };
        Ok(EgvSemaphore {
            ri: ri.clone(),
            semaphore,
        })
    }

    fn allocate_command_buffer(&self) -> Result<EgvCommandBuffer, EgvError> {
        let ri = &self.ri;
        let allocate_info = CommandBufferAllocateInfo::default()
            .command_pool(ri.pool)
            .command_buffer_count(1)
            .level(CommandBufferLevel::PRIMARY);
        let mut cmd = unsafe {
            ri.device
                .allocate_command_buffers(&allocate_info)
                .map_err(EgvError::AllocateCommandBuffer)?
        };
        Ok(EgvCommandBuffer {
            ri: ri.clone(),
            buf: cmd.pop().unwrap(),
        })
    }

    fn create_image(
        self: &Rc<Self>,
        data: &ImageData,
    ) -> Result<Rc<EgvImage<EgvAllocatedMemory>>, EgvError> {
        let extent = Extent3D {
            width: data.width() as _,
            height: data.height() as _,
            depth: 1,
        };
        if extent.width == 0 || extent.height == 0 {
            return Err(EgvError::EmptyImage);
        }
        let ri = &self.ri;
        if extent.width > ri.max_tex_width || extent.height > ri.max_tex_height {
            return Err(EgvError::TexTooLarge);
        }
        let dev = &ri.device;
        let image = {
            let create_info = ImageCreateInfo::default()
                .image_type(ImageType::TYPE_2D)
                .format(SRGB_FORMAT)
                .extent(extent)
                .mip_levels(1)
                .array_layers(1)
                .samples(SampleCountFlags::TYPE_1)
                .tiling(ImageTiling::OPTIMAL)
                .usage(ImageUsageFlags::SAMPLED | ImageUsageFlags::TRANSFER_DST)
                .sharing_mode(SharingMode::EXCLUSIVE);
            unsafe {
                dev.create_image(&create_info, None)
                    .map_err(EgvError::CreateImage)?
            }
        };
        let destroy_image = on_drop(|| unsafe { dev.destroy_image(image, None) });
        let memory = {
            let req = unsafe { dev.get_image_memory_requirements(image) };
            self.allocate_memory(req, UsageFlags::FAST_DEVICE_ACCESS, false)?
        };
        unsafe {
            dev.bind_image_memory(image, *memory.block.memory(), memory.block.offset())
                .map_err(EgvError::BindImageMemory)?;
        }
        let view = {
            let create_info = ImageViewCreateInfo::default()
                .image(image)
                .view_type(ImageViewType::TYPE_2D)
                .format(SRGB_FORMAT)
                .components(ComponentMapping {
                    r: ComponentSwizzle::R,
                    g: ComponentSwizzle::G,
                    b: ComponentSwizzle::B,
                    a: ComponentSwizzle::A,
                })
                .subresource_range(IMAGE_SUBRESOURCE_RANGE);
            unsafe {
                dev.create_image_view(&create_info, None)
                    .map_err(EgvError::CreateImageView)?
            }
        };
        let destroy_image_view = on_drop(|| unsafe { dev.destroy_image_view(view, None) });
        destroy_image_view.forget();
        destroy_image.forget();
        Ok(Rc::new(EgvImage {
            ri: ri.clone(),
            width: extent.width,
            height: extent.height,
            image,
            _memory: memory,
            image_view: view,
            layout: Cell::new(ImageLayout::UNDEFINED),
        }))
    }

    fn get_device_local_buffer(
        &self,
        sync: &mut EgvRendererCache,
        size: u64,
        usage: BufferUsageFlags,
    ) -> Result<EgvBuffer, EgvError> {
        {
            let mut best = None;
            let mut best_size = u64::MAX;
            for (i, buf) in sync.device_local_buffers.iter().enumerate() {
                if buf.size < size {
                    continue;
                }
                if buf.usage != usage {
                    continue;
                }
                if buf.size < best_size {
                    best = Some(i);
                    best_size = buf.size;
                }
            }
            if let Some(best) = best {
                return Ok(sync.device_local_buffers.swap_remove(best));
            }
        }
        self.create_device_local_buffer(size, usage)
    }

    fn create_device_local_buffer(
        &self,
        size: u64,
        usage: BufferUsageFlags,
    ) -> Result<EgvBuffer, EgvError> {
        self.create_buffer(
            size,
            usage,
            UsageFlags::FAST_DEVICE_ACCESS | UsageFlags::UPLOAD,
        )
    }

    fn create_staging_buffer(&self, size: u64) -> Result<EgvBuffer, EgvError> {
        self.create_buffer(
            size,
            BufferUsageFlags::TRANSFER_SRC,
            UsageFlags::TRANSIENT | UsageFlags::UPLOAD,
        )
    }

    fn create_buffer(
        &self,
        mut size: u64,
        usage: BufferUsageFlags,
        usage_flags: UsageFlags,
    ) -> Result<EgvBuffer, EgvError> {
        const MIN_SIZE: u64 = 1024;
        size = size.max(MIN_SIZE);
        let ri = &self.ri;
        if size > ri.max_buffer_size {
            return Err(EgvError::BufferTooLarge);
        }
        let dev = &ri.device;
        let buffer = {
            let create_info = BufferCreateInfo::default().size(size).usage(usage);
            unsafe {
                dev.create_buffer(&create_info, None)
                    .map_err(EgvError::CreateBuffer)?
            }
        };
        let destroy_buffer = on_drop(|| unsafe { dev.destroy_buffer(buffer, None) });
        let memory_requirements = unsafe { dev.get_buffer_memory_requirements(buffer) };
        let memory = self.allocate_memory(memory_requirements, usage_flags, true)?;
        unsafe {
            dev.bind_buffer_memory(buffer, *memory.block.memory(), memory.block.offset())
                .map_err(EgvError::BindBufferMemory)?;
        }
        destroy_buffer.forget();
        Ok(EgvBuffer {
            ri: ri.clone(),
            mapping: memory.mapping.unwrap(),
            host_coherent: memory
                .block
                .props()
                .contains(MemoryPropertyFlags::HOST_COHERENT),
            memory,
            buffer,
            size,
            usage,
        })
    }

    fn allocate_memory(
        &self,
        req: MemoryRequirements,
        usage: UsageFlags,
        map: bool,
    ) -> Result<EgvAllocatedMemory, EgvError> {
        let request = Request {
            size: req.size,
            align_mask: req.alignment - 1,
            usage,
            memory_types: req.memory_type_bits,
        };
        let ri = &self.ri;
        let block = unsafe {
            ri.allocator
                .borrow_mut()
                .alloc(AshMemoryDevice::wrap(&ri.device), request)
                .map_err(EgvError::AllocateMemory)?
        };
        let block = RefCell::new(ManuallyDrop::new(block));
        let deallocate = on_drop(|| unsafe {
            ri.allocator.borrow_mut().dealloc(
                AshMemoryDevice::wrap(&ri.device),
                ManuallyDrop::take(&mut block.borrow_mut()),
            );
        });
        let mut block_mut = block.borrow_mut();
        let mut mapping = None;
        if map {
            let size = block_mut.size() as usize;
            let ptr = unsafe {
                block_mut
                    .map(AshMemoryDevice::wrap(&ri.device), 0, size)
                    .map_err(EgvError::MapMemory)?
            };
            let slice = unsafe { slice::from_raw_parts_mut(ptr.as_ptr(), size) };
            mapping = Some(slice as *mut [u8]);
        }
        drop(block_mut);
        deallocate.forget();
        Ok(EgvAllocatedMemory {
            ri: ri.clone(),
            block: block.into_inner(),
            mapping,
        })
    }

    fn fill_index_buffer(
        &self,
        sync: &mut EgvRendererCache,
        primitives: &[ClippedPrimitive],
    ) -> Result<EgvBuffer, EgvError> {
        let indices: Vec<_> = primitives
            .iter()
            .filter_map(|c| match &c.primitive {
                Primitive::Mesh(m) => Some(&m.indices),
                Primitive::Callback(_) => None,
            })
            .flat_map(|i| i.iter().copied())
            .collect();
        let indices: &[u8] = uapi::as_bytes(&*indices);
        let buffer = self.get_device_local_buffer(
            sync,
            indices.len() as u64,
            BufferUsageFlags::INDEX_BUFFER,
        )?;
        buffer.upload(indices)?;
        Ok(buffer)
    }

    fn get_sampler(
        self: &Rc<Self>,
        samplers: &mut AHashMap<TextureOptions, Rc<VkSampler>>,
        options: &TextureOptions,
    ) -> Result<Rc<VkSampler>, EgvError> {
        let sampler = match samplers.entry(*options) {
            Entry::Occupied(o) => o.get().clone(),
            Entry::Vacant(v) => {
                let s = self.create_sampler(options)?;
                v.insert(s).clone()
            }
        };
        Ok(sampler)
    }

    fn create_sampler(
        self: &Rc<Self>,
        options: &TextureOptions,
    ) -> Result<Rc<VkSampler>, EgvError> {
        let address_mode = match options.wrap_mode {
            TextureWrapMode::ClampToEdge => SamplerAddressMode::CLAMP_TO_EDGE,
            TextureWrapMode::Repeat => SamplerAddressMode::REPEAT,
            TextureWrapMode::MirroredRepeat => SamplerAddressMode::MIRRORED_REPEAT,
        };
        let map_filter = |f: TextureFilter| match f {
            TextureFilter::Nearest => Filter::NEAREST,
            TextureFilter::Linear => Filter::LINEAR,
        };
        let create_info = SamplerCreateInfo::default()
            .mag_filter(map_filter(options.magnification))
            .min_filter(map_filter(options.minification))
            .address_mode_u(address_mode)
            .address_mode_v(address_mode)
            .address_mode_w(address_mode)
            .mipmap_mode(SamplerMipmapMode::NEAREST)
            .max_anisotropy(1.0)
            .min_lod(0.0)
            .max_lod(0.25)
            .border_color(BorderColor::FLOAT_TRANSPARENT_BLACK);
        let ri = &self.ri;
        let sampler = unsafe {
            ri.device
                .create_sampler(&create_info, None)
                .map_err(EgvError::CreateSampler)?
        };
        Ok(Rc::new(VkSampler {
            ri: ri.clone(),
            options: *options,
            sampler,
        }))
    }

    pub fn support(&self) -> &[Support] {
        &self.ri.dmabuf_support
    }

    pub fn max_texture_side(&self) -> usize {
        self.ri.max_tex_width.min(self.ri.max_tex_height) as usize
    }

    pub fn create_context(self: &Rc<Self>) -> Rc<EgvContext> {
        Rc::new(EgvContext {
            renderer: self.clone(),
            id: self.ri.next_context_id.fetch_add(1),
        })
    }
}

async fn wait_for_submissions(
    submissions: Rc<PendingSubmissions>,
    dev: Rc<EgvRendererInner>,
    ring: Rc<IoUring>,
) {
    loop {
        submissions.task_has_pending.set(false);
        let pending = submissions.pending.pop().await;
        submissions.task_has_pending.set(true);
        if let Some(sync_file) = &pending.sync_file
            && let Err(e) = ring.readable(sync_file).await
        {
            log::warn!(
                "Could not wait for sync file to become readable: {}",
                ErrorFmt(e),
            );
            dev.wait_idle();
        }
    }
}

impl EgvRendererInner {
    fn wait_idle(&self) {
        log::warn!("Blocking");
        let res = unsafe { self.device.device_wait_idle() };
        if let Err(e) = res {
            log::error!("Could not wait for device idle: {}", ErrorFmt(e));
            log::error!("This is unsound.");
        }
        self.submissions.pending.clear();
    }
}

impl EgvContext {
    pub fn import_framebuffer(
        self: &Rc<Self>,
        buf: &DmaBuf,
    ) -> Result<Rc<EgvFramebuffer>, EgvError> {
        let ri = &self.renderer.ri;
        if buf.format != EGV_FORMAT {
            return Err(EgvError::WrongFbFormat);
        }
        if buf.width <= 0 || buf.height <= 0 {
            return Err(EgvError::NonPositiveFbSize);
        }
        let Some(support) = ri
            .dmabuf_support
            .iter()
            .find(|s| s.modifier == buf.modifier)
        else {
            return Err(EgvError::UnsupportedModifier);
        };
        if buf.planes.len() != support.planes {
            return Err(EgvError::WrongPlaneCount);
        }
        let width = buf.width as u32;
        let height = buf.height as u32;
        if width > support.max_width || height > support.max_height {
            return Err(EgvError::TooLarge);
        }
        let dev = &ri.device;
        let disjoint = buf.is_disjoint();
        let image = {
            let image_create_flags = match disjoint {
                true => ImageCreateFlags::DISJOINT,
                false => ImageCreateFlags::empty(),
            };
            let plane_layouts: PlaneVec<_> = buf
                .planes
                .iter()
                .map(|p| SubresourceLayout {
                    offset: p.offset as _,
                    row_pitch: p.stride as _,
                    size: 0,
                    array_pitch: 0,
                    depth_pitch: 0,
                })
                .collect();
            let mut mod_info = ImageDrmFormatModifierExplicitCreateInfoEXT::default()
                .drm_format_modifier(buf.modifier)
                .plane_layouts(&plane_layouts);
            let mut memory_image_create_info = ExternalMemoryImageCreateInfo::default()
                .handle_types(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let info = ImageCreateInfo::default()
                .flags(image_create_flags)
                .image_type(ImageType::TYPE_2D)
                .format(VK_FB_FORMAT)
                .extent(Extent3D {
                    width,
                    height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(SampleCountFlags::TYPE_1)
                .tiling(ImageTiling::DRM_FORMAT_MODIFIER_EXT)
                .usage(ImageUsageFlags::COLOR_ATTACHMENT)
                .sharing_mode(SharingMode::EXCLUSIVE)
                .initial_layout(ImageLayout::UNDEFINED)
                .push_next(&mut mod_info)
                .push_next(&mut memory_image_create_info);
            unsafe {
                dev.create_image(&info, None)
                    .map_err(EgvError::CreateImage)?
            }
        };
        let destroy_image = on_drop(|| unsafe { dev.destroy_image(image, None) });
        let mut memories = PlaneVec::new();
        let mut free_memories = PlaneVec::new();
        {
            let num_device_memories = match disjoint {
                true => buf.planes.len(),
                false => 1,
            };
            let mut bind_image_plane_memory_infos = PlaneVec::new();
            for plane_idx in 0..num_device_memories {
                let dma_buf_plane = &buf.planes[plane_idx];
                let mut image_memory_requirements_info =
                    ImageMemoryRequirementsInfo2::default().image(image);
                let mut image_plane_memory_requirements_info;
                if disjoint {
                    let plane_aspect = match plane_idx {
                        0 => ImageAspectFlags::MEMORY_PLANE_0_EXT,
                        1 => ImageAspectFlags::MEMORY_PLANE_1_EXT,
                        2 => ImageAspectFlags::MEMORY_PLANE_2_EXT,
                        3 => ImageAspectFlags::MEMORY_PLANE_3_EXT,
                        _ => unreachable!(),
                    };
                    image_plane_memory_requirements_info =
                        ImagePlaneMemoryRequirementsInfo::default().plane_aspect(plane_aspect);
                    image_memory_requirements_info = image_memory_requirements_info
                        .push_next(&mut image_plane_memory_requirements_info);
                    bind_image_plane_memory_infos
                        .push(BindImagePlaneMemoryInfo::default().plane_aspect(plane_aspect));
                }
                let mut memory_requirements = MemoryRequirements2::default();
                unsafe {
                    dev.get_image_memory_requirements2(
                        &image_memory_requirements_info,
                        &mut memory_requirements,
                    );
                }
                let mut fd_props = MemoryFdPropertiesKHR::default();
                unsafe {
                    ri.external_memory_fd
                        .get_memory_fd_properties(
                            ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
                            dma_buf_plane.fd.raw(),
                            &mut fd_props,
                        )
                        .map_err(EgvError::GetMemoryFdProperties)?;
                }
                let memory_type_bits = memory_requirements.memory_requirements.memory_type_bits
                    & fd_props.memory_type_bits;
                if memory_type_bits == 0 {
                    return Err(EgvError::NoMemoryTypeForImport);
                }
                let fd = uapi::fcntl_dupfd_cloexec(dma_buf_plane.fd.raw(), 0)
                    .map_err(Into::into)
                    .map_err(EgvError::DupDmaBuf)?;
                let mut memory_dedicated_allocate_info =
                    MemoryDedicatedAllocateInfo::default().image(image);
                let mut import_memory_fd_info = ImportMemoryFdInfoKHR::default()
                    .fd(fd.raw())
                    .handle_type(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
                let memory_allocate_info = MemoryAllocateInfo::default()
                    .allocation_size(memory_requirements.memory_requirements.size)
                    .memory_type_index(memory_type_bits.trailing_zeros() as _)
                    .push_next(&mut import_memory_fd_info)
                    .push_next(&mut memory_dedicated_allocate_info);
                let device_memory = unsafe {
                    dev.allocate_memory(&memory_allocate_info, None)
                        .map_err(EgvError::ImportMemory)?
                };
                let _ = fd.unwrap();
                memories.push(device_memory);
                free_memories.push(on_drop(move || unsafe {
                    dev.free_memory(device_memory, None)
                }));
            }
            let mut bind_image_memory_infos = PlaneVec::new();
            let mut bind_image_plane_memory_infos = bind_image_plane_memory_infos.iter_mut();
            for mem in memories.iter().copied() {
                let mut info = BindImageMemoryInfo::default().image(image).memory(mem);
                if disjoint {
                    info = info.push_next(bind_image_plane_memory_infos.next().unwrap());
                }
                bind_image_memory_infos.push(info);
            }
            unsafe {
                dev.bind_image_memory2(&bind_image_memory_infos)
                    .map_err(EgvError::BindImageMemory)?;
            }
        }
        let image_view = {
            let info = ImageViewCreateInfo::default()
                .image(image)
                .view_type(ImageViewType::TYPE_2D)
                .format(VK_FB_FORMAT)
                .components(ComponentMapping {
                    r: ComponentSwizzle::IDENTITY,
                    g: ComponentSwizzle::IDENTITY,
                    b: ComponentSwizzle::IDENTITY,
                    a: ComponentSwizzle::IDENTITY,
                })
                .subresource_range(IMAGE_SUBRESOURCE_RANGE);
            unsafe {
                dev.create_image_view(&info, None)
                    .map_err(EgvError::CreateImageView)?
            }
        };
        let destroy_image_view = on_drop(|| unsafe { dev.destroy_image_view(image_view, None) });
        destroy_image_view.forget();
        free_memories.into_iter().for_each(|f| f.forget());
        destroy_image.forget();
        let image = Rc::new(EgvImage {
            ri: ri.clone(),
            width,
            height,
            image,
            image_view,
            _memory: EgvImportedMemory {
                ri: ri.clone(),
                memories,
            },
            layout: Cell::new(ImageLayout::UNDEFINED),
        });
        let fb = Rc::new(EgvFramebuffer {
            renderer: self.renderer.clone(),
            ctx: self.clone(),
            image,
        });
        Ok(fb)
    }
}

impl EgvFramebuffer {
    fn create_vertex_buffer(
        &self,
        sync: &mut EgvRendererCache,
        pixels_per_point: f32,
        primitives: &[ClippedPrimitive],
        offset: (f32, f32),
    ) -> Result<EgvBuffer, EgvError> {
        let width = self.image.width as f32 / pixels_per_point;
        let height = self.image.height as f32 / pixels_per_point;
        let vertices: Vec<_> = primitives
            .iter()
            .filter_map(|c| match &c.primitive {
                Primitive::Mesh(m) => Some(&m.vertices),
                Primitive::Callback(_) => None,
            })
            .flat_map(|i| i.iter().copied())
            .map(|mut v| {
                v.pos.x = 2.0 * (v.pos.x + offset.0) / width - 1.0;
                v.pos.y = 2.0 * (v.pos.y + offset.1) / height - 1.0;
                VkVertex {
                    pos: [v.pos.x, v.pos.y],
                    uv: [v.uv.x, v.uv.y],
                    color: [v.color.r(), v.color.g(), v.color.b(), v.color.a()],
                }
            })
            .collect();
        let vertices: &[u8] = uapi::as_bytes(&*vertices);
        let buffer = self.renderer.get_device_local_buffer(
            sync,
            vertices.len() as u64,
            BufferUsageFlags::VERTEX_BUFFER,
        )?;
        buffer.upload(vertices)?;
        Ok(buffer)
    }

    pub fn render(
        &self,
        delta: TexturesDelta,
        pixels_per_point: f32,
        primitives: &[ClippedPrimitive],
        offset: (f32, f32),
        sync_file: Option<&SyncFile>,
    ) -> Result<Option<SyncFile>, EgvError> {
        let renderer = &self.renderer;
        let ri = &renderer.ri;
        let dev = &ri.device;
        let cache = &mut *ri.cache.borrow_mut();
        let index_buffer = self.renderer.fill_index_buffer(cache, primitives)?;
        let vertex_buffer =
            self.create_vertex_buffer(cache, pixels_per_point, primitives, offset)?;
        let uploads = &mut cache.upload_todos;
        uploads.clear();
        for (id, delta) in delta.set {
            let id = (self.ctx.id, id);
            let mut options = delta.options;
            options.mipmap_mode = None;
            let mut create_sampled_image = || -> Result<_, EgvError> {
                let sampler = renderer.get_sampler(&mut cache.samplers, &options)?;
                let image = renderer.create_image(&delta.image)?;
                let sampled = EgvSampledImage {
                    image: image.clone(),
                    sampler,
                };
                Ok((image, sampled))
            };
            let image = match cache.images.entry(id) {
                Entry::Occupied(mut o) => {
                    let t = o.get();
                    if delta.pos.is_none()
                        && [t.image.width as usize, t.image.height as usize] != delta.image.size()
                    {
                        let (image, sampled) = create_sampled_image()?;
                        *o.get_mut() = sampled;
                        image
                    } else if t.sampler.options != options {
                        let sampler = self.renderer.get_sampler(&mut cache.samplers, &options)?;
                        let image = t.image.clone();
                        *o.get_mut() = EgvSampledImage {
                            image: image.clone(),
                            sampler,
                        };
                        image
                    } else {
                        t.image.clone()
                    }
                }
                Entry::Vacant(v) => {
                    if delta.pos.is_some() {
                        return Err(EgvError::PartialTextureUpdateForUnknownTexture(id.1));
                    }
                    let (image, sampled) = create_sampled_image()?;
                    v.insert(sampled);
                    image
                }
            };
            if let Some(pos) = delta.pos {
                let x2 = pos[0].saturating_add(delta.image.width());
                let y2 = pos[1].saturating_add(delta.image.height());
                if x2 > image.width as usize || y2 > image.height as usize {
                    return Err(EgvError::TextureUpdateOutOfBounds(id.1));
                }
            }
            let size = delta.image.width() as u64 * delta.image.height() as u64 * SRGB_FORMAT_BPP;
            uploads.push((image.clone(), renderer.create_staging_buffer(size)?, delta));
        }
        let buffer_memory_barriers = &mut cache.buffer_memory_barriers;
        buffer_memory_barriers.clear();
        let initial_image_barriers = &mut cache.initial_image_memory_barriers;
        initial_image_barriers.clear();
        let final_image_barriers = &mut cache.final_image_memory_barriers;
        final_image_barriers.clear();
        for (image, buf, delta) in &*uploads {
            match &delta.image {
                ImageData::Color(c) => {
                    let pixels = unsafe { AssertPacked::new(&c.pixels as &[_]) };
                    buf.upload(uapi::as_bytes(pixels))?;
                }
            }
            buffer_memory_barriers.push(
                BufferMemoryBarrier2::default()
                    .src_access_mask(AccessFlags2::HOST_WRITE)
                    .src_stage_mask(PipelineStageFlags2::HOST)
                    .dst_access_mask(AccessFlags2::TRANSFER_READ)
                    .dst_stage_mask(PipelineStageFlags2::TRANSFER)
                    .buffer(buf.buffer)
                    .size(WHOLE_SIZE),
            );
            initial_image_barriers.push(
                ImageMemoryBarrier2::default()
                    .src_access_mask(AccessFlags2::SHADER_READ)
                    .src_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER)
                    .dst_access_mask(AccessFlags2::TRANSFER_WRITE)
                    .dst_stage_mask(PipelineStageFlags2::TRANSFER)
                    .old_layout(image.layout.get())
                    .new_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                    .image(image.image)
                    .subresource_range(IMAGE_SUBRESOURCE_RANGE),
            );
            final_image_barriers.push(
                ImageMemoryBarrier2::default()
                    .src_access_mask(AccessFlags2::TRANSFER_WRITE)
                    .src_stage_mask(PipelineStageFlags2::TRANSFER)
                    .dst_access_mask(AccessFlags2::SHADER_READ)
                    .dst_stage_mask(PipelineStageFlags2::FRAGMENT_SHADER)
                    .old_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image(image.image)
                    .subresource_range(IMAGE_SUBRESOURCE_RANGE),
            );
        }
        let cmd = renderer.allocate_command_buffer()?;
        let buf = cmd.buf;
        {
            let begin_info =
                CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            unsafe {
                dev.begin_command_buffer(buf, &begin_info)
                    .map_err(EgvError::BeginCommandBuffer)?;
            }
        }
        unsafe {
            let info = DependencyInfo::default()
                .buffer_memory_barriers(&buffer_memory_barriers)
                .image_memory_barriers(&initial_image_barriers);
            dev.cmd_pipeline_barrier2(buf, &info);
        }
        for (image, staging, delta) in &*uploads {
            let x = delta.pos.unwrap_or_default()[0] as i32;
            let y = delta.pos.unwrap_or_default()[1] as i32;
            let region = BufferImageCopy2::default()
                .image_subresource(IMAGE_SUBRESOURCE_LAYERS)
                .image_offset(Offset3D { x, y, z: 0 })
                .image_extent(Extent3D {
                    width: delta.image.width() as u32,
                    height: delta.image.height() as u32,
                    depth: 1,
                });
            let info = CopyBufferToImageInfo2::default()
                .src_buffer(staging.buffer)
                .dst_image(image.image)
                .dst_image_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                .regions(slice::from_ref(&region));
            unsafe {
                dev.cmd_copy_buffer_to_image2(buf, &info);
            }
        }
        {
            final_image_barriers.push(
                ImageMemoryBarrier2::default()
                    .dst_access_mask(AccessFlags2::COLOR_ATTACHMENT_WRITE)
                    .dst_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                    .old_layout(ImageLayout::GENERAL)
                    .new_layout(ImageLayout::GENERAL)
                    .src_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                    .dst_queue_family_index(ri.queue_family)
                    .image(self.image.image)
                    .subresource_range(IMAGE_SUBRESOURCE_RANGE),
            );
            final_image_barriers.push(
                ImageMemoryBarrier2::default()
                    .src_access_mask(AccessFlags2::COLOR_ATTACHMENT_WRITE)
                    .src_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                    .dst_access_mask(AccessFlags2::COLOR_ATTACHMENT_WRITE)
                    .dst_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                    .old_layout(ImageLayout::GENERAL)
                    .new_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .src_queue_family_index(ri.queue_family)
                    .dst_queue_family_index(ri.queue_family)
                    .image(self.image.image)
                    .subresource_range(IMAGE_SUBRESOURCE_RANGE),
            )
        }
        unsafe {
            let info = DependencyInfo::default().image_memory_barriers(&final_image_barriers);
            dev.cmd_pipeline_barrier2(buf, &info);
        }
        for primitive in primitives {
            match &primitive.primitive {
                Primitive::Mesh(m) => {
                    if cache.images.not_contains_key(&(self.ctx.id, m.texture_id)) {
                        return Err(EgvError::UnknownTexture(m.texture_id));
                    }
                }
                Primitive::Callback(_) => {
                    unreachable!()
                }
            }
        }
        {
            let rendering_attachment_info = RenderingAttachmentInfo::default()
                .image_view(self.image.image_view)
                .image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(AttachmentLoadOp::DONT_CARE)
                .store_op(AttachmentStoreOp::STORE);
            let rendering_info = RenderingInfoKHR::default()
                .render_area(Rect2D {
                    offset: Default::default(),
                    extent: Extent2D {
                        width: self.image.width,
                        height: self.image.height,
                    },
                })
                .layer_count(1)
                .color_attachments(slice::from_ref(&rendering_attachment_info));
            unsafe {
                dev.cmd_begin_rendering(buf, &rendering_info);
            }
        }
        if primitives.is_not_empty() {
            unsafe {
                dev.cmd_bind_pipeline(buf, PipelineBindPoint::GRAPHICS, ri.pipeline);
                dev.cmd_bind_index_buffer(buf, index_buffer.buffer, 0, IndexType::UINT32);
                dev.cmd_bind_vertex_buffers(buf, 0, &[vertex_buffer.buffer], &[0]);
                dev.cmd_set_viewport(
                    buf,
                    0,
                    &[Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: self.image.width as f32,
                        height: self.image.height as f32,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    }],
                );
            }
        }
        let mut first_index = 0;
        let mut vertex_offset = 0;
        let mut sampled_images = Vec::with_capacity(primitives.len());
        for primitive in primitives {
            let mesh = match &primitive.primitive {
                Primitive::Mesh(m) => m,
                Primitive::Callback(_) => unreachable!(),
            };
            let sampled = cache.images.get(&(self.ctx.id, mesh.texture_id)).unwrap();
            sampled_images.push(sampled.clone());
            let image_info = DescriptorImageInfo::default()
                .sampler(sampled.sampler.sampler)
                .image_view(sampled.image.image_view)
                .image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let write_descriptor_set = WriteDescriptorSet::default()
                .descriptor_type(DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(slice::from_ref(&image_info));
            unsafe {
                ri.push_descriptor.cmd_push_descriptor_set(
                    buf,
                    PipelineBindPoint::GRAPHICS,
                    ri.pipeline_layout,
                    0,
                    slice::from_ref(&write_descriptor_set),
                );
            }
            {
                let c = primitive.clip_rect;
                let x1 = ((c.min.x + offset.0) * pixels_per_point).floor().max(0.0) as i32;
                let y1 = ((c.min.y + offset.1) * pixels_per_point).floor().max(0.0) as i32;
                let x2 = ((c.max.x + offset.0) * pixels_per_point).ceil().max(0.0) as i32;
                let y2 = ((c.max.y + offset.1) * pixels_per_point).ceil().max(0.0) as i32;
                unsafe {
                    dev.cmd_set_scissor(
                        buf,
                        0,
                        &[Rect2D {
                            offset: Offset2D { x: x1, y: y1 },
                            extent: Extent2D {
                                width: x2.wrapping_sub(x1) as u32,
                                height: y2.wrapping_sub(y1) as u32,
                            },
                        }],
                    );
                }
            }
            let index_count = mesh.indices.len() as u32;
            unsafe {
                dev.cmd_draw_indexed(buf, index_count, 1, first_index, vertex_offset, 0);
            }
            first_index += index_count;
            vertex_offset += mesh.vertices.len() as i32;
        }
        unsafe {
            dev.cmd_end_rendering(buf);
        }
        {
            final_image_barriers.clear();
            final_image_barriers.push(
                ImageMemoryBarrier2::default()
                    .src_access_mask(AccessFlags2::COLOR_ATTACHMENT_WRITE)
                    .src_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                    .dst_access_mask(AccessFlags2::COLOR_ATTACHMENT_WRITE)
                    .dst_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                    .old_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .new_layout(ImageLayout::GENERAL)
                    .src_queue_family_index(ri.queue_family)
                    .dst_queue_family_index(ri.queue_family)
                    .image(self.image.image)
                    .subresource_range(IMAGE_SUBRESOURCE_RANGE),
            );
            final_image_barriers.push(
                ImageMemoryBarrier2::default()
                    .dst_access_mask(AccessFlags2::COLOR_ATTACHMENT_WRITE)
                    .dst_stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                    .old_layout(ImageLayout::GENERAL)
                    .new_layout(ImageLayout::GENERAL)
                    .src_queue_family_index(ri.queue_family)
                    .dst_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                    .image(self.image.image)
                    .subresource_range(IMAGE_SUBRESOURCE_RANGE),
            );
            unsafe {
                let info = DependencyInfo::default().image_memory_barriers(&final_image_barriers);
                dev.cmd_pipeline_barrier2(buf, &info);
            }
        }
        unsafe {
            dev.end_command_buffer(buf)
                .map_err(EgvError::EndCommandBuffer)?;
        }
        let mut semaphore = None;
        let mut vk_semaphores = ArrayVec::<_, 1>::new();
        if let Some(sync_file) = sync_file {
            let s = match cache.semaphores.pop() {
                Some(f) => f,
                None => renderer.create_semaphore()?,
            };
            s.import(sync_file)?;
            let info = SemaphoreSubmitInfo::default()
                .semaphore(s.semaphore)
                .stage_mask(PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT);
            vk_semaphores.push(info);
            semaphore = Some(s);
        }
        let fence = match cache.fences.pop() {
            Some(f) => f,
            None => renderer.create_fence()?,
        };
        {
            let command_buffer_info = CommandBufferSubmitInfo::default().command_buffer(buf);
            let submit_info = SubmitInfo2::default()
                .command_buffer_infos(slice::from_ref(&command_buffer_info))
                .wait_semaphore_infos(&vk_semaphores);
            unsafe {
                dev.queue_submit2(ri.queue, slice::from_ref(&submit_info), fence.fence)
                    .map_err(EgvError::Submit)?;
            }
        }
        for id in delta.free {
            cache.images.remove(&(self.ctx.id, id));
        }
        let mut used_uploads = Vec::with_capacity(uploads.len());
        for (image, staging, _) in uploads.drain(..) {
            image.layout.set(ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            used_uploads.push((image, staging));
        }
        let sync_file = match fence.export() {
            Ok(f) => f,
            Err(e) => {
                log::error!("Could not export signal fence: {}", ErrorFmt(e));
                ri.wait_idle();
                None
            }
        };
        let pending = Pending {
            ri: ri.clone(),
            sync_file: sync_file.clone(),
            semaphore,
            fence: Some(fence),
            _cmd: cmd,
            _uploads: used_uploads,
            _sampled: sampled_images,
            _fb: self.image.clone(),
            index_buffer: Some(index_buffer),
            vertex_buffer: Some(vertex_buffer),
        };
        ri.submissions.pending.push(pending);
        Ok(sync_file)
    }
}

impl EgvBuffer {
    fn upload(&self, data: &[u8]) -> Result<(), EgvError> {
        assert!(self.mapping.len() >= data.len());
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), self.mapping.cast(), data.len());
        }
        if !self.host_coherent {
            let m = &self.memory;
            let mask = m.ri.non_coherent_atom_size - 1;
            let lo = m.block.offset() & !mask;
            let hi = (m.block.offset() + data.len() as u64 + mask) & !mask;
            let range = MappedMemoryRange::default()
                .memory(*m.block.memory())
                .offset(lo)
                .size(hi - lo);
            unsafe {
                m.ri.device
                    .flush_mapped_memory_ranges(slice::from_ref(&range))
                    .map_err(EgvError::FlushMemory)?;
            }
        }
        Ok(())
    }
}

impl EgvSemaphore {
    fn import(&self, sync_file: &SyncFile) -> Result<(), EgvError> {
        let fd = uapi::fcntl_dupfd_cloexec(sync_file.raw(), 0)
            .map_err(Into::into)
            .map_err(EgvError::DupSyncFile)?;
        let info = ImportSemaphoreFdInfoKHR::default()
            .flags(SemaphoreImportFlags::TEMPORARY)
            .semaphore(self.semaphore)
            .handle_type(ExternalSemaphoreHandleTypeFlags::SYNC_FD)
            .fd(fd.raw());
        unsafe {
            self.ri
                .external_semaphore_fd
                .import_semaphore_fd(&info)
                .map_err(EgvError::ImportSyncFile)?;
        }
        let _ = fd.unwrap();
        Ok(())
    }
}

impl EgvFence {
    fn export(&self) -> Result<Option<SyncFile>, EgvError> {
        let info = FenceGetFdInfoKHR::default()
            .fence(self.fence)
            .handle_type(ExternalFenceHandleTypeFlags::SYNC_FD);
        let fd = unsafe {
            self.ri
                .external_fence_fd
                .get_fence_fd(&info)
                .map_err(EgvError::ExportSyncFile)?
        };
        let fd = if fd == -1 {
            None
        } else {
            Some(SyncFile(Rc::new(OwnedFd::new(fd))))
        };
        Ok(fd)
    }
}

impl Drop for EgvBuffer {
    fn drop(&mut self) {
        unsafe {
            self.ri.device.destroy_buffer(self.buffer, None);
        }
    }
}

impl Drop for EgvRendererInner {
    fn drop(&mut self) {
        let dev = &self.device;
        unsafe {
            dev.destroy_pipeline(self.pipeline, None);
            dev.destroy_command_pool(self.pool, None);
            dev.destroy_pipeline_layout(self.pipeline_layout, None);
            dev.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            dev.destroy_shader_module(self.vert, None);
            dev.destroy_shader_module(self.frag, None);
            dev.destroy_device(None);
        }
    }
}

impl Drop for EgvImportedMemory {
    fn drop(&mut self) {
        unsafe {
            for &memory in &self.memories {
                self.ri.device.free_memory(memory, None);
            }
        }
    }
}

impl Drop for EgvAllocatedMemory {
    fn drop(&mut self) {
        if self.mapping.is_some() {
            unsafe {
                self.block.unmap(AshMemoryDevice::wrap(&self.ri.device));
            }
        }
        unsafe {
            self.ri.allocator.borrow_mut().dealloc(
                AshMemoryDevice::wrap(&self.ri.device),
                ManuallyDrop::take(&mut self.block),
            );
        }
    }
}

impl Drop for EgvCommandBuffer {
    fn drop(&mut self) {
        let ri = &self.ri;
        unsafe {
            ri.device
                .free_command_buffers(ri.pool, slice::from_ref(&self.buf));
        }
    }
}

impl Drop for VkSampler {
    fn drop(&mut self) {
        unsafe {
            self.ri.device.destroy_sampler(self.sampler, None);
        }
    }
}

impl<M> Drop for EgvImage<M> {
    fn drop(&mut self) {
        let dev = &self.ri.device;
        unsafe {
            dev.destroy_image_view(self.image_view, None);
            dev.destroy_image(self.image, None);
        }
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        let cache = &mut *self.ri.cache.borrow_mut();
        if let Some(v) = self.semaphore.take() {
            cache.semaphores.push(v);
        }
        if let Some(v) = self.fence.take() {
            cache.fences.push(v);
        }
        if let Some(v) = self.index_buffer.take() {
            cache.device_local_buffers.push(v);
        }
        if let Some(v) = self.vertex_buffer.take() {
            cache.device_local_buffers.push(v);
        }
    }
}

impl Drop for EgvFence {
    fn drop(&mut self) {
        let dev = &self.ri.device;
        unsafe {
            dev.destroy_fence(self.fence, None);
        }
    }
}

impl Drop for EgvSemaphore {
    fn drop(&mut self) {
        let dev = &self.ri.device;
        unsafe {
            dev.destroy_semaphore(self.semaphore, None);
        }
    }
}

impl Drop for EgvRenderer {
    fn drop(&mut self) {
        let ri = &self.ri;
        if ri.submissions.pending.is_not_empty() || ri.submissions.task_has_pending.get() {
            ri.wait_idle();
        }
        ri.cache.take();
    }
}

impl Drop for EgvContext {
    fn drop(&mut self) {
        self.renderer
            .ri
            .cache
            .borrow_mut()
            .images
            .retain(|&(id, _), _| id != self.id);
    }
}
