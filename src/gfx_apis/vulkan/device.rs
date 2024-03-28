use {
    crate::{
        format::XRGB8888,
        gfx_apis::vulkan::{
            format::VulkanFormat,
            instance::{
                map_extension_properties, ApiVersionDisplay, Extensions, VulkanInstance,
                API_VERSION,
            },
            util::OnDrop,
            VulkanError,
        },
        video::{
            drm::{sync_obj::SyncObjCtx, Drm},
            gbm::GbmDevice,
        },
    },
    ahash::AHashMap,
    arrayvec::ArrayVec,
    ash::{
        extensions::khr::{ExternalFenceFd, ExternalMemoryFd, ExternalSemaphoreFd, PushDescriptor},
        vk::{
            DeviceCreateInfo, DeviceMemory, DeviceQueueCreateInfo, ExtExternalMemoryDmaBufFn,
            ExtImageDrmFormatModifierFn, ExtPhysicalDeviceDrmFn, ExtQueueFamilyForeignFn,
            ExternalSemaphoreFeatureFlags, ExternalSemaphoreHandleTypeFlags,
            ExternalSemaphoreProperties, KhrDriverPropertiesFn, KhrExternalFenceFdFn,
            KhrExternalMemoryFdFn, KhrExternalSemaphoreFdFn, KhrPushDescriptorFn,
            MemoryPropertyFlags, MemoryType, PhysicalDevice, PhysicalDeviceDriverProperties,
            PhysicalDeviceDriverPropertiesKHR, PhysicalDeviceDrmPropertiesEXT,
            PhysicalDeviceDynamicRenderingFeatures, PhysicalDeviceExternalSemaphoreInfo,
            PhysicalDeviceProperties, PhysicalDeviceProperties2,
            PhysicalDeviceSynchronization2Features, PhysicalDeviceTimelineSemaphoreFeatures, Queue,
            QueueFlags, MAX_MEMORY_TYPES,
        },
        Device,
    },
    isnt::std_1::collections::IsntHashMap2Ext,
    std::{
        ffi::{CStr, CString},
        rc::Rc,
    },
    uapi::Ustr,
};

pub struct VulkanDevice {
    pub(super) physical_device: PhysicalDevice,
    pub(super) render_node: Rc<CString>,
    pub(super) gbm: GbmDevice,
    pub(super) sync_ctx: Rc<SyncObjCtx>,
    pub(super) instance: Rc<VulkanInstance>,
    pub(super) device: Device,
    pub(super) external_memory_fd: ExternalMemoryFd,
    pub(super) external_semaphore_fd: ExternalSemaphoreFd,
    pub(super) external_fence_fd: ExternalFenceFd,
    pub(super) push_descriptor: PushDescriptor,
    pub(super) formats: AHashMap<u32, VulkanFormat>,
    pub(super) memory_types: ArrayVec<MemoryType, MAX_MEMORY_TYPES>,
    pub(super) graphics_queue: Queue,
    pub(super) graphics_queue_idx: u32,
}

impl Drop for VulkanDevice {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
        }
    }
}

impl VulkanDevice {
    pub(super) fn find_memory_type(
        &self,
        flags: MemoryPropertyFlags,
        memory_type_bits: u32,
    ) -> Option<u32> {
        for (idx, ty) in self.memory_types.iter().enumerate() {
            if memory_type_bits & (1 << idx as u32) != 0 {
                if ty.property_flags.contains(flags) {
                    return Some(idx as _);
                }
            }
        }
        None
    }
}

struct FreeMem<'a>(&'a Device, DeviceMemory);

impl<'a> Drop for FreeMem<'a> {
    fn drop(&mut self) {
        unsafe {
            self.0.free_memory(self.1, None);
        }
    }
}

impl VulkanInstance {
    fn get_device_extensions(&self, phy_dev: PhysicalDevice) -> Result<Extensions, VulkanError> {
        unsafe {
            self.instance
                .enumerate_device_extension_properties(phy_dev)
                .map(map_extension_properties)
                .map_err(VulkanError::DeviceExtensions)
        }
    }

    fn find_dev(&self, drm: &Drm) -> Result<PhysicalDevice, VulkanError> {
        let stat = match uapi::fstat(drm.raw()) {
            Ok(s) => s,
            Err(e) => return Err(VulkanError::Fstat(e.into())),
        };
        let dev = stat.st_rdev;
        log::info!(
            "Searching for vulkan device with devnum {}:{}",
            uapi::major(dev),
            uapi::minor(dev)
        );
        let phy_devs = unsafe { self.instance.enumerate_physical_devices() };
        let phy_devs = match phy_devs {
            Ok(d) => d,
            Err(e) => return Err(VulkanError::EnumeratePhysicalDevices(e)),
        };
        let mut devices = vec![];
        for phy_dev in phy_devs {
            let props = unsafe { self.instance.get_physical_device_properties(phy_dev) };
            if props.api_version < API_VERSION {
                devices.push((props, None, None));
                continue;
            }
            let extensions = match self.get_device_extensions(phy_dev) {
                Ok(e) => e,
                Err(e) => {
                    log::error!(
                        "Could not enumerate extensions of device with id {}: {:#}",
                        props.device_id,
                        e
                    );
                    devices.push((props, None, None));
                    continue;
                }
            };
            if !extensions.contains_key(ExtPhysicalDeviceDrmFn::name()) {
                devices.push((props, Some(extensions), None));
                continue;
            }
            let has_driver_props = extensions.contains_key(KhrDriverPropertiesFn::name());
            let mut drm_props = PhysicalDeviceDrmPropertiesEXT::builder().build();
            let mut driver_props = PhysicalDeviceDriverPropertiesKHR::builder().build();
            let mut props2 = PhysicalDeviceProperties2::builder().push_next(&mut drm_props);
            if has_driver_props {
                props2 = props2.push_next(&mut driver_props);
            }
            unsafe {
                self.instance
                    .get_physical_device_properties2(phy_dev, &mut props2);
            }
            let primary_dev =
                uapi::makedev(drm_props.primary_major as _, drm_props.primary_minor as _);
            let render_dev =
                uapi::makedev(drm_props.render_major as _, drm_props.render_minor as _);
            if primary_dev == dev || render_dev == dev {
                log::info!("Device with id {} matches", props.device_id);
                log_device(&props, Some(&extensions), Some(&driver_props));
                return Ok(phy_dev);
            }
            devices.push((props, Some(extensions), Some(driver_props)));
        }
        if devices.is_empty() {
            log::warn!("Found no devices");
        } else {
            log::warn!("Found the following devices but none matches:");
            for (props, extensions, driver_props) in devices.iter() {
                log::warn!("Found the following devices but none matches:");
                log::warn!("-----");
                log_device(props, extensions.as_ref(), driver_props.as_ref());
            }
        }
        Err(VulkanError::NoDeviceFound(dev))
    }

    fn find_graphics_queue(&self, phy_dev: PhysicalDevice) -> Result<u32, VulkanError> {
        let props = unsafe {
            self.instance
                .get_physical_device_queue_family_properties(phy_dev)
        };
        props
            .iter()
            .position(|p| p.queue_flags.contains(QueueFlags::GRAPHICS))
            .map(|v| v as _)
            .ok_or(VulkanError::NoGraphicsQueue)
    }

    fn supports_semaphore_import(&self, phy_dev: PhysicalDevice) -> bool {
        let mut props = ExternalSemaphoreProperties::builder().build();
        let info = PhysicalDeviceExternalSemaphoreInfo::builder()
            .handle_type(ExternalSemaphoreHandleTypeFlags::OPAQUE_FD)
            .build();
        unsafe {
            self.instance
                .get_physical_device_external_semaphore_properties(phy_dev, &info, &mut props);
        }
        props
            .external_semaphore_features
            .contains(ExternalSemaphoreFeatureFlags::IMPORTABLE)
    }

    pub fn create_device(self: &Rc<Self>, drm: &Drm) -> Result<Rc<VulkanDevice>, VulkanError> {
        let render_node = drm
            .get_render_node()
            .map_err(VulkanError::FetchRenderNode)?
            .ok_or(VulkanError::NoRenderNode)
            .map(Rc::new)?;
        let gbm = GbmDevice::new(drm).map_err(VulkanError::Gbm)?;
        let phy_dev = self.find_dev(drm)?;
        let extensions = self.get_device_extensions(phy_dev)?;
        for &ext in REQUIRED_DEVICE_EXTENSIONS {
            if extensions.not_contains_key(ext) {
                return Err(VulkanError::MissingDeviceExtension(ext));
            }
        }
        let graphics_queue_idx = self.find_graphics_queue(phy_dev)?;
        if !self.supports_semaphore_import(phy_dev) {
            return Err(VulkanError::SyncobjImport);
        }
        let enabled_extensions: Vec<_> = REQUIRED_DEVICE_EXTENSIONS
            .iter()
            .map(|n| n.as_ptr())
            .collect();
        let mut semaphore_features =
            PhysicalDeviceTimelineSemaphoreFeatures::builder().timeline_semaphore(true);
        let mut synchronization2_features =
            PhysicalDeviceSynchronization2Features::builder().synchronization2(true);
        let mut dynamic_rendering_features =
            PhysicalDeviceDynamicRenderingFeatures::builder().dynamic_rendering(true);
        let queue_create_info = DeviceQueueCreateInfo::builder()
            .queue_family_index(graphics_queue_idx)
            .queue_priorities(&[1.0])
            .build();
        let device_create_info = DeviceCreateInfo::builder()
            .push_next(&mut semaphore_features)
            .push_next(&mut synchronization2_features)
            .push_next(&mut dynamic_rendering_features)
            .queue_create_infos(std::slice::from_ref(&queue_create_info))
            .enabled_extension_names(&enabled_extensions);
        let device = unsafe {
            self.instance
                .create_device(phy_dev, &device_create_info, None)
        };
        let device = match device {
            Ok(d) => d,
            Err(e) => return Err(VulkanError::CreateDevice(e)),
        };
        let destroy_device = OnDrop(|| unsafe { device.destroy_device(None) });
        let formats = self.load_formats(phy_dev)?;
        let supports_xrgb8888 = formats
            .get(&XRGB8888.drm)
            .map(|f| {
                let mut supports_rendering = false;
                let mut supports_texturing = false;
                f.modifiers.values().for_each(|v| {
                    supports_rendering |= v.render_max_extents.is_some();
                    supports_texturing |= v.texture_max_extents.is_some();
                });
                supports_rendering && supports_texturing
            })
            .unwrap_or(false);
        if !supports_xrgb8888 {
            return Err(VulkanError::XRGB8888);
        }
        destroy_device.forget();
        let external_memory_fd = ExternalMemoryFd::new(&self.instance, &device);
        let external_semaphore_fd = ExternalSemaphoreFd::new(&self.instance, &device);
        let external_fence_fd = ExternalFenceFd::new(&self.instance, &device);
        let push_descriptor = PushDescriptor::new(&self.instance, &device);
        let memory_properties =
            unsafe { self.instance.get_physical_device_memory_properties(phy_dev) };
        let memory_types = memory_properties.memory_types
            [..memory_properties.memory_type_count as _]
            .iter()
            .copied()
            .collect();
        let graphics_queue = unsafe { device.get_device_queue(graphics_queue_idx, 0) };
        Ok(Rc::new(VulkanDevice {
            physical_device: phy_dev,
            render_node,
            sync_ctx: Rc::new(SyncObjCtx::new(gbm.drm.fd())),
            gbm,
            instance: self.clone(),
            device,
            external_memory_fd,
            external_semaphore_fd,
            external_fence_fd,
            push_descriptor,
            formats,
            memory_types,
            graphics_queue,
            graphics_queue_idx,
        }))
    }
}

const REQUIRED_DEVICE_EXTENSIONS: &[&CStr] = &[
    KhrExternalMemoryFdFn::name(),
    KhrExternalSemaphoreFdFn::name(),
    KhrExternalFenceFdFn::name(),
    ExtExternalMemoryDmaBufFn::name(),
    ExtQueueFamilyForeignFn::name(),
    ExtImageDrmFormatModifierFn::name(),
    KhrPushDescriptorFn::name(),
];

fn log_device(
    props: &PhysicalDeviceProperties,
    extensions: Option<&Extensions>,
    driver_props: Option<&PhysicalDeviceDriverProperties>,
) {
    log::info!("  api version: {}", ApiVersionDisplay(props.api_version));
    log::info!(
        "  driver version: {}",
        ApiVersionDisplay(props.driver_version)
    );
    log::info!("  vendor id: {}", props.vendor_id);
    log::info!("  device id: {}", props.device_id);
    log::info!("  device type: {:?}", props.device_type);
    unsafe {
        log::info!(
            "  device name: {}",
            Ustr::from_ptr(props.device_name.as_ptr()).display()
        );
    }
    if props.api_version < API_VERSION {
        log::warn!("  device does not support vulkan 1.3");
    }
    if let Some(extensions) = extensions {
        if !extensions.contains_key(ExtPhysicalDeviceDrmFn::name()) {
            log::warn!("  device does support not the VK_EXT_physical_device_drm extension");
        }
    }
    if let Some(driver_props) = driver_props {
        unsafe {
            log::info!(
                "  driver: {} ({})",
                Ustr::from_ptr(driver_props.driver_name.as_ptr()).display(),
                Ustr::from_ptr(driver_props.driver_info.as_ptr()).display()
            );
        }
    }
}
