use {
    crate::{
        format::{ABGR16161616F, FORMATS, Format},
        gfx_apis::vulkan::{VulkanError, instance::VulkanInstance},
        video::{LINEAR_MODIFIER, Modifier},
    },
    ahash::AHashMap,
    ash::{
        vk,
        vk::{
            DrmFormatModifierPropertiesEXT, DrmFormatModifierPropertiesListEXT,
            ExternalImageFormatProperties, ExternalMemoryFeatureFlags,
            ExternalMemoryHandleTypeFlags, FormatFeatureFlags, FormatProperties, FormatProperties2,
            ImageFormatProperties2, ImageTiling, ImageType, ImageUsageFlags, PhysicalDevice,
            PhysicalDeviceExternalImageFormatInfo, PhysicalDeviceImageDrmFormatModifierInfoEXT,
            PhysicalDeviceImageFormatInfo2, SharingMode,
        },
    },
    isnt::std_1::collections::IsntHashMapExt,
    std::cmp::min,
};

#[derive(Debug)]
pub struct VulkanFormat {
    pub format: &'static Format,
    pub modifiers: AHashMap<Modifier, VulkanModifier>,
    pub shm: Option<VulkanInternalFormat>,
}

#[derive(Debug)]
pub struct VulkanModifier {
    pub modifier: Modifier,
    pub planes: usize,
    pub features: FormatFeatureFlags,
    pub render_limits: Option<VulkanModifierLimits>,
    pub texture_limits: Option<VulkanModifierLimits>,
    pub transfer_limits: Option<VulkanModifierLimits>,
    pub render_needs_bridge: bool,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct VulkanModifierLimits {
    pub max_width: u32,
    pub max_height: u32,
    pub exportable: bool,
}

#[derive(Debug, Default)]
pub struct VulkanInternalFormat {
    pub limits: VulkanModifierLimits,
}

#[derive(Copy, Clone, Debug)]
pub struct VulkanBlendBufferLimits {
    pub max_width: u32,
    pub max_height: u32,
}

const FRAMEBUFFER_FEATURES: FormatFeatureFlags = FormatFeatureFlags::from_raw(
    0 | FormatFeatureFlags::COLOR_ATTACHMENT.as_raw()
        | FormatFeatureFlags::COLOR_ATTACHMENT_BLEND.as_raw(),
);
const FRAMEBUFFER_BRIDGED_FEATURES: FormatFeatureFlags = FormatFeatureFlags::TRANSFER_DST;
const TEX_FEATURES: FormatFeatureFlags = FormatFeatureFlags::from_raw(
    0 | FormatFeatureFlags::SAMPLED_IMAGE.as_raw()
        | FormatFeatureFlags::TRANSFER_SRC.as_raw()
        | FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR.as_raw(),
);
const TRANSFER_FEATURES: FormatFeatureFlags = FormatFeatureFlags::from_raw(
    FormatFeatureFlags::TRANSFER_SRC.as_raw() | FormatFeatureFlags::TRANSFER_DST.as_raw(),
);
const SHM_FEATURES: FormatFeatureFlags =
    FormatFeatureFlags::from_raw(TRANSFER_FEATURES.as_raw() | TEX_FEATURES.as_raw());

const FRAMEBUFFER_USAGE: ImageUsageFlags = ImageUsageFlags::from_raw(
    ImageUsageFlags::COLOR_ATTACHMENT.as_raw() | ImageUsageFlags::TRANSFER_SRC.as_raw(),
);
const FRAMEBUFFER_BRIDGED_USAGE: ImageUsageFlags = ImageUsageFlags::TRANSFER_DST;
const TEX_USAGE: ImageUsageFlags = ImageUsageFlags::from_raw(
    ImageUsageFlags::SAMPLED.as_raw() | ImageUsageFlags::TRANSFER_SRC.as_raw(),
);
const TRANSFER_USAGE: ImageUsageFlags = ImageUsageFlags::from_raw(
    ImageUsageFlags::TRANSFER_SRC.as_raw() | ImageUsageFlags::TRANSFER_DST.as_raw(),
);
const SHM_USAGE: ImageUsageFlags =
    ImageUsageFlags::from_raw(TRANSFER_USAGE.as_raw() | TEX_USAGE.as_raw());

pub const BLEND_FORMAT: &Format = ABGR16161616F;
const BLEND_FEATURES: FormatFeatureFlags = FormatFeatureFlags::from_raw(
    0 | FormatFeatureFlags::COLOR_ATTACHMENT.as_raw()
        | FormatFeatureFlags::COLOR_ATTACHMENT_BLEND.as_raw()
        | FormatFeatureFlags::SAMPLED_IMAGE.as_raw(),
);
pub const BLEND_USAGE: ImageUsageFlags = ImageUsageFlags::from_raw(
    ImageUsageFlags::COLOR_ATTACHMENT.as_raw() | ImageUsageFlags::SAMPLED.as_raw(),
);

impl VulkanInstance {
    pub(super) fn load_formats(
        &self,
        phy_dev: PhysicalDevice,
    ) -> Result<AHashMap<u32, VulkanFormat>, VulkanError> {
        let mut res = AHashMap::new();
        for format in FORMATS {
            self.load_format(phy_dev, format, &mut res)?;
        }
        Ok(res)
    }

    fn load_format(
        &self,
        phy_dev: PhysicalDevice,
        format: &'static Format,
        dst: &mut AHashMap<u32, VulkanFormat>,
    ) -> Result<(), VulkanError> {
        let mut modifier_props = DrmFormatModifierPropertiesListEXT::default();
        let mut format_properties = FormatProperties2::default().push_next(&mut modifier_props);
        unsafe {
            self.instance.get_physical_device_format_properties2(
                phy_dev,
                format.vk_format,
                &mut format_properties,
            );
        }
        let shm = self.load_shm_format(phy_dev, format, &format_properties.format_properties)?;
        let modifiers = self.load_drm_format(
            phy_dev,
            format,
            &format_properties.format_properties,
            &modifier_props,
        )?;
        if shm.is_some() || modifiers.is_not_empty() {
            dst.insert(
                format.drm,
                VulkanFormat {
                    format,
                    modifiers,
                    shm,
                },
            );
        }
        Ok(())
    }

    pub fn load_blend_format_limits(
        &self,
        phy_dev: PhysicalDevice,
    ) -> Result<VulkanBlendBufferLimits, VulkanError> {
        let format_properties = unsafe {
            self.instance
                .get_physical_device_format_properties(phy_dev, BLEND_FORMAT.vk_format)
        };
        let l = self
            .load_internal_format(
                phy_dev,
                BLEND_FORMAT,
                &format_properties,
                BLEND_FEATURES,
                BLEND_USAGE,
            )?
            .unwrap_or_default();
        Ok(VulkanBlendBufferLimits {
            max_width: l.limits.max_width,
            max_height: l.limits.max_height,
        })
    }

    fn load_shm_format(
        &self,
        phy_dev: PhysicalDevice,
        format: &Format,
        props: &FormatProperties,
    ) -> Result<Option<VulkanInternalFormat>, VulkanError> {
        self.load_internal_format(phy_dev, format, props, SHM_FEATURES, SHM_USAGE)
    }

    fn load_internal_format(
        &self,
        phy_dev: PhysicalDevice,
        format: &Format,
        props: &FormatProperties,
        features: FormatFeatureFlags,
        usage: ImageUsageFlags,
    ) -> Result<Option<VulkanInternalFormat>, VulkanError> {
        if !props.optimal_tiling_features.contains(features) {
            return Ok(None);
        }
        let format_info = PhysicalDeviceImageFormatInfo2::default()
            .ty(ImageType::TYPE_2D)
            .format(format.vk_format)
            .tiling(ImageTiling::OPTIMAL)
            .usage(usage);
        let mut format_properties = ImageFormatProperties2::default();
        let res = unsafe {
            self.instance.get_physical_device_image_format_properties2(
                phy_dev,
                &format_info,
                &mut format_properties,
            )
        };
        if let Err(e) = res {
            return match e {
                vk::Result::ERROR_FORMAT_NOT_SUPPORTED => Ok(None),
                _ => Err(VulkanError::LoadImageProperties(e)),
            };
        }
        Ok(Some(VulkanInternalFormat {
            limits: VulkanModifierLimits {
                max_width: format_properties.image_format_properties.max_extent.width,
                max_height: format_properties.image_format_properties.max_extent.height,
                exportable: false,
            },
        }))
    }

    fn load_drm_format(
        &self,
        phy_dev: PhysicalDevice,
        format: &Format,
        internal_format_properties: &FormatProperties,
        props: &DrmFormatModifierPropertiesListEXT,
    ) -> Result<AHashMap<Modifier, VulkanModifier>, VulkanError> {
        if props.drm_format_modifier_count == 0 {
            return Ok(AHashMap::new());
        }
        let mut drm_mods = vec![
            DrmFormatModifierPropertiesEXT::default();
            props.drm_format_modifier_count as usize
        ];
        let mut modifier_props = DrmFormatModifierPropertiesListEXT::default()
            .drm_format_modifier_properties(&mut drm_mods);
        let mut format_properties = FormatProperties2::default().push_next(&mut modifier_props);
        unsafe {
            self.instance.get_physical_device_format_properties2(
                phy_dev,
                format.vk_format,
                &mut format_properties,
            );
        };
        let mut mods = AHashMap::new();
        for modifier in drm_mods {
            let mut render_limits = self.get_max_extents(
                phy_dev,
                format,
                FRAMEBUFFER_FEATURES,
                FRAMEBUFFER_USAGE,
                &modifier,
            )?;
            let texture_limits =
                self.get_max_extents(phy_dev, format, TEX_FEATURES, TEX_USAGE, &modifier)?;
            let transfer_limits = self.get_max_extents(
                phy_dev,
                format,
                TRANSFER_FEATURES,
                TRANSFER_USAGE,
                &modifier,
            )?;
            let mut render_needs_bridge = false;
            if render_limits.is_none() && modifier.drm_format_modifier == LINEAR_MODIFIER {
                render_limits = self.get_fb_bridged_max_extents(
                    phy_dev,
                    format,
                    internal_format_properties,
                    &modifier,
                )?;
                if render_limits.is_some() {
                    render_needs_bridge = true;
                }
            }
            mods.insert(
                modifier.drm_format_modifier,
                VulkanModifier {
                    modifier: modifier.drm_format_modifier,
                    planes: modifier.drm_format_modifier_plane_count as _,
                    features: modifier.drm_format_modifier_tiling_features,
                    render_limits,
                    texture_limits,
                    transfer_limits,
                    render_needs_bridge,
                },
            );
        }
        Ok(mods)
    }

    fn get_fb_bridged_max_extents(
        &self,
        phy_dev: PhysicalDevice,
        format: &Format,
        internal_format_properties: &FormatProperties,
        modifier: &DrmFormatModifierPropertiesEXT,
    ) -> Result<Option<VulkanModifierLimits>, VulkanError> {
        let transfer_dst_max_extents = self.get_max_extents(
            phy_dev,
            format,
            FRAMEBUFFER_BRIDGED_FEATURES,
            FRAMEBUFFER_BRIDGED_USAGE,
            &modifier,
        )?;
        let Some(transfer_dst_max_extents) = transfer_dst_max_extents else {
            return Ok(None);
        };
        let bridge_format = self.load_internal_format(
            phy_dev,
            format,
            internal_format_properties,
            FRAMEBUFFER_FEATURES,
            FRAMEBUFFER_USAGE,
        )?;
        let Some(bridge_format) = bridge_format else {
            return Ok(None);
        };
        Ok(Some(VulkanModifierLimits {
            max_width: min(
                transfer_dst_max_extents.max_width,
                bridge_format.limits.max_width,
            ),
            max_height: min(
                transfer_dst_max_extents.max_height,
                bridge_format.limits.max_height,
            ),
            exportable: transfer_dst_max_extents.exportable,
        }))
    }

    fn get_max_extents(
        &self,
        phy_dev: PhysicalDevice,
        format: &Format,
        features: FormatFeatureFlags,
        usage: ImageUsageFlags,
        props: &DrmFormatModifierPropertiesEXT,
    ) -> Result<Option<VulkanModifierLimits>, VulkanError> {
        if !props.drm_format_modifier_tiling_features.contains(features) {
            return Ok(None);
        }
        let mut modifier_info = PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
            .drm_format_modifier(props.drm_format_modifier)
            .sharing_mode(SharingMode::EXCLUSIVE);
        let mut external_image_format_info = PhysicalDeviceExternalImageFormatInfo::default()
            .handle_type(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let image_format_info = PhysicalDeviceImageFormatInfo2::default()
            .ty(ImageType::TYPE_2D)
            .format(format.vk_format)
            .usage(usage)
            .tiling(ImageTiling::DRM_FORMAT_MODIFIER_EXT)
            .push_next(&mut external_image_format_info)
            .push_next(&mut modifier_info);

        let mut external_image_format_props = ExternalImageFormatProperties::default();
        let mut image_format_props =
            ImageFormatProperties2::default().push_next(&mut external_image_format_props);

        let res = unsafe {
            self.instance.get_physical_device_image_format_properties2(
                phy_dev,
                &image_format_info,
                &mut image_format_props,
            )
        };

        if let Err(e) = res {
            return match e {
                vk::Result::ERROR_FORMAT_NOT_SUPPORTED => Ok(None),
                _ => Err(VulkanError::LoadImageProperties(e)),
            };
        }
        let image_format_props = &image_format_props.image_format_properties;
        let external_memory_features = &external_image_format_props
            .external_memory_properties
            .external_memory_features;
        let importable = external_memory_features.contains(ExternalMemoryFeatureFlags::IMPORTABLE);
        if !importable {
            return Ok(None);
        }
        let exportable = external_memory_features.contains(ExternalMemoryFeatureFlags::EXPORTABLE);

        Ok(Some(VulkanModifierLimits {
            max_width: image_format_props.max_extent.width,
            max_height: image_format_props.max_extent.height,
            exportable,
        }))
    }
}
