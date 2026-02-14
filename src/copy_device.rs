use {
    crate::{
        async_engine::{AsyncEngine, SpawnedFuture},
        format::{FORMATS, Format},
        gfx_api::SyncFile,
        io_uring::IoUring,
        rect::{Rect, Region},
        utils::{
            clonecell::CloneCell, copyhashmap::CopyHashMap, errorfmt::ErrorFmt, queue::AsyncQueue,
            stack::Stack,
        },
        video::{
            LINEAR_MODIFIER, LINEAR_STRIDE_ALIGN, Modifier,
            dmabuf::{DmaBuf, DmaBufIds, DmaBufPlane, PlaneVec},
        },
        vulkan_core::{
            VULKAN_API_VERSION, VulkanCoreError, VulkanCoreInstance, map_extension_properties,
        },
    },
    ahash::{AHashMap, AHashSet},
    arrayvec::ArrayVec,
    ash::{
        Device,
        ext::{
            external_memory_dma_buf, image_drm_format_modifier, physical_device_drm,
            queue_family_foreign,
        },
        khr::{external_fence_fd, external_memory_fd, external_semaphore_fd},
        vk::{
            self, AccessFlags2, BindImageMemoryInfo, BindImagePlaneMemoryInfo, BlitImageInfo2,
            BufferCopy2, BufferCreateInfo, BufferImageCopy2, BufferMemoryBarrier2,
            BufferUsageFlags, CommandBuffer, CommandBufferAllocateInfo, CommandBufferBeginInfo,
            CommandBufferSubmitInfo, CommandBufferUsageFlags, CommandPoolCreateFlags,
            CommandPoolCreateInfo, CopyBufferInfo2, CopyBufferToImageInfo2, CopyImageInfo2,
            CopyImageToBufferInfo2, DependencyInfo, DeviceCreateInfo, DeviceMemory,
            DeviceQueueCreateInfo, DrmFormatModifierPropertiesEXT,
            DrmFormatModifierPropertiesListEXT, ExportFenceCreateInfo, ExportMemoryAllocateInfo,
            Extent3D, ExternalBufferProperties, ExternalFenceFeatureFlags,
            ExternalFenceHandleTypeFlags, ExternalFenceProperties,
            ExternalImageFormatPropertiesKHR, ExternalMemoryBufferCreateInfo,
            ExternalMemoryBufferCreateInfoKHR, ExternalMemoryFeatureFlags,
            ExternalMemoryHandleTypeFlags, ExternalMemoryImageCreateInfo,
            ExternalSemaphoreFeatureFlags, ExternalSemaphoreHandleTypeFlags,
            ExternalSemaphoreProperties, Fence, FenceCreateInfo, FenceGetFdInfoKHR, Filter,
            FormatFeatureFlags, FormatProperties2, ImageAspectFlags, ImageBlit2, ImageCopy2,
            ImageCreateFlags, ImageCreateInfo, ImageDrmFormatModifierExplicitCreateInfoEXT,
            ImageFormatProperties2, ImageLayout, ImageMemoryBarrier2, ImageMemoryRequirementsInfo2,
            ImagePlaneMemoryRequirementsInfo, ImageSubresourceLayers, ImageSubresourceRange,
            ImageTiling, ImageType, ImageUsageFlags, ImportMemoryFdInfoKHR,
            ImportSemaphoreFdInfoKHR, MemoryAllocateInfo, MemoryDedicatedAllocateInfo,
            MemoryFdPropertiesKHR, MemoryGetFdInfoKHR, MemoryPropertyFlags, MemoryRequirements2,
            MemoryType, Offset3D, PhysicalDevice, PhysicalDeviceDrmPropertiesEXT,
            PhysicalDeviceExternalBufferInfo, PhysicalDeviceExternalFenceInfo,
            PhysicalDeviceExternalImageFormatInfoKHR, PhysicalDeviceExternalSemaphoreInfo,
            PhysicalDeviceFeatures2, PhysicalDeviceImageDrmFormatModifierInfoEXT,
            PhysicalDeviceImageFormatInfo2, PhysicalDeviceProperties2,
            PhysicalDeviceSynchronization2Features, PipelineStageFlags2, QUEUE_FAMILY_FOREIGN_EXT,
            Queue, QueueFlags, SampleCountFlags, SemaphoreCreateInfo, SemaphoreImportFlags,
            SemaphoreSubmitInfo, SharingMode, SubmitInfo2, SubresourceLayout, WHOLE_SIZE,
        },
    },
    bstr::ByteSlice,
    isnt::std_1::collections::IsntHashMapExt,
    linearize::{Linearize, LinearizeExt, StaticCopyMap, StaticMap, static_copy_map, static_map},
    log::Level,
    run_on_drop::on_drop,
    std::{
        cell::{Cell, RefCell},
        ffi::CStr,
        fmt::{Debug, Formatter},
        io,
        ops::Deref,
        rc::Rc,
        slice,
    },
    thiserror::Error,
    uapi::{AsUstr, OwnedFd, c},
    vk::{Buffer, CommandPool, Image, Semaphore},
};

#[derive(Debug, Error)]
pub enum CopyDeviceError {
    #[error(transparent)]
    Core(#[from] VulkanCoreError),
    #[error("Could not create a semaphore")]
    CreateSemaphore(#[source] vk::Result),
    #[error("Could not create a fence")]
    CreateFence(#[source] vk::Result),
    #[error("Could not dup a sync file")]
    DupSyncFile(#[source] io::Error),
    #[error("Could not dup a dma buf")]
    DupDmaBuf(#[source] io::Error),
    #[error("Could not import a sync file")]
    ImportSyncFile(#[source] vk::Result),
    #[error("Could not export a sync file")]
    ExportSyncFile(#[source] vk::Result),
    #[error("Could not submit the copy")]
    SubmitCopy(#[source] vk::Result),
    #[error("Could not enumerate the physical devices")]
    EnumeratePhysicalDevice(#[source] vk::Result),
    #[error("Could not find a corresponding vulkan device")]
    NoVulkanDevice,
    #[error("Device does not support vulkan 1.3")]
    NoVulkan13,
    #[error("Device does not support the synchronization2 feature")]
    NoSynchronization2,
    #[error("Device does not support the device extension {}", .0.as_ustr().as_bytes().as_bstr())]
    MissingDeviceExtensions(&'static CStr),
    #[error("Device does not support importing sync files")]
    NoSyncFileImport,
    #[error("Device does not support exporting sync files")]
    NoSyncFileExport,
    #[error("Device does not support importing dma bufs as buffers")]
    NoDmaBufBufferImport,
    #[error("Device does not have a graphics queue family")]
    NoGfxQueueFamily,
    #[error("Could not create the device")]
    CreateDevice(#[source] vk::Result),
    #[error("Could not create a command pool")]
    CreateCommandPool(#[source] vk::Result),
    #[error("Could not create a command buffer")]
    CreateCommandBuffer(#[source] vk::Result),
    #[error("Copy source and destination must have the same size")]
    NotSameSize,
    #[error("Copy source has a non-positive size")]
    NonPositiveSize,
    #[error("The size calculation overflowed")]
    SizeOverflow,
    #[error("The format and/or modifier is not supported")]
    UnsupportedFormat,
    #[error("the image is too large")]
    TooLarge,
    #[error("Copy source has an incorrect number of planes")]
    WrongNumberOfPlanes,
    #[error("Could not create a buffer")]
    CreateBuffer(#[source] vk::Result),
    #[error("Device returned an unexpected required buffer size")]
    UnexpectedBufferSize,
    #[error("Could not query memory fd properties")]
    GetMemoryFdProperties(#[source] vk::Result),
    #[error("Could not find a memory type for import")]
    NoMemoryTypeForImport,
    #[error("Could not import memory")]
    ImportMemory(#[source] vk::Result),
    #[error("Could not bind buffer memory")]
    BindBufferMemory(#[source] vk::Result),
    #[error("Could not bind image memory")]
    BindImageMemory(#[source] vk::Result),
    #[error("Could not create an image")]
    CreateImage(#[source] vk::Result),
    #[error("Could not begin a command buffer")]
    BeginCommandBuffer(#[source] vk::Result),
    #[error("Could not end a command buffer")]
    EndCommandBuffer(#[source] vk::Result),
    #[error("The previous copy is still executing")]
    Busy,
    #[error("The device does not support dmabuf export")]
    NoDmabufExport,
    #[error("Could not find a memory type for import")]
    NoMemoryTypeForAllocation,
    #[error("Could not allocate memory")]
    AllocateMemory(#[source] vk::Result),
    #[error("Could not export a dmabuf")]
    ExportDmabuf(#[source] vk::Result),
    #[error("Both buffers are off device")]
    BothOffDevice,
    #[error("Cannot blit between these formats")]
    BlitNotSupported,
}

type Keyed<T> = StaticMap<TransferType, T>;
type KeyedCopy<T> = StaticCopyMap<TransferType, T>;

pub struct PhysicalCopyDevice {
    ring: Rc<IoUring>,
    eng: Rc<AsyncEngine>,
    instance: VulkanCoreInstance,
    physical_device: PhysicalDevice,
    support: AHashMap<u32, StaticMap<Dir, Vec<CopyDeviceSupport>>>,
    queues_to_allocate: Vec<QueueToAllocate>,
    queues: KeyedCopy<QueueIndex>,
    supports_dmabuf_export: bool,
    memory_types: Vec<MemoryType>,
    rects: RefCell<Vec<(i32, i32, u32, u32)>>,
    buffer_copy_2: RefCell<Vec<BufferCopy2<'static>>>,
    buffer_image_copy_2: RefCell<Vec<BufferImageCopy2<'static>>>,
    image_copy_2: RefCell<Vec<ImageCopy2<'static>>>,
    image_blit_2: RefCell<Vec<ImageBlit2<'static>>>,
}

#[derive(Debug)]
struct QueueToAllocate {
    family: u32,
    num: usize,
}

#[derive(Copy, Clone, Default, Debug)]
struct QueueIndex {
    allocate_idx: usize,
    family: u32,
    idx_within_family: u32,
    transfer_granularity_mask: (u32, u32),
}

pub struct CopyDevice {
    _tasks: Vec<SpawnedFuture<()>>,
    dev: Rc<CopyDeviceInner>,
}

struct CopyDeviceInner {
    phy: Rc<PhysicalCopyDevice>,
    dev: Device,
    unique_pools: Vec<CommandPool>,
    pools: Keyed<CommandPool>,
    queues: KeyedCopy<Queue>,
    external_semaphore_fd: external_semaphore_fd::Device,
    external_fence_fd: external_fence_fd::Device,
    external_memory_fd: external_memory_fd::Device,
    semaphores: Stack<VulkanSemaphore>,
    fences: Stack<VulkanFence>,
    submissions: Keyed<Rc<PendingSubmissions>>,
}

#[derive(Default)]
struct PendingSubmissions {
    task_has_pending: Cell<bool>,
    pending: AsyncQueue<Pending>,
}

pub struct CopyDeviceCopy {
    inner: Rc<CopyDeviceCopyInner>,
    _dev: Rc<CopyDevice>,
}

struct CopyDeviceCopyInner {
    dev: Rc<CopyDeviceInner>,
    busy: CloneCell<Option<SyncFile>>,
    width: u32,
    height: u32,
    command_buffer: CommandBuffer,
    tt: TransferType,
    ty: CopyDeviceCopyType,
}

enum CopyDeviceCopyType {
    BufferToBuffer {
        src: VulkanBuffer,
        dst: VulkanBuffer,
        stride: u32,
        bpp: u32,
    },
    BufferToImage {
        buf: VulkanBuffer,
        buf_format: &'static Format,
        buf_stride: u32,
        img: VulkanImage,
    },
    ImageToBuffer {
        img: VulkanImage,
        buf: VulkanBuffer,
        buf_format: &'static Format,
        buf_stride: u32,
    },
    ImageToImage {
        src: VulkanImage,
        dst: VulkanImage,
    },
    Blit {
        src: VulkanImage,
        dst: VulkanImage,
    },
}

struct Pending {
    dev: Rc<CopyDeviceInner>,
    sync_file: Option<SyncFile>,
    copy: Rc<CopyDeviceCopyInner>,
    semaphore: Option<VulkanSemaphore>,
    fence: Option<VulkanFence>,
}

struct VulkanSemaphore {
    dev: Rc<CopyDeviceInner>,
    semaphore: Semaphore,
}

struct VulkanFence {
    dev: Rc<CopyDeviceInner>,
    fence: Fence,
}

struct VulkanBuffer {
    dev: Rc<CopyDeviceInner>,
    buf: Buffer,
    mem: DeviceMemory,
}

struct VulkanImage {
    dev: Rc<CopyDeviceInner>,
    img: Image,
    mem: PlaneVec<DeviceMemory>,
}

#[derive(Copy, Clone)]
pub struct CopyDeviceSupport {
    pub modifier: Modifier,
    pub planes: usize,
    pub max_width: u32,
    pub max_height: u32,
    pub blit: bool,
}

pub struct CopyDeviceBuffer {
    device: Rc<CopyDeviceInner>,
    memory: DeviceMemory,
    dmabuf: DmaBuf,
}

#[derive(Copy, Clone, Debug, Linearize)]
enum TransferType {
    Blit,
    Intra,
    Download,
    Upload,
}

#[derive(Copy, Clone, Debug, Linearize)]
enum Dir {
    Src,
    Dst,
}

struct ClassifiedDmabuf<'a> {
    fd_props: PlaneVec<MemoryFdPropertiesKHR<'static>>,
    on_device: bool,
    buffer_possible: bool,
    format: &'a CopyDeviceSupport,
}

pub struct CopyDeviceRegistry {
    ring: Rc<IoUring>,
    eng: Rc<AsyncEngine>,
    devs: CopyHashMap<c::dev_t, Option<Rc<PhysicalCopyDevice>>>,
}

const DEVICE_EXTENSIONS: [&CStr; 6] = [
    external_semaphore_fd::NAME,
    external_fence_fd::NAME,
    external_memory_fd::NAME,
    external_memory_dma_buf::NAME,
    image_drm_format_modifier::NAME,
    queue_family_foreign::NAME,
];

impl PhysicalCopyDevice {
    fn new(
        ring: &Rc<IoUring>,
        eng: &Rc<AsyncEngine>,
        dev: c::dev_t,
    ) -> Result<Rc<Self>, CopyDeviceError> {
        let core_instance = VulkanCoreInstance::new(Level::Debug)?;
        let instance = &core_instance.instance;
        let physical_device;
        let device_extensions;
        let device_properties;
        let supports_dmabuf_export;
        'find_device: {
            let devices = unsafe {
                instance
                    .enumerate_physical_devices()
                    .map_err(CopyDeviceError::EnumeratePhysicalDevice)?
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
                if exts.not_contains_key(physical_device_drm::NAME) {
                    continue 'outer;
                }
                let mut drm_props = PhysicalDeviceDrmPropertiesEXT::default();
                let mut props = PhysicalDeviceProperties2::default().push_next(&mut drm_props);
                unsafe {
                    instance.get_physical_device_properties2(phy, &mut props);
                }
                let props = props.properties;
                let major = uapi::major(dev) as i64;
                let minor = uapi::minor(dev) as i64;
                let matches = (drm_props.has_primary == vk::TRUE
                    && drm_props.primary_major == major
                    && drm_props.primary_minor == minor)
                    || (drm_props.has_render == vk::TRUE
                        && drm_props.render_major == major
                        && drm_props.render_minor == minor);
                if matches {
                    physical_device = phy;
                    device_extensions = exts;
                    device_properties = props;
                    break 'find_device;
                }
            }
            return Err(CopyDeviceError::NoVulkanDevice);
        }
        if device_properties.api_version < VULKAN_API_VERSION {
            return Err(CopyDeviceError::NoVulkan13);
        }
        for ext in DEVICE_EXTENSIONS {
            if device_extensions.not_contains_key(ext) {
                return Err(CopyDeviceError::MissingDeviceExtensions(ext));
            }
        }
        {
            let mut synchronization2_features = PhysicalDeviceSynchronization2Features::default();
            let mut physical_device_features =
                PhysicalDeviceFeatures2::default().push_next(&mut synchronization2_features);
            unsafe {
                instance
                    .get_physical_device_features2(physical_device, &mut physical_device_features);
            }
            if synchronization2_features.synchronization2 != vk::TRUE {
                return Err(CopyDeviceError::NoSynchronization2);
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
                return Err(CopyDeviceError::NoSyncFileImport);
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
                return Err(CopyDeviceError::NoSyncFileExport);
            }
        }
        {
            let info = PhysicalDeviceExternalBufferInfo::default()
                .handle_type(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
                .usage(BufferUsageFlags::TRANSFER_SRC | BufferUsageFlags::TRANSFER_DST);
            let mut props = ExternalBufferProperties::default();
            unsafe {
                instance.get_physical_device_external_buffer_properties(
                    physical_device,
                    &info,
                    &mut props,
                );
            }
            let features = props.external_memory_properties.external_memory_features;
            let supported = features.contains(ExternalMemoryFeatureFlags::IMPORTABLE);
            if !supported {
                return Err(CopyDeviceError::NoDmaBufBufferImport);
            }
            supports_dmabuf_export = features.contains(ExternalMemoryFeatureFlags::EXPORTABLE);
        }
        let (queues_to_allocate, queue_indices) = {
            let families =
                unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
            let mut transfer_only = None;
            let mut compute_only = None;
            let mut gfx = None;
            for (idx, family) in families.iter().enumerate() {
                let idx = idx as u32;
                let g = family.min_image_transfer_granularity;
                let g = (g.width.wrapping_sub(1), g.height.wrapping_sub(1));
                if g.0 == u32::MAX || g.1 == u32::MAX {
                    continue;
                }
                let count = family.queue_count;
                if count == 0 {
                    continue;
                }
                let v = (idx, g, count);
                let flags = family.queue_flags;
                if flags.contains(QueueFlags::GRAPHICS) {
                    if gfx.is_none() {
                        gfx = Some(v);
                    }
                } else if flags.contains(QueueFlags::COMPUTE) {
                    if compute_only.is_none() {
                        compute_only = Some(v);
                    }
                } else if flags.contains(QueueFlags::TRANSFER) {
                    if transfer_only.is_none() {
                        transfer_only = Some(v);
                    }
                }
            }
            let gfx = gfx.ok_or(CopyDeviceError::NoGfxQueueFamily)?;
            allocate_queues(gfx, compute_only, transfer_only)
        };
        let mut support = AHashMap::default();
        for format in FORMATS {
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
                        format.vk_format,
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
            let mut format_support = StaticMap::<_, Vec<_>>::default();
            for modifier in list {
                for dir in Dir::variants() {
                    let format_feature_flags = match dir {
                        Dir::Src => FormatFeatureFlags::TRANSFER_SRC,
                        Dir::Dst => FormatFeatureFlags::TRANSFER_DST,
                    };
                    let blit_feature_flags = match dir {
                        Dir::Src => FormatFeatureFlags::BLIT_SRC,
                        Dir::Dst => FormatFeatureFlags::BLIT_DST,
                    };
                    let image_usage_flags = match dir {
                        Dir::Src => ImageUsageFlags::TRANSFER_SRC,
                        Dir::Dst => ImageUsageFlags::TRANSFER_DST,
                    };
                    let image_features = modifier.drm_format_modifier_tiling_features;
                    if !image_features.contains(format_feature_flags) {
                        continue;
                    }
                    let supports_blit = image_features.contains(blit_feature_flags);
                    let mut modifier_info = PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
                        .drm_format_modifier(modifier.drm_format_modifier);
                    let mut external_memory_info =
                        PhysicalDeviceExternalImageFormatInfoKHR::default()
                            .handle_type(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
                    let info = PhysicalDeviceImageFormatInfo2::default()
                        .format(format.vk_format)
                        .ty(ImageType::TYPE_2D)
                        .tiling(ImageTiling::DRM_FORMAT_MODIFIER_EXT)
                        .usage(image_usage_flags)
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
                        format_support[dir].push(CopyDeviceSupport {
                            modifier: modifier.drm_format_modifier,
                            planes: modifier.drm_format_modifier_plane_count as usize,
                            max_width: me.width,
                            max_height: me.height,
                            blit: supports_blit,
                        });
                    }
                }
            }
            support.insert(format.drm, format_support);
        }
        let memory_info =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let dev = Rc::new(PhysicalCopyDevice {
            ring: ring.clone(),
            eng: eng.clone(),
            instance: core_instance,
            physical_device,
            support,
            queues_to_allocate,
            queues: queue_indices,
            supports_dmabuf_export,
            memory_types: memory_info.memory_types_as_slice().to_vec(),
            rects: Default::default(),
            buffer_copy_2: Default::default(),
            image_blit_2: Default::default(),
            image_copy_2: Default::default(),
            buffer_image_copy_2: Default::default(),
        });
        Ok(dev)
    }

    pub fn src_support(&self, format: &Format) -> &[CopyDeviceSupport] {
        self.support(format, Dir::Src)
    }

    pub fn dst_support(&self, format: &Format) -> &[CopyDeviceSupport] {
        self.support(format, Dir::Dst)
    }

    fn support(&self, format: &Format, dir: Dir) -> &[CopyDeviceSupport] {
        self.support
            .get(&format.drm)
            .map(|s| s[dir].as_slice())
            .unwrap_or_default()
    }

    pub fn create_device(self: &Rc<Self>) -> Result<Rc<CopyDevice>, CopyDeviceError> {
        let instance = &self.instance.instance;
        let device = {
            let priorities = [1.0; TransferType::LENGTH];
            let queue_create_info: Vec<_> = self
                .queues_to_allocate
                .iter()
                .map(|q| {
                    DeviceQueueCreateInfo::default()
                        .queue_family_index(q.family)
                        .queue_priorities(&priorities[..q.num])
                })
                .collect();
            let extensions = DEVICE_EXTENSIONS.map(|e| e.as_ptr());
            let mut synchronization2_features =
                PhysicalDeviceSynchronization2Features::default().synchronization2(true);
            let info = DeviceCreateInfo::default()
                .queue_create_infos(&queue_create_info)
                .enabled_extension_names(&extensions)
                .push_next(&mut synchronization2_features);
            unsafe {
                instance
                    .create_device(self.physical_device, &info, None)
                    .map_err(CopyDeviceError::CreateDevice)?
            }
        };
        let destroy_device = on_drop(|| unsafe { device.destroy_device(None) });
        let external_semaphore_fd = external_semaphore_fd::Device::new(instance, &device);
        let external_fence_fd = external_fence_fd::Device::new(instance, &device);
        let external_memory_fd = external_memory_fd::Device::new(instance, &device);
        let queues = self.queues.map_values(|idx| unsafe {
            device.get_device_queue(idx.family, idx.idx_within_family)
        });
        let mut unique_pools = vec![];
        let mut destroy_pools = vec![];
        for q in &self.queues_to_allocate {
            let info = CommandPoolCreateInfo::default()
                .queue_family_index(q.family)
                .flags(CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
            let pool = unsafe {
                device
                    .create_command_pool(&info, None)
                    .map_err(CopyDeviceError::CreateCommandPool)?
            };
            unique_pools.push(pool);
            let device = &device;
            let destroy_pool = on_drop(move || unsafe { device.destroy_command_pool(pool, None) });
            destroy_pools.push(destroy_pool);
        }
        let pools: StaticMap<TransferType, _> = static_map! {
            tt => unique_pools[self.queues[tt].allocate_idx]
        };
        let submissions_list: Vec<Vec<Rc<PendingSubmissions>>> = self
            .queues_to_allocate
            .iter()
            .map(|q| vec![Default::default(); q.num])
            .collect();
        let submissions = self
            .queues
            .into_static_map()
            .map_values(|q| submissions_list[q.allocate_idx][q.idx_within_family as usize].clone());
        destroy_pools.into_iter().for_each(|v| v.forget());
        destroy_device.forget();
        let dev = Rc::new(CopyDeviceInner {
            phy: self.clone(),
            dev: device,
            unique_pools,
            pools,
            queues,
            external_semaphore_fd,
            external_fence_fd,
            external_memory_fd,
            semaphores: Default::default(),
            fences: Default::default(),
            submissions,
        });
        let mut tasks = vec![];
        for submissions in submissions_list.iter().flatten().cloned() {
            let future = wait_for_submissions(submissions, dev.clone(), self.ring.clone());
            let task = self.eng.spawn("copy-device-await-pending", future);
            tasks.push(task);
        }
        let queue = Rc::new(CopyDevice { dev, _tasks: tasks });
        Ok(queue)
    }
}

async fn wait_for_submissions(
    submissions: Rc<PendingSubmissions>,
    dev: Rc<CopyDeviceInner>,
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

impl CopyDevice {
    fn classify_dmabuf(
        &self,
        buf: &DmaBuf,
        dir: Dir,
    ) -> Result<ClassifiedDmabuf<'_>, CopyDeviceError> {
        if buf.width <= 0 || buf.height <= 0 {
            return Err(CopyDeviceError::NonPositiveSize);
        }
        let width = buf.width as u32;
        let height = buf.height as u32;
        let Some(format) = self
            .dev
            .phy
            .support(buf.format, dir)
            .iter()
            .find(|s| s.modifier == buf.modifier)
        else {
            return Err(CopyDeviceError::UnsupportedFormat);
        };
        if width > format.max_width || height > format.max_height {
            return Err(CopyDeviceError::TooLarge);
        }
        if buf.planes.len() != format.planes {
            return Err(CopyDeviceError::WrongNumberOfPlanes);
        }
        let mut fd_props = PlaneVec::new();
        for plane in &buf.planes {
            let mut props = MemoryFdPropertiesKHR::default();
            unsafe {
                self.dev
                    .external_memory_fd
                    .get_memory_fd_properties(
                        ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
                        plane.fd.raw(),
                        &mut props,
                    )
                    .map_err(CopyDeviceError::GetMemoryFdProperties)?;
            }
            fd_props.push(props);
            if buf.is_one_file() {
                break;
            }
        }
        let mut on_device = true;
        for prop in &fd_props {
            let mut plane_on_device = false;
            for (idx, ty) in self.dev.phy.memory_types.iter().enumerate() {
                if prop.memory_type_bits & (1 << idx) != 0
                    && ty
                        .property_flags
                        .contains(MemoryPropertyFlags::DEVICE_LOCAL)
                {
                    plane_on_device = true;
                    break;
                }
            }
            if !plane_on_device {
                on_device = false;
                break;
            }
        }
        let buffer_possible = buf.modifier == LINEAR_MODIFIER
            && buf.planes.len() == 1
            && buf.planes[0].stride % buf.format.bpp == 0
            && width <= buf.planes[0].stride / buf.format.bpp;
        Ok(ClassifiedDmabuf {
            fd_props,
            on_device,
            buffer_possible,
            format,
        })
    }

    fn import_buffer(
        &self,
        tt: TransferType,
        class: &ClassifiedDmabuf,
        buf: &DmaBuf,
        dir: Dir,
    ) -> Result<VulkanBuffer, CopyDeviceError> {
        assert!(class.buffer_possible);
        let height = buf.height as u32;
        let plane = &buf.planes[0];
        let queue_family = self.dev.phy.queues[tt].family;
        let buffer_size = plane.stride as u64 * height as u64;
        let buffer = {
            let buffer_usage_flags = match dir {
                Dir::Src => BufferUsageFlags::TRANSFER_SRC,
                Dir::Dst => BufferUsageFlags::TRANSFER_DST,
            };
            let mut external_info = ExternalMemoryBufferCreateInfoKHR::default()
                .handle_types(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let info = BufferCreateInfo::default()
                .size(buffer_size)
                .usage(buffer_usage_flags)
                .queue_family_indices(slice::from_ref(&queue_family))
                .push_next(&mut external_info);
            unsafe {
                self.dev
                    .dev
                    .create_buffer(&info, None)
                    .map_err(CopyDeviceError::CreateBuffer)?
            }
        };
        let destroy_buffer = on_drop(|| unsafe { self.dev.dev.destroy_buffer(buffer, None) });
        let memory = {
            let out = unsafe { self.dev.dev.get_buffer_memory_requirements(buffer) };
            if out.size > buffer_size {
                return Err(CopyDeviceError::UnexpectedBufferSize);
            }
            let memory_type_bits = class.fd_props[0].memory_type_bits & out.memory_type_bits;
            if memory_type_bits == 0 {
                return Err(CopyDeviceError::NoMemoryTypeForImport);
            }
            let fd = uapi::fcntl_dupfd_cloexec(plane.fd.raw(), 0)
                .map_err(Into::into)
                .map_err(CopyDeviceError::DupDmaBuf)?;
            let mut dedicated_allocation = MemoryDedicatedAllocateInfo::default().buffer(buffer);
            let mut external_memory = ImportMemoryFdInfoKHR::default()
                .handle_type(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
                .fd(fd.raw());
            let allocate_info = MemoryAllocateInfo::default()
                .allocation_size(out.size)
                .memory_type_index(memory_type_bits.trailing_zeros() as _)
                .push_next(&mut external_memory)
                .push_next(&mut dedicated_allocation);
            let memory = unsafe {
                self.dev
                    .dev
                    .allocate_memory(&allocate_info, None)
                    .map_err(CopyDeviceError::ImportMemory)?
            };
            let _ = fd.unwrap();
            memory
        };
        let free_memory = on_drop(|| unsafe { self.dev.dev.free_memory(memory, None) });
        unsafe {
            self.dev
                .dev
                .bind_buffer_memory(buffer, memory, 0)
                .map_err(CopyDeviceError::BindBufferMemory)?;
        }
        free_memory.forget();
        destroy_buffer.forget();
        Ok(VulkanBuffer {
            dev: self.dev.clone(),
            buf: buffer,
            mem: memory,
        })
    }

    fn import_image(
        &self,
        tt: TransferType,
        class: &ClassifiedDmabuf,
        buf: &DmaBuf,
        dir: Dir,
    ) -> Result<VulkanImage, CopyDeviceError> {
        let dev = &self.dev.dev;
        let disjoint = buf.is_disjoint();
        let queue_family = self.dev.phy.queues[tt].family;
        let image = {
            let image_create_flags = match disjoint {
                true => ImageCreateFlags::DISJOINT,
                false => ImageCreateFlags::empty(),
            };
            let image_usage_flags = match dir {
                Dir::Src => ImageUsageFlags::TRANSFER_SRC,
                Dir::Dst => ImageUsageFlags::TRANSFER_DST,
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
                .format(buf.format.vk_format)
                .extent(Extent3D {
                    width: buf.width as _,
                    height: buf.height as _,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(SampleCountFlags::TYPE_1)
                .tiling(ImageTiling::DRM_FORMAT_MODIFIER_EXT)
                .usage(image_usage_flags)
                .sharing_mode(SharingMode::EXCLUSIVE)
                .queue_family_indices(slice::from_ref(&queue_family))
                .initial_layout(ImageLayout::UNDEFINED)
                .push_next(&mut mod_info)
                .push_next(&mut memory_image_create_info);
            unsafe {
                dev.create_image(&info, None)
                    .map_err(CopyDeviceError::CreateImage)?
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
                let memory_type_bits = memory_requirements.memory_requirements.memory_type_bits
                    & class.fd_props[plane_idx].memory_type_bits;
                if memory_type_bits == 0 {
                    return Err(CopyDeviceError::NoMemoryTypeForImport);
                }
                let fd = uapi::fcntl_dupfd_cloexec(dma_buf_plane.fd.raw(), 0)
                    .map_err(Into::into)
                    .map_err(CopyDeviceError::DupDmaBuf)?;
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
                        .map_err(CopyDeviceError::ImportMemory)?
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
                    .map_err(CopyDeviceError::BindImageMemory)?;
            }
        }
        free_memories.into_iter().for_each(|f| f.forget());
        destroy_image.forget();
        Ok(VulkanImage {
            dev: self.dev.clone(),
            img: image,
            mem: memories,
        })
    }

    pub fn create_copy(
        self: &Rc<Self>,
        src: &DmaBuf,
        dst: &DmaBuf,
    ) -> Result<CopyDeviceCopy, CopyDeviceError> {
        if (dst.width, dst.height) != (src.width, src.height) {
            return Err(CopyDeviceError::NotSameSize);
        }
        let src_class = self.classify_dmabuf(src, Dir::Src)?;
        let dst_class = self.classify_dmabuf(dst, Dir::Dst)?;
        let blit = src.format != dst.format;
        if blit && (!src_class.format.blit || !dst_class.format.blit) {
            return Err(CopyDeviceError::BlitNotSupported);
        }
        let tt = match (src_class.on_device, dst_class.on_device) {
            (false, false) => return Err(CopyDeviceError::BothOffDevice),
            _ if blit => TransferType::Blit,
            (false, true) => TransferType::Upload,
            (true, false) => TransferType::Download,
            (true, true) => TransferType::Intra,
        };
        let dev = &self.dev.dev;
        let command_buffer = {
            let info = CommandBufferAllocateInfo::default()
                .command_pool(self.dev.pools[tt])
                .command_buffer_count(1);
            let mut buf = unsafe {
                dev.allocate_command_buffers(&info)
                    .map_err(CopyDeviceError::CreateCommandBuffer)?
            };
            assert_eq!(buf.len(), 1);
            buf.pop().unwrap()
        };
        let free_command_buffer =
            on_drop(|| unsafe { dev.free_command_buffers(self.dev.pools[tt], &[command_buffer]) });
        let ty = if blit {
            CopyDeviceCopyType::Blit {
                src: self.import_image(tt, &src_class, src, Dir::Src)?,
                dst: self.import_image(tt, &dst_class, dst, Dir::Dst)?,
            }
        } else if !src_class.buffer_possible && !dst_class.buffer_possible {
            CopyDeviceCopyType::ImageToImage {
                src: self.import_image(tt, &src_class, src, Dir::Src)?,
                dst: self.import_image(tt, &dst_class, dst, Dir::Dst)?,
            }
        } else if src_class.buffer_possible
            && dst_class.buffer_possible
            && src.planes[0].stride == dst.planes[0].stride
        {
            CopyDeviceCopyType::BufferToBuffer {
                src: self.import_buffer(tt, &src_class, src, Dir::Src)?,
                dst: self.import_buffer(tt, &dst_class, dst, Dir::Dst)?,
                stride: src.planes[0].stride,
                bpp: src.format.bpp,
            }
        } else if src_class.buffer_possible {
            CopyDeviceCopyType::BufferToImage {
                buf: self.import_buffer(tt, &src_class, src, Dir::Src)?,
                buf_format: src.format,
                buf_stride: src.planes[0].stride,
                img: self.import_image(tt, &dst_class, dst, Dir::Dst)?,
            }
        } else {
            CopyDeviceCopyType::ImageToBuffer {
                img: self.import_image(tt, &src_class, src, Dir::Src)?,
                buf: self.import_buffer(tt, &dst_class, dst, Dir::Dst)?,
                buf_format: dst.format,
                buf_stride: dst.planes[0].stride,
            }
        };
        free_command_buffer.forget();
        Ok(CopyDeviceCopy {
            inner: Rc::new(CopyDeviceCopyInner {
                dev: self.dev.clone(),
                busy: Default::default(),
                width: src.width as _,
                height: src.height as _,
                command_buffer,
                tt,
                ty,
            }),
            _dev: self.clone(),
        })
    }

    pub fn create_buffer(
        &self,
        dma_buf_ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
    ) -> Result<CopyDeviceBuffer, CopyDeviceError> {
        if !self.dev.phy.supports_dmabuf_export {
            return Err(CopyDeviceError::NoDmabufExport);
        }
        if width <= 0 || height <= 0 {
            return Err(CopyDeviceError::NonPositiveSize);
        }
        let stride = width as u32 * format.bpp as u32;
        let Some(stride) = stride.checked_next_multiple_of(LINEAR_STRIDE_ALIGN as u32) else {
            return Err(CopyDeviceError::SizeOverflow);
        };
        let Some(size) = (stride as u64).checked_mul(height as u64) else {
            return Err(CopyDeviceError::SizeOverflow);
        };
        let dev = &self.dev.dev;
        let buffer = {
            let mut external_info = ExternalMemoryBufferCreateInfo::default()
                .handle_types(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let info = BufferCreateInfo::default()
                .size(size)
                .usage(BufferUsageFlags::TRANSFER_SRC | BufferUsageFlags::TRANSFER_DST)
                .sharing_mode(SharingMode::EXCLUSIVE)
                .push_next(&mut external_info);
            unsafe {
                dev.create_buffer(&info, None)
                    .map_err(CopyDeviceError::CreateBuffer)?
            }
        };
        let _destroy_buffer = on_drop(|| unsafe { dev.destroy_buffer(buffer, None) });
        let memory = {
            let memory_requirements = unsafe { dev.get_buffer_memory_requirements(buffer) };
            let required_flags =
                MemoryPropertyFlags::DEVICE_LOCAL | MemoryPropertyFlags::HOST_VISIBLE;
            let index = 'index: {
                for (idx, ty) in self.dev.phy.memory_types.iter().enumerate() {
                    if memory_requirements.memory_type_bits & (1 << idx) != 0
                        && ty.property_flags.contains(required_flags)
                    {
                        break 'index idx;
                    }
                }
                return Err(CopyDeviceError::NoMemoryTypeForAllocation);
            };
            let mut dedicated_allocation = MemoryDedicatedAllocateInfo::default().buffer(buffer);
            let mut external_memory = ExportMemoryAllocateInfo::default()
                .handle_types(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            let info = MemoryAllocateInfo::default()
                .allocation_size(memory_requirements.size)
                .memory_type_index(index as _)
                .push_next(&mut external_memory)
                .push_next(&mut dedicated_allocation);
            unsafe {
                dev.allocate_memory(&info, None)
                    .map_err(CopyDeviceError::AllocateMemory)?
            }
        };
        let free_memory = on_drop(|| unsafe { dev.free_memory(memory, None) });
        let fd = {
            let info = MemoryGetFdInfoKHR::default()
                .memory(memory)
                .handle_type(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
            unsafe {
                self.dev
                    .external_memory_fd
                    .get_memory_fd(&info)
                    .map_err(CopyDeviceError::ExportDmabuf)?
            }
        };
        let fd = Rc::new(OwnedFd::new(fd));
        let mut dmabuf = DmaBuf {
            id: dma_buf_ids.next(),
            width,
            height,
            format,
            modifier: LINEAR_MODIFIER,
            planes: Default::default(),
            is_disjoint: Default::default(),
        };
        dmabuf.planes.push(DmaBufPlane {
            offset: 0,
            stride,
            fd,
        });
        free_memory.forget();
        Ok(CopyDeviceBuffer {
            device: self.dev.clone(),
            memory,
            dmabuf,
        })
    }
}

impl CopyDeviceInner {
    fn wait_idle(&self) {
        log::warn!("Blocking");
        let res = unsafe { self.dev.device_wait_idle() };
        if let Err(e) = res {
            log::error!("Could not wait for device idle: {}", ErrorFmt(e));
            log::error!("This is unsound.");
        }
        for submissions in self.submissions.values() {
            submissions.pending.clear();
        }
    }

    fn create_semaphore(self: &Rc<Self>) -> Result<VulkanSemaphore, CopyDeviceError> {
        let create_info = SemaphoreCreateInfo::default();
        let semaphore = unsafe {
            self.dev
                .create_semaphore(&create_info, None)
                .map_err(CopyDeviceError::CreateSemaphore)?
        };
        Ok(VulkanSemaphore {
            dev: self.clone(),
            semaphore,
        })
    }

    fn create_fence(self: &Rc<Self>) -> Result<VulkanFence, CopyDeviceError> {
        let mut export_info =
            ExportFenceCreateInfo::default().handle_types(ExternalFenceHandleTypeFlags::SYNC_FD);
        let create_info = FenceCreateInfo::default().push_next(&mut export_info);
        let fence = unsafe {
            self.dev
                .create_fence(&create_info, None)
                .map_err(CopyDeviceError::CreateFence)?
        };
        Ok(VulkanFence {
            dev: self.clone(),
            fence,
        })
    }
}

impl CopyDeviceCopy {
    fn ensure_not_busy(&self) -> Result<(), CopyDeviceError> {
        let slf = &*self.inner;
        let Some(busy) = slf.busy.get() else {
            return Ok(());
        };
        let mut pollfd = c::pollfd {
            fd: busy.raw(),
            events: c::POLLIN,
            revents: 0,
        };
        let res = uapi::poll(slice::from_mut(&mut pollfd), 0);
        if res != Ok(1) {
            return Err(CopyDeviceError::Busy);
        }
        slf.busy.take();
        Ok(())
    }

    pub fn execute(
        &self,
        sync_file: Option<&SyncFile>,
        region: Option<&Region>,
    ) -> Result<Option<SyncFile>, CopyDeviceError> {
        self.ensure_not_busy()?;
        let slf = &*self.inner;
        let tt = slf.tt;
        let dev = &slf.dev.dev;
        let cmd = slf.command_buffer;
        let queue_family = slf.dev.phy.queues[tt].family;
        let region_buf;
        let width = slf.width;
        let height = slf.height;
        let region = match region {
            Some(r) => r,
            _ => {
                region_buf = Region::new(Rect::new_saturating(0, 0, width as i32, height as i32));
                &region_buf
            }
        };
        let (x_mask, y_mask) = slf.dev.phy.queues[tt].transfer_granularity_mask;
        let rects = &mut *slf.dev.phy.rects.borrow_mut();
        rects.clear();
        for rect in region.iter() {
            let x1 = (rect.x1().max(0) as u32 & !x_mask).min(width);
            let y1 = (rect.y1().max(0) as u32 & !y_mask).min(height);
            let x2 = ((rect.x2().max(0) as u32 + x_mask) & !x_mask).min(width);
            let y2 = ((rect.y2().max(0) as u32 + y_mask) & !y_mask).min(height);
            let width = x2 - x1;
            let height = y2 - y1;
            if width == 0 || height == 0 {
                continue;
            }
            rects.push((x1 as i32, y1 as i32, width, height));
        }
        if rects.is_empty() {
            return Ok(None);
        }
        let begin_info =
            CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            dev.begin_command_buffer(cmd, &begin_info)
                .map_err(CopyDeviceError::BeginCommandBuffer)?;
        }
        macro_rules! initial_buffer_barriers {
            ($($buf:expr, $access:expr;)*) => {
                [$(
                    BufferMemoryBarrier2::default()
                        .dst_stage_mask(PipelineStageFlags2::TRANSFER)
                        .dst_access_mask($access)
                        .src_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                        .dst_queue_family_index(queue_family)
                        .buffer($buf.buf)
                        .size(WHOLE_SIZE),
                )*]
            };
        }
        macro_rules! final_buffer_barriers {
            ($($buf:expr, $access:expr;)*) => {
                [$(
                    BufferMemoryBarrier2::default()
                        .src_stage_mask(PipelineStageFlags2::TRANSFER)
                        .src_access_mask($access)
                        .src_queue_family_index(queue_family)
                        .dst_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                        .buffer($buf.buf)
                        .size(WHOLE_SIZE),
                )*]
            };
        }
        let image_subresource_range = ImageSubresourceRange {
            aspect_mask: ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        };
        let image_subresource = ImageSubresourceLayers {
            aspect_mask: ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        };
        macro_rules! initial_image_barriers {
            ($($img:expr, $layout:expr, $access:expr;)*) => {
                [$(
                    ImageMemoryBarrier2::default()
                        .dst_stage_mask(PipelineStageFlags2::TRANSFER)
                        .dst_access_mask($access)
                        .old_layout(ImageLayout::GENERAL)
                        .new_layout(ImageLayout::GENERAL)
                        .src_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                        .dst_queue_family_index(queue_family)
                        .image($img.img)
                        .subresource_range(image_subresource_range),
                    ImageMemoryBarrier2::default()
                        .src_stage_mask(PipelineStageFlags2::TRANSFER)
                        .src_access_mask($access)
                        .dst_stage_mask(PipelineStageFlags2::TRANSFER)
                        .dst_access_mask($access)
                        .old_layout(ImageLayout::GENERAL)
                        .new_layout($layout)
                        .src_queue_family_index(queue_family)
                        .dst_queue_family_index(queue_family)
                        .image($img.img)
                        .subresource_range(image_subresource_range),
                )*]
            };
        }
        macro_rules! final_image_barriers {
            ($($img:expr, $layout:expr, $access:expr;)*) => {
                [$(
                    ImageMemoryBarrier2::default()
                        .src_stage_mask(PipelineStageFlags2::TRANSFER)
                        .src_access_mask($access)
                        .dst_stage_mask(PipelineStageFlags2::TRANSFER)
                        .dst_access_mask($access)
                        .old_layout($layout)
                        .new_layout(ImageLayout::GENERAL)
                        .src_queue_family_index(queue_family)
                        .dst_queue_family_index(queue_family)
                        .image($img.img)
                        .subresource_range(image_subresource_range),
                    ImageMemoryBarrier2::default()
                        .src_stage_mask(PipelineStageFlags2::TRANSFER)
                        .src_access_mask($access)
                        .old_layout(ImageLayout::GENERAL)
                        .new_layout(ImageLayout::GENERAL)
                        .src_queue_family_index(queue_family)
                        .dst_queue_family_index(QUEUE_FAMILY_FOREIGN_EXT)
                        .image($img.img)
                        .subresource_range(image_subresource_range),
                )*]
            };
        }
        match &slf.ty {
            CopyDeviceCopyType::BufferToBuffer {
                src,
                dst,
                stride,
                bpp,
            } => {
                let regions = &mut *slf.dev.phy.buffer_copy_2.borrow_mut();
                regions.clear();
                let stride = *stride as u64;
                let bpp = *bpp as u64;
                for &mut (x, y, width, height) in rects {
                    let lo = y as u64 * stride + x as u64 * bpp;
                    let size = (height as u64 - 1) * stride + width as u64 * bpp;
                    let region = BufferCopy2::default()
                        .src_offset(lo)
                        .dst_offset(lo)
                        .size(size);
                    regions.push(region);
                }
                use AccessFlags2 as A;
                let initial_barriers = initial_buffer_barriers![
                    src, A::TRANSFER_READ;
                    dst, A::TRANSFER_WRITE;
                ];
                let final_barriers = final_buffer_barriers![
                    src, A::TRANSFER_READ;
                    dst, A::TRANSFER_WRITE;
                ];
                let initial_dependency_info =
                    DependencyInfo::default().buffer_memory_barriers(&initial_barriers);
                let final_dependency_info =
                    DependencyInfo::default().buffer_memory_barriers(&final_barriers);
                let copy_buffer_info = CopyBufferInfo2::default()
                    .src_buffer(src.buf)
                    .dst_buffer(dst.buf)
                    .regions(regions);
                unsafe {
                    dev.cmd_pipeline_barrier2(cmd, &initial_dependency_info);
                    dev.cmd_copy_buffer2(cmd, &copy_buffer_info);
                    dev.cmd_pipeline_barrier2(cmd, &final_dependency_info);
                }
            }
            CopyDeviceCopyType::BufferToImage {
                buf,
                buf_format,
                buf_stride,
                img,
            }
            | CopyDeviceCopyType::ImageToBuffer {
                img,
                buf,
                buf_format,
                buf_stride,
            } => {
                let regions = &mut *slf.dev.phy.buffer_image_copy_2.borrow_mut();
                regions.clear();
                for &mut (x, y, width, height) in rects {
                    let offset = y as u64 * *buf_stride as u64 + x as u64 * buf_format.bpp as u64;
                    let region = BufferImageCopy2::default()
                        .buffer_offset(offset)
                        .buffer_row_length(*buf_stride / buf_format.bpp)
                        .buffer_image_height(slf.height)
                        .image_subresource(image_subresource)
                        .image_offset(Offset3D { x, y, z: 0 })
                        .image_extent(Extent3D {
                            width,
                            height,
                            depth: 1,
                        });
                    regions.push(region);
                }
                let buffer_to_image = match &slf.ty {
                    CopyDeviceCopyType::BufferToImage { .. } => true,
                    CopyDeviceCopyType::ImageToBuffer { .. } => false,
                    _ => unreachable!(),
                };
                let image_access_mask;
                let image_layout;
                let buffer_access_mask;
                match buffer_to_image {
                    true => {
                        image_access_mask = AccessFlags2::TRANSFER_WRITE;
                        image_layout = ImageLayout::TRANSFER_DST_OPTIMAL;
                        buffer_access_mask = AccessFlags2::TRANSFER_READ;
                    }
                    false => {
                        image_access_mask = AccessFlags2::TRANSFER_READ;
                        image_layout = ImageLayout::TRANSFER_SRC_OPTIMAL;
                        buffer_access_mask = AccessFlags2::TRANSFER_WRITE;
                    }
                }
                let initial_image_barriers = initial_image_barriers![
                    img, image_layout, image_access_mask;
                ];
                let final_image_barriers = final_image_barriers![
                    img, image_layout, image_access_mask;
                ];
                let initial_buffer_barriers = initial_buffer_barriers![
                    buf, buffer_access_mask;
                ];
                let final_buffer_barriers = final_buffer_barriers![
                    buf, buffer_access_mask;
                ];
                let initial_dependency_info = DependencyInfo::default()
                    .buffer_memory_barriers(&initial_buffer_barriers)
                    .image_memory_barriers(&initial_image_barriers);
                let final_dependency_info = DependencyInfo::default()
                    .buffer_memory_barriers(&final_buffer_barriers)
                    .image_memory_barriers(&final_image_barriers);
                unsafe {
                    dev.cmd_pipeline_barrier2(cmd, &initial_dependency_info);
                    match buffer_to_image {
                        true => {
                            let copy = CopyBufferToImageInfo2::default()
                                .src_buffer(buf.buf)
                                .dst_image(img.img)
                                .dst_image_layout(image_layout)
                                .regions(&regions);
                            dev.cmd_copy_buffer_to_image2(cmd, &copy);
                        }
                        false => {
                            let copy = CopyImageToBufferInfo2::default()
                                .src_image(img.img)
                                .src_image_layout(image_layout)
                                .dst_buffer(buf.buf)
                                .regions(&regions);
                            dev.cmd_copy_image_to_buffer2(cmd, &copy);
                        }
                    }
                    dev.cmd_pipeline_barrier2(cmd, &final_dependency_info);
                }
            }
            CopyDeviceCopyType::ImageToImage { src, dst } => {
                let regions = &mut *slf.dev.phy.image_copy_2.borrow_mut();
                regions.clear();
                for &mut (x, y, width, height) in rects {
                    let region = ImageCopy2::default()
                        .src_subresource(image_subresource)
                        .src_offset(Offset3D { x, y, z: 0 })
                        .dst_subresource(image_subresource)
                        .dst_offset(Offset3D { x, y, z: 0 })
                        .extent(Extent3D {
                            width,
                            height,
                            depth: 1,
                        });
                    regions.push(region);
                }
                use {AccessFlags2 as A, ImageLayout as L};
                let initial_barriers = initial_image_barriers![
                    src, L::TRANSFER_SRC_OPTIMAL, A::TRANSFER_READ;
                    dst, L::TRANSFER_DST_OPTIMAL, A::TRANSFER_WRITE;
                ];
                let final_barriers = final_image_barriers![
                    src, L::TRANSFER_SRC_OPTIMAL, A::TRANSFER_READ;
                    dst, L::TRANSFER_DST_OPTIMAL, A::TRANSFER_WRITE;
                ];
                let initial_dependency_info =
                    DependencyInfo::default().image_memory_barriers(&initial_barriers);
                let final_dependency_info =
                    DependencyInfo::default().image_memory_barriers(&final_barriers);
                let copy_image_info = CopyImageInfo2::default()
                    .src_image(src.img)
                    .src_image_layout(L::TRANSFER_SRC_OPTIMAL)
                    .dst_image(dst.img)
                    .dst_image_layout(L::TRANSFER_DST_OPTIMAL)
                    .regions(regions);
                unsafe {
                    dev.cmd_pipeline_barrier2(cmd, &initial_dependency_info);
                    dev.cmd_copy_image2(cmd, &copy_image_info);
                    dev.cmd_pipeline_barrier2(cmd, &final_dependency_info);
                }
            }
            CopyDeviceCopyType::Blit { src, dst } => {
                let regions = &mut *slf.dev.phy.image_blit_2.borrow_mut();
                regions.clear();
                for &mut (x, y, width, height) in rects {
                    let x1 = x;
                    let y1 = y;
                    let x2 = x1 + width as i32;
                    let y2 = y1 + height as i32;
                    let offsets = [
                        Offset3D { x: x1, y: y1, z: 0 },
                        Offset3D { x: x2, y: y2, z: 1 },
                    ];
                    let region = ImageBlit2::default()
                        .src_subresource(image_subresource)
                        .src_offsets(offsets)
                        .dst_subresource(image_subresource)
                        .dst_offsets(offsets);
                    regions.push(region);
                }
                use {AccessFlags2 as A, ImageLayout as L};
                let initial_barriers = initial_image_barriers![
                    src, L::TRANSFER_SRC_OPTIMAL, A::TRANSFER_READ;
                    dst, L::TRANSFER_DST_OPTIMAL, A::TRANSFER_WRITE;
                ];
                let final_barriers = final_image_barriers![
                    src, L::TRANSFER_SRC_OPTIMAL, A::TRANSFER_READ;
                    dst, L::TRANSFER_DST_OPTIMAL, A::TRANSFER_WRITE;
                ];
                let initial_dependency_info =
                    DependencyInfo::default().image_memory_barriers(&initial_barriers);
                let final_dependency_info =
                    DependencyInfo::default().image_memory_barriers(&final_barriers);
                let blit_image_info = BlitImageInfo2::default()
                    .src_image(src.img)
                    .src_image_layout(L::TRANSFER_SRC_OPTIMAL)
                    .dst_image(dst.img)
                    .dst_image_layout(L::TRANSFER_DST_OPTIMAL)
                    .regions(regions)
                    .filter(Filter::NEAREST);
                unsafe {
                    dev.cmd_pipeline_barrier2(cmd, &initial_dependency_info);
                    dev.cmd_blit_image2(cmd, &blit_image_info);
                    dev.cmd_pipeline_barrier2(cmd, &final_dependency_info);
                }
            }
        };
        unsafe {
            dev.end_command_buffer(cmd)
                .map_err(CopyDeviceError::EndCommandBuffer)?;
        }
        let mut wait_semaphore = None;
        let mut wait_semaphores = ArrayVec::<_, 1>::new();
        if let Some(sync_file) = sync_file {
            let semaphore = match slf.dev.semaphores.pop() {
                Some(s) => s,
                _ => slf.dev.create_semaphore()?,
            };
            semaphore.import(sync_file)?;
            let info = SemaphoreSubmitInfo::default()
                .semaphore(semaphore.semaphore)
                .stage_mask(PipelineStageFlags2::TRANSFER);
            wait_semaphores.push(info);
            wait_semaphore = Some(semaphore);
        }
        let signal_fence = match slf.dev.fences.pop() {
            Some(s) => s,
            _ => slf.dev.create_fence()?,
        };
        let command_buffer_info = CommandBufferSubmitInfo::default().command_buffer(cmd);
        let submit_info = SubmitInfo2::default()
            .command_buffer_infos(slice::from_ref(&command_buffer_info))
            .wait_semaphore_infos(&wait_semaphores);
        unsafe {
            slf.dev
                .dev
                .queue_submit2(
                    slf.dev.queues[tt],
                    slice::from_ref(&submit_info),
                    signal_fence.fence,
                )
                .map_err(CopyDeviceError::SubmitCopy)?;
        }
        let sync_file = match signal_fence.export() {
            Ok(f) => f,
            Err(e) => {
                log::error!("Could not export signal fence: {}", ErrorFmt(e));
                slf.dev.wait_idle();
                None
            }
        };
        slf.busy.set(sync_file.clone());
        let pending = Pending {
            dev: slf.dev.clone(),
            sync_file: sync_file.clone(),
            copy: self.inner.clone(),
            semaphore: wait_semaphore,
            fence: Some(signal_fence),
        };
        slf.dev.submissions[tt].pending.push(pending);
        Ok(sync_file)
    }
}

impl VulkanSemaphore {
    fn import(&self, sync_file: &OwnedFd) -> Result<(), CopyDeviceError> {
        let fd = uapi::fcntl_dupfd_cloexec(sync_file.raw(), 0)
            .map_err(Into::into)
            .map_err(CopyDeviceError::DupSyncFile)?;
        let info = ImportSemaphoreFdInfoKHR::default()
            .flags(SemaphoreImportFlags::TEMPORARY)
            .semaphore(self.semaphore)
            .handle_type(ExternalSemaphoreHandleTypeFlags::SYNC_FD)
            .fd(fd.raw());
        unsafe {
            self.dev
                .external_semaphore_fd
                .import_semaphore_fd(&info)
                .map_err(CopyDeviceError::ImportSyncFile)?;
        }
        let _ = fd.unwrap();
        Ok(())
    }
}

impl VulkanFence {
    fn export(&self) -> Result<Option<SyncFile>, CopyDeviceError> {
        let info = FenceGetFdInfoKHR::default()
            .fence(self.fence)
            .handle_type(ExternalFenceHandleTypeFlags::SYNC_FD);
        let fd = unsafe {
            self.dev
                .external_fence_fd
                .get_fence_fd(&info)
                .map_err(CopyDeviceError::ExportSyncFile)?
        };
        let fd = if fd == -1 {
            None
        } else {
            Some(SyncFile(Rc::new(OwnedFd::new(fd))))
        };
        Ok(fd)
    }
}

impl CopyDeviceRegistry {
    pub fn new(ring: &Rc<IoUring>, eng: &Rc<AsyncEngine>) -> Self {
        Self {
            ring: ring.clone(),
            eng: eng.clone(),
            devs: Default::default(),
        }
    }

    pub fn remove(&self, dev: c::dev_t) {
        self.devs.remove(&dev);
    }

    pub fn get(&self, dev: c::dev_t) -> Option<Rc<PhysicalCopyDevice>> {
        if let Some(dev) = self.devs.get(&dev) {
            return dev;
        }
        match PhysicalCopyDevice::new(&self.ring, &self.eng, dev).map(Some) {
            Ok(cd) => {
                self.devs.set(dev, cd.clone());
                cd
            }
            Err(e) => {
                let maj = uapi::major(dev);
                let min = uapi::minor(dev);
                log::warn!(
                    "Could not create physical copy device for {maj}:{min}: {}",
                    ErrorFmt(e),
                );
                self.devs.set(dev, None);
                None
            }
        }
    }
}

impl Drop for VulkanSemaphore {
    fn drop(&mut self) {
        unsafe {
            self.dev.dev.destroy_semaphore(self.semaphore, None);
        }
    }
}

impl Drop for VulkanFence {
    fn drop(&mut self) {
        unsafe {
            self.dev.dev.destroy_fence(self.fence, None);
        }
    }
}

impl Drop for CopyDeviceCopyInner {
    fn drop(&mut self) {
        unsafe {
            self.dev.dev.free_command_buffers(
                self.dev.pools[self.tt],
                slice::from_ref(&self.command_buffer),
            );
        }
    }
}

impl Drop for CopyDeviceInner {
    fn drop(&mut self) {
        unsafe {
            for &pool in &self.unique_pools {
                self.dev.destroy_command_pool(pool, None);
            }
            self.dev.destroy_device(None);
        }
    }
}

impl Drop for CopyDevice {
    fn drop(&mut self) {
        let dev = &self.dev;
        let has_pending = dev
            .submissions
            .values()
            .any(|s| s.task_has_pending.get() || s.pending.is_not_empty());
        if has_pending {
            dev.wait_idle();
        }
        dev.semaphores.take();
        dev.fences.take();
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(v) = self.semaphore.take() {
            self.dev.semaphores.push(v);
        }
        if let Some(v) = self.fence.take() {
            self.dev.fences.push(v);
        }
        if self.copy.busy.get() == self.sync_file {
            self.copy.busy.take();
        }
    }
}

impl CopyDeviceBuffer {
    pub fn dmabuf(&self) -> &DmaBuf {
        &self.dmabuf
    }
}

impl Drop for CopyDeviceBuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.dev.free_memory(self.memory, None);
        }
    }
}

impl Debug for CopyDeviceBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CopyDeviceBuffer").finish_non_exhaustive()
    }
}

impl Debug for PhysicalCopyDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhysicalCopyDevice").finish_non_exhaustive()
    }
}

impl Debug for CopyDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CopyDevice").finish_non_exhaustive()
    }
}

impl Debug for CopyDeviceCopy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CopyDeviceCopy").finish_non_exhaustive()
    }
}

impl Drop for VulkanBuffer {
    fn drop(&mut self) {
        let dev = &self.dev.dev;
        unsafe {
            dev.destroy_buffer(self.buf, None);
            dev.free_memory(self.mem, None);
        }
    }
}

impl Drop for VulkanImage {
    fn drop(&mut self) {
        let dev = &self.dev.dev;
        unsafe {
            dev.destroy_image(self.img, None);
            for &mem in &self.mem {
                dev.free_memory(mem, None);
            }
        }
    }
}

impl Deref for CopyDevice {
    type Target = Rc<PhysicalCopyDevice>;

    fn deref(&self) -> &Self::Target {
        &self.dev.phy
    }
}

type QueueInfo = (u32, (u32, u32), u32);

fn allocate_queues(
    gfx: QueueInfo,
    compute_only: Option<QueueInfo>,
    transfer_only: Option<QueueInfo>,
) -> (Vec<QueueToAllocate>, KeyedCopy<QueueIndex>) {
    let intra = compute_only.unwrap_or(gfx);
    let cross = transfer_only.unwrap_or(intra);
    let mut distinct_families = AHashSet::default();
    distinct_families.insert(cross);
    distinct_families.insert(intra);
    distinct_families.insert(gfx);
    let mut queues_to_allocate = vec![];
    macro_rules! index {
        ($qi:expr, $within:expr) => {
            QueueIndex {
                allocate_idx: queues_to_allocate.len(),
                family: $qi.0,
                idx_within_family: $within as u32,
                transfer_granularity_mask: $qi.1,
            }
        };
    }
    macro_rules! alloc {
        ($qi:expr, $num:expr) => {
            QueueToAllocate {
                family: $qi.0,
                num: $num as usize,
            }
        };
    }
    let (blit, intra_idx, download, upload);
    if distinct_families.len() == 3 {
        let num_cross = cross.2.min(2) as usize;
        blit = index!(gfx, 0);
        queues_to_allocate.push(alloc!(gfx, 1));
        intra_idx = index!(intra, 0);
        queues_to_allocate.push(alloc!(intra, 1));
        download = index!(cross, 0);
        upload = index!(cross, num_cross - 1);
        queues_to_allocate.push(alloc!(cross, num_cross));
    } else if distinct_families.len() == 1 {
        let qi = cross;
        let num = qi.2.min(4);
        match num {
            1 => {
                blit = index!(qi, 0);
                intra_idx = index!(qi, 0);
                download = index!(qi, 0);
                upload = index!(qi, 0);
            }
            2 => {
                blit = index!(qi, 0);
                intra_idx = index!(qi, 0);
                download = index!(qi, 0);
                upload = index!(qi, 1);
            }
            3 => {
                blit = index!(qi, 0);
                intra_idx = index!(qi, 0);
                download = index!(qi, 1);
                upload = index!(qi, 2);
            }
            4 => {
                blit = index!(qi, 0);
                intra_idx = index!(qi, 1);
                download = index!(qi, 2);
                upload = index!(qi, 3);
            }
            _ => unreachable!(),
        }
        queues_to_allocate.push(alloc!(qi, num));
    } else {
        if gfx == intra {
            let num_gfx = gfx.2.min(2);
            blit = index!(gfx, 0);
            intra_idx = index!(gfx, num_gfx - 1);
            queues_to_allocate.push(alloc!(gfx, num_gfx));
            let num_cross = cross.2.min(2);
            download = index!(cross, 0);
            upload = index!(cross, num_cross - 1);
            queues_to_allocate.push(alloc!(cross, num_cross));
        } else {
            // if cross == gfx then intra == gfx
            assert_eq!(intra, cross);
            blit = index!(gfx, 0);
            queues_to_allocate.push(alloc!(gfx, 1));
            let num_intra = intra.2.min(3);
            match num_intra {
                1 => {
                    intra_idx = index!(intra, 0);
                    download = index!(intra, 0);
                    upload = index!(intra, 0);
                }
                2 => {
                    intra_idx = index!(intra, 0);
                    download = index!(intra, 0);
                    upload = index!(intra, 1);
                }
                3 => {
                    intra_idx = index!(intra, 0);
                    download = index!(intra, 1);
                    upload = index!(intra, 2);
                }
                _ => unreachable!(),
            }
            queues_to_allocate.push(alloc!(intra, num_intra));
        }
    }
    let queue_indices = static_copy_map! {
        TransferType::Blit => blit,
        TransferType::Intra => intra_idx,
        TransferType::Download => download,
        TransferType::Upload => upload,
    };
    (queues_to_allocate, queue_indices)
}
