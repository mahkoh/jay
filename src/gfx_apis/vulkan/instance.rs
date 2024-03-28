use {
    crate::{
        async_engine::AsyncEngine,
        gfx_apis::vulkan::{util::OnDrop, VulkanError, VULKAN_VALIDATION},
        io_uring::IoUring,
    },
    ahash::{AHashMap, AHashSet},
    ash::{
        extensions::ext::DebugUtils,
        vk::{
            api_version_major, api_version_minor, api_version_patch, api_version_variant,
            ApplicationInfo, Bool32, DebugUtilsMessageSeverityFlagsEXT,
            DebugUtilsMessageTypeFlagsEXT, DebugUtilsMessengerCallbackDataEXT,
            DebugUtilsMessengerCreateInfoEXT, DebugUtilsMessengerEXT, ExtDebugUtilsFn,
            ExtValidationFeaturesFn, ExtensionProperties, InstanceCreateInfo, LayerProperties,
            ValidationFeaturesEXT, API_VERSION_1_3, FALSE,
        },
        Entry, Instance, LoadingError,
    },
    isnt::std_1::collections::IsntHashMap2Ext,
    log::Level,
    once_cell::sync::Lazy,
    std::{
        ffi::{c_void, CStr, CString},
        fmt::{Display, Formatter},
        iter::IntoIterator,
        rc::Rc,
        slice,
        sync::Arc,
    },
    uapi::{ustr, Ustr},
};

pub struct VulkanInstance {
    pub(super) _entry: &'static Entry,
    pub(super) instance: Instance,
    pub(super) debug_utils: DebugUtils,
    pub(super) messenger: DebugUtilsMessengerEXT,
    pub(super) eng: Rc<AsyncEngine>,
    pub(super) ring: Rc<IoUring>,
}

impl VulkanInstance {
    pub fn new(
        eng: &Rc<AsyncEngine>,
        ring: &Rc<IoUring>,
        validation: bool,
    ) -> Result<Rc<Self>, VulkanError> {
        static ENTRY: Lazy<Result<Entry, Arc<LoadingError>>> =
            Lazy::new(|| unsafe { Entry::load() }.map_err(Arc::new));
        let entry = match &*ENTRY {
            Ok(e) => e,
            Err(e) => return Err(VulkanError::Load(e.clone())),
        };
        let extensions = get_instance_extensions(entry, None)?;
        for &ext in REQUIRED_INSTANCE_EXTENSIONS {
            if extensions.not_contains_key(ext) {
                return Err(VulkanError::MissingInstanceExtension(ext));
            }
        }
        let mut enabled_extensions: Vec<_> = REQUIRED_INSTANCE_EXTENSIONS
            .iter()
            .map(|c| c.as_ptr())
            .collect();
        let app_info = ApplicationInfo::builder()
            .api_version(API_VERSION)
            .application_name(ustr!("jay").as_c_str().unwrap())
            .application_version(1);
        let mut severity = DebugUtilsMessageSeverityFlagsEXT::empty()
            | DebugUtilsMessageSeverityFlagsEXT::ERROR
            | DebugUtilsMessageSeverityFlagsEXT::WARNING;
        if *VULKAN_VALIDATION {
            severity |= DebugUtilsMessageSeverityFlagsEXT::INFO
                | DebugUtilsMessageSeverityFlagsEXT::VERBOSE;
        }
        let types = DebugUtilsMessageTypeFlagsEXT::empty()
            | DebugUtilsMessageTypeFlagsEXT::VALIDATION
            | DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
            | DebugUtilsMessageTypeFlagsEXT::GENERAL;
        let mut debug_info = DebugUtilsMessengerCreateInfoEXT::builder()
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
            ValidationFeaturesEXT::builder().enabled_validation_features(&validation_features);
        let mut create_info = InstanceCreateInfo::builder()
            .application_info(&app_info)
            .push_next(&mut debug_info);
        let validation_layer_name = VALIDATION_LAYER.as_ptr();
        if validation {
            if get_available_layers(entry)?.contains(VALIDATION_LAYER) {
                create_info =
                    create_info.enabled_layer_names(slice::from_ref(&validation_layer_name));
                let extensions = get_instance_extensions(entry, Some(VALIDATION_LAYER))?;
                if extensions.contains_key(ExtValidationFeaturesFn::name()) {
                    enabled_extensions.push(ExtValidationFeaturesFn::name().as_ptr());
                    create_info = create_info.push_next(&mut validation_info);
                } else {
                    log::warn!("{:?} is not available", ExtValidationFeaturesFn::name(),);
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
            Err(e) => return Err(VulkanError::CreateInstance(e)),
        };
        let destroy_instance = OnDrop(|| unsafe { instance.destroy_instance(None) });
        let debug_utils = DebugUtils::new(entry, &instance);
        let messenger = unsafe { debug_utils.create_debug_utils_messenger(&debug_info, None) };
        let messenger = match messenger {
            Ok(m) => m,
            Err(e) => return Err(VulkanError::Messenger(e)),
        };
        destroy_instance.forget();
        Ok(Rc::new(Self {
            _entry: entry,
            instance,
            debug_utils,
            messenger,
            eng: eng.clone(),
            ring: ring.clone(),
        }))
    }
}

impl Drop for VulkanInstance {
    fn drop(&mut self) {
        unsafe {
            self.debug_utils
                .destroy_debug_utils_messenger(self.messenger, None);
            self.instance.destroy_instance(None);
        }
    }
}

const REQUIRED_INSTANCE_EXTENSIONS: &[&CStr] = &[ExtDebugUtilsFn::name()];

const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";

pub type Extensions = AHashMap<CString, u32>;

fn get_instance_extensions(entry: &Entry, layer: Option<&CStr>) -> Result<Extensions, VulkanError> {
    entry
        .enumerate_instance_extension_properties(layer)
        .map_err(VulkanError::InstanceExtensions)
        .map(map_extension_properties)
}

fn get_available_layers(entry: &Entry) -> Result<AHashSet<CString>, VulkanError> {
    entry
        .enumerate_instance_layer_properties()
        .map_err(VulkanError::InstanceLayers)
        .map(map_layer_properties)
}

fn map_layer_properties(props: Vec<LayerProperties>) -> AHashSet<CString> {
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
    let data = &*p_callback_data;
    let message = Ustr::from_ptr(data.p_message);
    let message_id_name = if data.p_message_id_name.is_null() {
        ustr!("<null>")
    } else {
        Ustr::from_ptr(data.p_message_id_name)
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

pub const API_VERSION: u32 = API_VERSION_1_3;
