use crate::env::JAY_VULKAN_VALIDATION;
use crate::eventfd_cache::EventfdError;
use crate::syncobj::SyncobjError;
use crate::utils::bhash::BHashMap;
use crate::utils::bhash::BHashSet;
use crate::utils::hash_map_ext::HashMapExt;
use crate::utils::major_minor::MajorMinor;
use crate::utils::major_minor::major_minor;
use ash::Entry;
use ash::Instance;
use ash::LoadingError;
use ash::ext::debug_utils;
use ash::ext::validation_features;
use ash::vk::API_VERSION_1_3;
use ash::vk::ApplicationInfo;
use ash::vk::Bool32;
use ash::vk::DebugUtilsMessageSeverityFlagsEXT;
use ash::vk::DebugUtilsMessageTypeFlagsEXT;
use ash::vk::DebugUtilsMessengerCallbackDataEXT;
use ash::vk::DebugUtilsMessengerCreateInfoEXT;
use ash::vk::DebugUtilsMessengerEXT;
use ash::vk::ExtensionProperties;
use ash::vk::ExternalSemaphoreFeatureFlags;
use ash::vk::ExternalSemaphoreHandleTypeFlags;
use ash::vk::ExternalSemaphoreProperties;
use ash::vk::FALSE;
use ash::vk::InstanceCreateInfo;
use ash::vk::LayerProperties;
use ash::vk::PhysicalDevice;
use ash::vk::PhysicalDeviceDrmPropertiesEXT;
use ash::vk::PhysicalDeviceExternalSemaphoreInfo;
use ash::vk::PhysicalDeviceFeatures;
use ash::vk::PhysicalDeviceFeatures2;
use ash::vk::PhysicalDeviceTimelineSemaphoreFeatures;
use ash::vk::SemaphoreType;
use ash::vk::SemaphoreTypeCreateInfo;
use ash::vk::ValidationFeaturesEXT;
use ash::vk::api_version_major;
use ash::vk::api_version_minor;
use ash::vk::api_version_patch;
use ash::vk::api_version_variant;
use ash::vk::{self};
use dlopen_note::dlopen_note;
use log::Level;
use run_on_drop::on_drop;
use std::ffi::CStr;
use std::ffi::CString;
use std::ffi::c_void;
use std::fmt::Display;
use std::fmt::Formatter;
use std::slice;
use std::sync::Arc;
use std::sync::LazyLock;
use thiserror::Error;
use uapi::Ustr;
use uapi::c;
use uapi::ustr;

pub mod device;
pub mod fence;
pub mod gpu_alloc_ash;
pub mod sync;
pub mod timeline_semaphore;

dlopen_note! {
    soname: ["libvulkan.so.1"],
    feature: "vulkan",
    description: "required for the vulkan renderer",
    priority: "recommended",
}

static VULKAN_ENTRY: LazyLock<Result<Entry, Arc<LoadingError>>> =
    LazyLock::new(|| unsafe { Entry::load() }.map_err(Arc::new));

#[derive(Debug, Error)]
pub enum VulkanCoreError {
    #[error("Could not load libvulkan.so")]
    Load(#[source] Arc<LoadingError>),
    #[error("Could not list instance extensions")]
    InstanceExtensions(#[source] vk::Result),
    #[error("Could not list instance layers")]
    InstanceLayers(#[source] vk::Result),
    #[error("Missing required instance extension {0:?}")]
    MissingInstanceExtension(&'static CStr),
    #[error("Could not create an instance")]
    CreateInstance(#[source] vk::Result),
    #[error("Could not create a debug-utils messenger")]
    Messenger(#[source] vk::Result),
    #[error("Could not create a fence")]
    CreateFence(#[source] vk::Result),
    #[error("Could not export a sync file from a semaphore")]
    ExportSyncFile(#[source] vk::Result),
    #[error("Could not create a semaphore")]
    CreateSemaphore(#[source] vk::Result),
    #[error("Device does not support timeline semaphore export")]
    TimelineExportNotSupported,
    #[error("Could not export an opaque fd from a semaphore")]
    ExportTimelineSemaphore(#[source] vk::Result),
    #[error("Could not signal the timeline semaphore")]
    SignalSemaphore(#[source] vk::Result),
    #[error("Could not query last signaled sync obj point")]
    QueryLastSignaled(#[source] SyncobjError),
    #[error("Mapping between syncobj points and timeline semaphore points is unexpected")]
    UnsupportedPointMapping,
    #[error("Could not acquire an eventfd")]
    AcquireEventfd(#[source] EventfdError),
    #[error("Could not create a sync obj eventfd wait")]
    CreateSyncobjWait(#[source] SyncobjError),
    #[error("Device does not have a syncobj ctx")]
    NoSyncobjCtx,
}

pub struct VulkanCoreInstance {
    _entry: &'static Entry,
    pub instance: Instance,
    debug_utils: debug_utils::Instance,
    messenger: DebugUtilsMessengerEXT,
    pub log_level: Level,
    pub validation: bool,
}

pub struct VulkanDeviceFeatures {
    #[expect(dead_code)]
    pub features: PhysicalDeviceFeatures,
    pub semaphore_features: PhysicalDeviceTimelineSemaphoreFeatures<'static>,
}

impl VulkanCoreInstance {
    pub fn new(log_level: Level) -> Result<Self, VulkanCoreError> {
        let entry = match &*VULKAN_ENTRY {
            Ok(e) => e,
            Err(e) => return Err(VulkanCoreError::Load(e.clone())),
        };
        let extensions = get_instance_extensions(entry, None)?;
        for &ext in REQUIRED_INSTANCE_EXTENSIONS {
            if extensions.not_contains_key(ext) {
                return Err(VulkanCoreError::MissingInstanceExtension(ext));
            }
        }
        let mut enabled_extensions: Vec<_> = REQUIRED_INSTANCE_EXTENSIONS
            .iter()
            .map(|c| c.as_ptr())
            .collect();
        let app_info = ApplicationInfo::default()
            .api_version(VULKAN_API_VERSION)
            .application_name(c"jay")
            .application_version(1);
        let mut severity = DebugUtilsMessageSeverityFlagsEXT::empty()
            | DebugUtilsMessageSeverityFlagsEXT::ERROR
            | DebugUtilsMessageSeverityFlagsEXT::WARNING;
        let validation = *JAY_VULKAN_VALIDATION;
        if validation {
            severity |= DebugUtilsMessageSeverityFlagsEXT::INFO
                | DebugUtilsMessageSeverityFlagsEXT::VERBOSE;
        }
        let types = DebugUtilsMessageTypeFlagsEXT::empty()
            | DebugUtilsMessageTypeFlagsEXT::VALIDATION
            | DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
            | DebugUtilsMessageTypeFlagsEXT::GENERAL;
        let mut debug_info = DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(severity)
            .message_type(types)
            .pfn_user_callback(Some(debug_callback));
        let validation_features = [
            // ash::vk::ValidationFeatureEnableEXT::DEBUG_PRINTF,
            // ash::vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
            // ash::vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
            // ash::vk::ValidationFeatureEnableEXT::GPU_ASSISTED,
        ];
        let mut validation_info =
            ValidationFeaturesEXT::default().enabled_validation_features(&validation_features);
        let mut create_info = InstanceCreateInfo::default()
            .application_info(&app_info)
            .push_next(&mut debug_info);
        let validation_layer_name = VALIDATION_LAYER.as_ptr();
        if validation {
            if get_available_layers(entry)?.contains(VALIDATION_LAYER) {
                create_info =
                    create_info.enabled_layer_names(slice::from_ref(&validation_layer_name));
                let extensions = get_instance_extensions(entry, Some(VALIDATION_LAYER))?;
                if extensions.contains_key(validation_features::NAME) {
                    enabled_extensions.push(validation_features::NAME.as_ptr());
                    create_info = create_info.push_next(&mut validation_info);
                } else {
                    log::warn!("{:?} is not available", validation_features::NAME);
                }
            } else {
                log::warn!(
                    "Vulkan validation was requested but validation layers are not available"
                );
            }
        }
        create_info = create_info.enabled_extension_names(&enabled_extensions);
        let instance = match unsafe { entry.create_instance(&create_info, None) } {
            Ok(i) => i,
            Err(e) => return Err(VulkanCoreError::CreateInstance(e)),
        };
        let destroy_instance = on_drop(|| unsafe { instance.destroy_instance(None) });
        let debug_utils = debug_utils::Instance::new(entry, &instance);
        let messenger = unsafe { debug_utils.create_debug_utils_messenger(&debug_info, None) };
        let messenger = match messenger {
            Ok(m) => m,
            Err(e) => return Err(VulkanCoreError::Messenger(e)),
        };
        destroy_instance.forget();
        Ok(Self {
            _entry: entry,
            instance,
            debug_utils,
            messenger,
            log_level,
            validation,
        })
    }

    pub fn get_features(&self, phy_dev: PhysicalDevice) -> VulkanDeviceFeatures {
        let mut semaphore_features = PhysicalDeviceTimelineSemaphoreFeatures::default();
        let mut features = PhysicalDeviceFeatures2::default().push_next(&mut semaphore_features);
        unsafe {
            self.instance
                .get_physical_device_features2(phy_dev, &mut features);
        }
        VulkanDeviceFeatures {
            features: features.features,
            semaphore_features,
        }
    }

    pub fn supports_timeline_opaque_export(
        &self,
        phy_dev: PhysicalDevice,
        features: &VulkanDeviceFeatures,
    ) -> bool {
        if features.semaphore_features.timeline_semaphore == vk::TRUE {
            return self.supports_semaphore_opaque_export(phy_dev);
        }
        false
    }

    fn supports_semaphore_opaque_export(&self, phy_dev: PhysicalDevice) -> bool {
        let mut props = ExternalSemaphoreProperties::default();
        let mut type_info =
            SemaphoreTypeCreateInfo::default().semaphore_type(SemaphoreType::TIMELINE);
        let info = PhysicalDeviceExternalSemaphoreInfo::default()
            .handle_type(ExternalSemaphoreHandleTypeFlags::OPAQUE_FD)
            .push_next(&mut type_info);
        unsafe {
            self.instance
                .get_physical_device_external_semaphore_properties(phy_dev, &info, &mut props);
        }
        props
            .external_semaphore_features
            .contains(ExternalSemaphoreFeatureFlags::EXPORTABLE)
    }
}

impl Drop for VulkanCoreInstance {
    fn drop(&mut self) {
        unsafe {
            self.debug_utils
                .destroy_debug_utils_messenger(self.messenger, None);
            self.instance.destroy_instance(None);
        }
    }
}

const REQUIRED_INSTANCE_EXTENSIONS: &[&CStr] = &[debug_utils::NAME];

const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";

pub type Extensions = BHashMap<CString, u32>;

fn get_instance_extensions(
    entry: &Entry,
    layer: Option<&CStr>,
) -> Result<Extensions, VulkanCoreError> {
    unsafe {
        entry
            .enumerate_instance_extension_properties(layer)
            .map_err(VulkanCoreError::InstanceExtensions)
            .map(map_extension_properties)
    }
}

fn get_available_layers(entry: &Entry) -> Result<BHashSet<CString>, VulkanCoreError> {
    unsafe {
        entry
            .enumerate_instance_layer_properties()
            .map_err(VulkanCoreError::InstanceLayers)
            .map(map_layer_properties)
    }
}

fn map_layer_properties(props: Vec<LayerProperties>) -> BHashSet<CString> {
    props
        .into_iter()
        .map(|e| unsafe { CStr::from_ptr(e.layer_name.as_ptr()).to_owned() })
        .collect()
}

pub fn map_extension_properties(props: Vec<ExtensionProperties>) -> Extensions {
    props
        .into_iter()
        .map(|e| {
            let s = unsafe { CStr::from_ptr(e.extension_name.as_ptr()) };
            (s.to_owned(), e.spec_version)
        })
        .collect()
}

unsafe extern "system" fn debug_callback(
    message_severity: DebugUtilsMessageSeverityFlagsEXT,
    _message_types: DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> Bool32 {
    let _level = match message_severity {
        DebugUtilsMessageSeverityFlagsEXT::ERROR => Level::Error,
        DebugUtilsMessageSeverityFlagsEXT::WARNING => Level::Warn,
        DebugUtilsMessageSeverityFlagsEXT::INFO => Level::Info,
        DebugUtilsMessageSeverityFlagsEXT::VERBOSE => Level::Trace,
        _ => Level::Warn,
    };
    let data = unsafe { &*p_callback_data };
    let message = unsafe { Ustr::from_ptr(data.p_message) };
    let message_id_name = if data.p_message_id_name.is_null() {
        ustr!("<null>")
    } else {
        unsafe { Ustr::from_ptr(data.p_message_id_name) }
    };
    log::log!(
        Level::Info,
        "VULKAN: {} ({})",
        message.display(),
        message_id_name.display()
    );
    FALSE
}

pub struct ApiVersionDisplay(pub u32);

impl Display for ApiVersionDisplay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            api_version_variant(self.0),
            api_version_major(self.0),
            api_version_minor(self.0),
            api_version_patch(self.0),
        )
    }
}

pub const VULKAN_API_VERSION: u32 = API_VERSION_1_3;

pub fn vk_is_drm_dev(drm_props: &PhysicalDeviceDrmPropertiesEXT<'_>, dev: c::dev_t) -> bool {
    let MajorMinor { major, minor } = major_minor(dev);
    (drm_props.has_primary == vk::TRUE
        && drm_props.primary_major == major as i64
        && drm_props.primary_minor == minor as i64)
        || (drm_props.has_render == vk::TRUE
            && drm_props.render_major == major as i64
            && drm_props.render_minor == minor as i64)
}
