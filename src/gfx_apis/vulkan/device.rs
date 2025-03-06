use {
    crate::{
        format::XRGB8888,
        gfx_apis::vulkan::{
            VulkanError,
            format::{VulkanBlendBufferLimits, VulkanFormat},
            instance::{
                API_VERSION, ApiVersionDisplay, Extensions, VulkanInstance,
                map_extension_properties,
            },
        },
        utils::on_drop::OnDrop,
        video::{
            drm::{Drm, sync_obj::SyncObjCtx},
            gbm::GbmDevice,
        },
    },
    ahash::AHashMap,
    arrayvec::ArrayVec,
    ash::{
        Device,
        ext::{
            descriptor_buffer, external_memory_dma_buf, image_drm_format_modifier,
            physical_device_drm, queue_family_foreign,
        },
        khr::{
            driver_properties, external_fence_fd, external_memory_fd, external_semaphore_fd,
            push_descriptor,
        },
        vk::{
            DeviceCreateInfo, DeviceQueueCreateInfo, DeviceSize, DriverId,
            ExternalSemaphoreFeatureFlags, ExternalSemaphoreHandleTypeFlags,
            ExternalSemaphoreProperties, MAX_MEMORY_TYPES, MemoryPropertyFlags, MemoryType,
            PhysicalDevice, PhysicalDeviceBufferDeviceAddressFeatures,
            PhysicalDeviceDescriptorBufferFeaturesEXT, PhysicalDeviceDescriptorBufferPropertiesEXT,
            PhysicalDeviceDriverProperties, PhysicalDeviceDriverPropertiesKHR,
            PhysicalDeviceDrmPropertiesEXT, PhysicalDeviceDynamicRenderingFeatures,
            PhysicalDeviceExternalSemaphoreInfo, PhysicalDeviceProperties,
            PhysicalDeviceProperties2, PhysicalDeviceSynchronization2Features,
            PhysicalDeviceTimelineSemaphoreFeatures, PhysicalDeviceVulkan12Properties, Queue,
            QueueFlags,
        },
    },
    isnt::std_1::collections::IsntHashMap2Ext,
    std::{
        ffi::{CStr, CString},
        rc::Rc,
        sync::Arc,
    },
    uapi::Ustr,
};

pub struct VulkanDevice {
    pub(super) physical_device: PhysicalDevice,
    pub(super) render_node: Rc<CString>,
    pub(super) gbm: Rc<GbmDevice>,
    pub(super) sync_ctx: Rc<SyncObjCtx>,
    pub(super) instance: Rc<VulkanInstance>,
    pub(super) device: Arc<Device>,
    pub(super) external_memory_fd: external_memory_fd::Device,
    pub(super) external_semaphore_fd: external_semaphore_fd::Device,
    pub(super) external_fence_fd: external_fence_fd::Device,
    pub(super) push_descriptor: push_descriptor::Device,
    pub(super) image_drm_format_modifier: image_drm_format_modifier::Device,
    pub(super) descriptor_buffer: Option<descriptor_buffer::Device>,
    pub(super) formats: AHashMap<u32, VulkanFormat>,
    pub(super) blend_limits: VulkanBlendBufferLimits,
    pub(super) memory_types: ArrayVec<MemoryType, MAX_MEMORY_TYPES>,
    pub(super) graphics_queue: Queue,
    pub(super) graphics_queue_idx: u32,
    pub(super) transfer_queue: Option<Queue>,
    pub(super) distinct_transfer_queue_family_idx: Option<u32>,
    pub(super) transfer_granularity_mask: (u32, u32),
    pub(super) descriptor_buffer_offset_mask: DeviceSize,
    pub(super) sampler_descriptor_size: usize,
    pub(super) sampled_image_descriptor_size: usize,
    pub(super) is_anv: bool,
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
        let dev = drm.dev();
        log::log!(
            self.log_level,
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
            if !extensions.contains_key(physical_device_drm::NAME) {
                devices.push((props, Some(extensions), None));
                continue;
            }
            let has_driver_props = extensions.contains_key(driver_properties::NAME);
            let mut drm_props = PhysicalDeviceDrmPropertiesEXT::default();
            let mut driver_props = PhysicalDeviceDriverPropertiesKHR::default();
            let mut props2 = PhysicalDeviceProperties2::default().push_next(&mut drm_props);
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
                log::log!(self.log_level, "Device with id {} matches", props.device_id);
                log_device(
                    self.log_level,
                    &props,
                    Some(&extensions),
                    Some(&driver_props),
                );
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
                log_device(
                    self.log_level,
                    props,
                    extensions.as_ref(),
                    driver_props.as_ref(),
                );
            }
        }
        Err(VulkanError::NoDeviceFound(dev))
    }

    fn find_queues(
        &self,
        phy_dev: PhysicalDevice,
    ) -> Result<(u32, Option<(u32, u32, u32)>), VulkanError> {
        let props = unsafe {
            self.instance
                .get_physical_device_queue_family_properties(phy_dev)
        };
        let gfx_queue = props
            .iter()
            .position(|p| p.queue_flags.contains(QueueFlags::GRAPHICS))
            .ok_or(VulkanError::NoGraphicsQueue)?;
        let transfer_queue = 'transfer: {
            let mut transfer_only = None;
            let mut compute_only = None;
            let mut separate_gfx = None;
            for (idx, props) in props.iter().enumerate() {
                if idx == gfx_queue {
                    continue;
                }
                let g = &props.min_image_transfer_granularity;
                if g.width == 0 || g.height == 0 {
                    continue;
                }
                let f = props.queue_flags;
                use QueueFlags as F;
                if !f.intersects(F::GRAPHICS | F::COMPUTE) && f.intersects(F::TRANSFER) {
                    transfer_only = Some(idx);
                } else if !f.intersects(F::GRAPHICS) && f.intersects(F::COMPUTE) {
                    compute_only = Some(idx);
                } else if f.intersects(F::GRAPHICS) {
                    separate_gfx = Some(idx);
                }
            }
            if let Some(idx) = transfer_only.or(compute_only).or(separate_gfx) {
                break 'transfer Some(idx);
            }
            if props[gfx_queue].queue_count > 1 {
                break 'transfer Some(gfx_queue);
            }
            None
        };
        let mut width_mask = 0;
        let mut height_mask = 0;
        if let Some(idx) = transfer_queue {
            let g = &props[idx].min_image_transfer_granularity;
            width_mask = g.width.wrapping_sub(1);
            height_mask = g.height.wrapping_sub(1);
        }
        Ok((
            gfx_queue as _,
            transfer_queue.map(|v| (v as _, width_mask, height_mask)),
        ))
    }

    fn supports_semaphore_import(&self, phy_dev: PhysicalDevice) -> bool {
        let mut props = ExternalSemaphoreProperties::default();
        let info = PhysicalDeviceExternalSemaphoreInfo::default()
            .handle_type(ExternalSemaphoreHandleTypeFlags::SYNC_FD);
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
        let supports_descriptor_buffer = extensions.contains_key(descriptor_buffer::NAME);
        if !supports_descriptor_buffer {
            log::warn!("Vulkan device does not support descriptor buffers");
        }
        let (graphics_queue_family_idx, transfer_queue_family) = self.find_queues(phy_dev)?;
        let mut distinct_transfer_queue_family_idx = None;
        let mut transfer_granularity_mask = (0, 0);
        if let Some((idx, width_mask, height_mask)) = transfer_queue_family {
            if idx != graphics_queue_family_idx {
                distinct_transfer_queue_family_idx = Some(idx);
            }
            transfer_granularity_mask = (width_mask, height_mask);
        }
        if !self.supports_semaphore_import(phy_dev) {
            return Err(VulkanError::SyncFileImport);
        }
        let mut enabled_extensions: Vec<_> = REQUIRED_DEVICE_EXTENSIONS
            .iter()
            .map(|n| n.as_ptr())
            .collect();
        if supports_descriptor_buffer {
            enabled_extensions.push(descriptor_buffer::NAME.as_ptr());
        }
        let mut semaphore_features =
            PhysicalDeviceTimelineSemaphoreFeatures::default().timeline_semaphore(true);
        let mut synchronization2_features =
            PhysicalDeviceSynchronization2Features::default().synchronization2(true);
        let mut dynamic_rendering_features =
            PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);
        let mut descriptor_buffer_features =
            PhysicalDeviceDescriptorBufferFeaturesEXT::default().descriptor_buffer(true);
        let mut buffer_device_address_features =
            PhysicalDeviceBufferDeviceAddressFeatures::default().buffer_device_address(true);
        let mut queue_create_infos = ArrayVec::<_, 2>::new();
        queue_create_infos.push(
            DeviceQueueCreateInfo::default()
                .queue_family_index(graphics_queue_family_idx)
                .queue_priorities(&[1.0]),
        );
        if let Some((tq, _, _)) = transfer_queue_family {
            queue_create_infos.push(
                DeviceQueueCreateInfo::default()
                    .queue_family_index(tq)
                    .queue_priorities(&[1.0]),
            );
        }
        let mut device_create_info = DeviceCreateInfo::default()
            .push_next(&mut semaphore_features)
            .push_next(&mut synchronization2_features)
            .push_next(&mut dynamic_rendering_features)
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&enabled_extensions);
        if supports_descriptor_buffer {
            device_create_info = device_create_info
                .push_next(&mut descriptor_buffer_features)
                .push_next(&mut buffer_device_address_features);
        }
        let device = unsafe {
            self.instance
                .create_device(phy_dev, &device_create_info, None)
        };
        let device = match device {
            Ok(d) => d,
            Err(e) => return Err(VulkanError::CreateDevice(e)),
        };
        let destroy_device = OnDrop(|| unsafe { device.destroy_device(None) });
        let blend_limits = self.load_blend_format_limits(phy_dev)?;
        let formats = self.load_formats(phy_dev)?;
        let supports_xrgb8888 = formats
            .get(&XRGB8888.drm)
            .map(|f| {
                let mut supports_rendering = false;
                let mut supports_texturing = false;
                f.modifiers.values().for_each(|v| {
                    supports_rendering |= v.render_limits.is_some();
                    supports_texturing |= v.texture_limits.is_some();
                });
                supports_rendering && supports_texturing
            })
            .unwrap_or(false);
        if !supports_xrgb8888 {
            return Err(VulkanError::XRGB8888);
        }
        destroy_device.forget();
        let external_memory_fd = external_memory_fd::Device::new(&self.instance, &device);
        let external_semaphore_fd = external_semaphore_fd::Device::new(&self.instance, &device);
        let external_fence_fd = external_fence_fd::Device::new(&self.instance, &device);
        let push_descriptor = push_descriptor::Device::new(&self.instance, &device);
        let image_drm_format_modifier =
            image_drm_format_modifier::Device::new(&self.instance, &device);
        let descriptor_buffer = supports_descriptor_buffer
            .then(|| descriptor_buffer::Device::new(&self.instance, &device));
        let mut descriptor_buffer_props = PhysicalDeviceDescriptorBufferPropertiesEXT::default();
        let mut physical_device_vulkan12_properties = PhysicalDeviceVulkan12Properties::default();
        let mut physical_device_properties2 = PhysicalDeviceProperties2::default()
            .push_next(&mut physical_device_vulkan12_properties);
        if supports_descriptor_buffer {
            physical_device_properties2 =
                physical_device_properties2.push_next(&mut descriptor_buffer_props);
        }
        unsafe {
            self.instance
                .get_physical_device_properties2(phy_dev, &mut physical_device_properties2);
        }
        let mut descriptor_buffer_offset_mask = 0;
        let mut sampler_descriptor_size = 0;
        let mut sampled_image_descriptor_size = 0;
        if supports_descriptor_buffer {
            descriptor_buffer_offset_mask = descriptor_buffer_props
                .descriptor_buffer_offset_alignment
                .checked_next_power_of_two()
                .unwrap()
                - 1;
            sampler_descriptor_size = descriptor_buffer_props.sampler_descriptor_size;
            sampled_image_descriptor_size = descriptor_buffer_props.sampled_image_descriptor_size;
        }
        let memory_properties =
            unsafe { self.instance.get_physical_device_memory_properties(phy_dev) };
        let memory_types = memory_properties.memory_types
            [..memory_properties.memory_type_count as _]
            .iter()
            .copied()
            .collect();
        let graphics_queue = unsafe { device.get_device_queue(graphics_queue_family_idx, 0) };
        let transfer_queue = transfer_queue_family.map(|(family_idx, _, _)| {
            let queue_idx = match family_idx == graphics_queue_family_idx {
                true => 1,
                false => 0,
            };
            unsafe { device.get_device_queue(family_idx, queue_idx) }
        });
        Ok(Rc::new(VulkanDevice {
            physical_device: phy_dev,
            render_node,
            sync_ctx: Rc::new(SyncObjCtx::new(gbm.drm.fd())),
            gbm: Rc::new(gbm),
            instance: self.clone(),
            device: Arc::new(device),
            external_memory_fd,
            external_semaphore_fd,
            external_fence_fd,
            push_descriptor,
            image_drm_format_modifier,
            descriptor_buffer,
            formats,
            memory_types,
            graphics_queue,
            graphics_queue_idx: graphics_queue_family_idx,
            transfer_queue,
            distinct_transfer_queue_family_idx,
            transfer_granularity_mask,
            descriptor_buffer_offset_mask,
            sampler_descriptor_size,
            sampled_image_descriptor_size,
            blend_limits,
            is_anv: physical_device_vulkan12_properties.driver_id
                == DriverId::INTEL_OPEN_SOURCE_MESA,
        }))
    }
}

const REQUIRED_DEVICE_EXTENSIONS: &[&CStr] = &[
    external_memory_fd::NAME,
    external_semaphore_fd::NAME,
    external_fence_fd::NAME,
    external_memory_dma_buf::NAME,
    queue_family_foreign::NAME,
    image_drm_format_modifier::NAME,
    push_descriptor::NAME,
];

fn log_device(
    level: log::Level,
    props: &PhysicalDeviceProperties,
    extensions: Option<&Extensions>,
    driver_props: Option<&PhysicalDeviceDriverProperties>,
) {
    log::log!(
        level,
        "  api version: {}",
        ApiVersionDisplay(props.api_version)
    );
    log::log!(
        level,
        "  driver version: {}",
        ApiVersionDisplay(props.driver_version)
    );
    log::log!(level, "  vendor id: {}", props.vendor_id);
    log::log!(level, "  device id: {}", props.device_id);
    log::log!(level, "  device type: {:?}", props.device_type);
    unsafe {
        log::log!(
            level,
            "  device name: {}",
            Ustr::from_ptr(props.device_name.as_ptr()).display()
        );
    }
    if props.api_version < API_VERSION {
        log::warn!("  device does not support vulkan 1.3");
    }
    if let Some(extensions) = extensions {
        if !extensions.contains_key(physical_device_drm::NAME) {
            log::warn!("  device does support not the VK_EXT_physical_device_drm extension");
        }
    }
    if let Some(driver_props) = driver_props {
        unsafe {
            log::log!(
                level,
                "  driver: {} ({})",
                Ustr::from_ptr(driver_props.driver_name.as_ptr()).display(),
                Ustr::from_ptr(driver_props.driver_info.as_ptr()).display()
            );
        }
    }
}
