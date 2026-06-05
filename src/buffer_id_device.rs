use {
    crate::{
        utils::{copyhashmap::CopyHashMap, errorfmt::ErrorFmt},
        video::dmabuf::{DmaBuf, PlaneVec},
        vulkan_core::{VulkanCoreError, VulkanCoreInstance, map_extension_properties},
    },
    ash::{
        Device,
        ext::{external_memory_dma_buf, physical_device_drm},
        khr::{external_memory_fd, maintenance9},
        vk::{
            self, API_VERSION_1_1, DeviceCreateInfo, ExternalMemoryHandleTypeFlags,
            MemoryFdPropertiesKHR, MemoryPropertyFlags, MemoryType, PhysicalDeviceDrmPropertiesEXT,
            PhysicalDeviceMaintenance9FeaturesKHR, PhysicalDeviceProperties2,
        },
    },
    bstr::ByteSlice,
    isnt::std_1::collections::IsntHashMapExt,
    log::Level,
    run_on_drop::on_drop,
    std::{
        error::Error,
        ffi::CStr,
        fmt::{Debug, Formatter},
        rc::Rc,
    },
    thiserror::Error,
    uapi::{AsUstr, c},
};

#[derive(Debug, Error)]
pub enum BufferIdDeviceError {
    #[error(transparent)]
    Core(#[from] VulkanCoreError),
    #[error("Could not enumerate the physical devices")]
    EnumeratePhysicalDevice(#[source] vk::Result),
    #[error("Could not find a corresponding vulkan device")]
    NoVulkanDevice,
    #[error("Device does not support vulkan 1.1")]
    NoVulkan11,
    #[error("Device does not support the device extension {}", .0.as_ustr().as_bytes().as_bstr())]
    MissingDeviceExtensions(&'static CStr),
    #[error("Could not create the device")]
    CreateDevice(#[source] vk::Result),
    #[error("Could not query memory fd properties")]
    GetMemoryFdProperties(#[source] vk::Result),
}

pub struct BufferIdDevice {
    _instance: VulkanCoreInstance,
    memory_types: Vec<MemoryType>,
    dev: Device,
    external_memory_fd: external_memory_fd::Device,
}

#[derive(Default)]
pub struct BufferIdDeviceRegistry {
    devs: CopyHashMap<c::dev_t, Option<Rc<BufferIdDevice>>>,
}

pub trait BufferIdDeviceDyn {
    fn is_on_device(&self, buf: &DmaBuf) -> Result<bool, Box<dyn Error>>;
}

const DEVICE_EXTENSIONS: [&CStr; 3] = [
    external_memory_fd::NAME,
    external_memory_dma_buf::NAME,
    maintenance9::NAME,
];

impl BufferIdDevice {
    fn new(dev: c::dev_t) -> Result<Rc<Self>, BufferIdDeviceError> {
        let core_instance = VulkanCoreInstance::new(Level::Debug)?;
        let instance = &core_instance.instance;
        let physical_device;
        let device_extensions;
        let device_properties;
        'find_device: {
            let devices = unsafe {
                instance
                    .enumerate_physical_devices()
                    .map_err(BufferIdDeviceError::EnumeratePhysicalDevice)?
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
            return Err(BufferIdDeviceError::NoVulkanDevice);
        }
        if device_properties.api_version < API_VERSION_1_1 {
            return Err(BufferIdDeviceError::NoVulkan11);
        }
        for ext in DEVICE_EXTENSIONS {
            if device_extensions.not_contains_key(ext) {
                return Err(BufferIdDeviceError::MissingDeviceExtensions(ext));
            }
        }
        let memory_info =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let device = {
            let extensions = DEVICE_EXTENSIONS.map(|e| e.as_ptr());
            let mut maint9_features =
                PhysicalDeviceMaintenance9FeaturesKHR::default().maintenance9(true);
            let info = DeviceCreateInfo::default()
                .enabled_extension_names(&extensions)
                .push_next(&mut maint9_features);
            unsafe {
                instance
                    .create_device(physical_device, &info, None)
                    .map_err(BufferIdDeviceError::CreateDevice)?
            }
        };
        let destroy_device = on_drop(|| unsafe { device.destroy_device(None) });
        let external_memory_fd = external_memory_fd::Device::new(instance, &device);
        destroy_device.forget();
        Ok(Rc::new(BufferIdDevice {
            _instance: core_instance,
            memory_types: memory_info.memory_types_as_slice().to_vec(),
            dev: device,
            external_memory_fd,
        }))
    }
}

impl BufferIdDevice {
    pub fn is_on_device(&self, buf: &DmaBuf) -> Result<bool, BufferIdDeviceError> {
        let mut fd_props = PlaneVec::new();
        for plane in &buf.planes {
            let mut props = MemoryFdPropertiesKHR::default();
            unsafe {
                self.external_memory_fd
                    .get_memory_fd_properties(
                        ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
                        plane.fd.raw(),
                        &mut props,
                    )
                    .map_err(BufferIdDeviceError::GetMemoryFdProperties)?;
            }
            fd_props.push(props);
            if buf.is_one_file() {
                break;
            }
        }
        let mut on_device = true;
        for prop in &fd_props {
            let mut plane_on_device = false;
            for (idx, ty) in self.memory_types.iter().enumerate() {
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
        Ok(on_device)
    }
}

impl BufferIdDeviceDyn for BufferIdDevice {
    fn is_on_device(&self, buf: &DmaBuf) -> Result<bool, Box<dyn Error>> {
        self.is_on_device(buf).map_err(|e| Box::new(e) as _)
    }
}

impl BufferIdDeviceRegistry {
    pub fn remove(&self, dev: c::dev_t) {
        self.devs.remove(&dev);
    }

    pub fn get(&self, dev: c::dev_t) -> Option<Rc<BufferIdDevice>> {
        if let Some(dev) = self.devs.get(&dev) {
            return dev;
        }
        match BufferIdDevice::new(dev).map(Some) {
            Ok(cd) => {
                self.devs.set(dev, cd.clone());
                cd
            }
            Err(e) => {
                let maj = uapi::major(dev);
                let min = uapi::minor(dev);
                log::warn!(
                    "Could not create buffer id device for {maj}:{min}: {}",
                    ErrorFmt(e),
                );
                self.devs.set(dev, None);
                None
            }
        }
    }
}

impl Drop for BufferIdDevice {
    fn drop(&mut self) {
        unsafe {
            self.dev.destroy_device(None);
        }
    }
}

impl Debug for BufferIdDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferIdDevice").finish_non_exhaustive()
    }
}
