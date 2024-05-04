use {
    crate::{
        format::{Format, FORMATS},
        gfx_apis::vulkan::{instance::VulkanInstance, VulkanError},
        video::{Modifier, LINEAR_MODIFIER},
    },
    ahash::AHashMap,
    ash::{
        vk,
        vk::{
            DrmFormatModifierPropertiesEXT, DrmFormatModifierPropertiesListEXT,
            ExternalImageFormatProperties, ExternalMemoryFeatureFlags,
            ExternalMemoryHandleTypeFlags, FormatFeatureFlags, FormatProperties2,
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
    pub render_max_extents: Option<VulkanMaxExtents>,
    pub texture_max_extents: Option<VulkanMaxExtents>,
    pub render_needs_bridge: bool,
}

#[derive(Copy, Clone, Debug)]
pub struct VulkanMaxExtents {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct VulkanInternalFormat {
    pub max_extents: VulkanMaxExtents,
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
const SHM_FEATURES: FormatFeatureFlags = FormatFeatureFlags::from_raw(
    0 | FormatFeatureFlags::TRANSFER_SRC.as_raw()
        | FormatFeatureFlags::TRANSFER_DST.as_raw()
        | TEX_FEATURES.as_raw(),
);

const FRAMEBUFFER_USAGE: ImageUsageFlags = ImageUsageFlags::from_raw(
    0 | ImageUsageFlags::COLOR_ATTACHMENT.as_raw() | ImageUsageFlags::TRANSFER_SRC.as_raw(),
);
const FRAMEBUFFER_BRIDGED_USAGE: ImageUsageFlags = ImageUsageFlags::TRANSFER_DST;
const TEX_USAGE: ImageUsageFlags = ImageUsageFlags::from_raw(
    0 | ImageUsageFlags::SAMPLED.as_raw() | ImageUsageFlags::TRANSFER_SRC.as_raw(),
);
const SHM_USAGE: ImageUsageFlags = ImageUsageFlags::from_raw(
    0 | ImageUsageFlags::TRANSFER_SRC.as_raw()
        | ImageUsageFlags::TRANSFER_DST.as_raw()
        | TEX_USAGE.as_raw(),
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
        let mut modifier_props = DrmFormatModifierPropertiesListEXT::builder().build();
        let mut format_properties = FormatProperties2::builder()
            .push_next(&mut modifier_props)
            .build();
        unsafe {
            self.instance.get_physical_device_format_properties2(
                phy_dev,
                format.vk_format,
                &mut format_properties,
            );
        }
        let shm = self.load_shm_format(phy_dev, format, &format_properties)?;
        let modifiers =
            self.load_drm_format(phy_dev, format, &format_properties, &modifier_props)?;
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

    fn load_shm_format(
        &self,
        phy_dev: PhysicalDevice,
        format: &Format,
        props: &FormatProperties2,
    ) -> Result<Option<VulkanInternalFormat>, VulkanError> {
        if format.shm_info.is_none() {
            return Ok(None);
        }
        self.load_internal_format(phy_dev, format, props, SHM_FEATURES, SHM_USAGE)
    }

    fn load_internal_format(
        &self,
        phy_dev: PhysicalDevice,
        format: &Format,
        props: &FormatProperties2,
        features: FormatFeatureFlags,
        usage: ImageUsageFlags,
    ) -> Result<Option<VulkanInternalFormat>, VulkanError> {
        if !props
            .format_properties
            .optimal_tiling_features
            .contains(features)
        {
            return Ok(None);
        }
        let format_info = PhysicalDeviceImageFormatInfo2::builder()
            .ty(ImageType::TYPE_2D)
            .format(format.vk_format)
            .tiling(ImageTiling::OPTIMAL)
            .usage(usage);
        let mut format_properties = ImageFormatProperties2::builder();
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
            max_extents: VulkanMaxExtents {
                width: format_properties.image_format_properties.max_extent.width,
                height: format_properties.image_format_properties.max_extent.height,
            },
        }))
    }

    fn load_drm_format(
        &self,
        phy_dev: PhysicalDevice,
        format: &Format,
        internal_format_properties: &FormatProperties2,
        props: &DrmFormatModifierPropertiesListEXT,
    ) -> Result<AHashMap<Modifier, VulkanModifier>, VulkanError> {
        if props.drm_format_modifier_count == 0 {
            return Ok(AHashMap::new());
        }
        let mut drm_mods = vec![
            DrmFormatModifierPropertiesEXT::default();
            props.drm_format_modifier_count as usize
        ];
        let mut modifier_props = DrmFormatModifierPropertiesListEXT::builder()
            .drm_format_modifier_properties(&mut drm_mods)
            .build();
        let mut format_properties = FormatProperties2::builder()
            .push_next(&mut modifier_props)
            .build();
        unsafe {
            self.instance.get_physical_device_format_properties2(
                phy_dev,
                format.vk_format,
                &mut format_properties,
            );
        };
        let mut mods = AHashMap::new();
        for modifier in drm_mods {
            let mut render_max_extents = self.get_max_extents(
                phy_dev,
                format,
                FRAMEBUFFER_FEATURES,
                FRAMEBUFFER_USAGE,
                &modifier,
            )?;
            let texture_max_extents =
                self.get_max_extents(phy_dev, format, TEX_FEATURES, TEX_USAGE, &modifier)?;
            let mut render_needs_bridge = false;
            if render_max_extents.is_none() && modifier.drm_format_modifier == LINEAR_MODIFIER {
                render_max_extents = self.get_fb_bridged_max_extents(
                    phy_dev,
                    format,
                    internal_format_properties,
                    &modifier,
                )?;
                if render_max_extents.is_some() {
                    render_needs_bridge = true;
                }
            }
            mods.insert(
                modifier.drm_format_modifier,
                VulkanModifier {
                    modifier: modifier.drm_format_modifier,
                    planes: modifier.drm_format_modifier_plane_count as _,
                    features: modifier.drm_format_modifier_tiling_features,
                    render_max_extents,
                    texture_max_extents,
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
        internal_format_properties: &FormatProperties2,
        modifier: &DrmFormatModifierPropertiesEXT,
    ) -> Result<Option<VulkanMaxExtents>, VulkanError> {
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
        Ok(Some(VulkanMaxExtents {
            width: min(
                transfer_dst_max_extents.width,
                bridge_format.max_extents.width,
            ),
            height: min(
                transfer_dst_max_extents.height,
                bridge_format.max_extents.height,
            ),
        }))
    }

    fn get_max_extents(
        &self,
        phy_dev: PhysicalDevice,
        format: &Format,
        features: FormatFeatureFlags,
        usage: ImageUsageFlags,
        props: &DrmFormatModifierPropertiesEXT,
    ) -> Result<Option<VulkanMaxExtents>, VulkanError> {
        if !props.drm_format_modifier_tiling_features.contains(features) {
            return Ok(None);
        }
        let mut modifier_info = PhysicalDeviceImageDrmFormatModifierInfoEXT::builder()
            .drm_format_modifier(props.drm_format_modifier)
            .sharing_mode(SharingMode::EXCLUSIVE)
            .build();
        let mut external_image_format_info = PhysicalDeviceExternalImageFormatInfo::builder()
            .handle_type(ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
            .build();
        let image_format_info = PhysicalDeviceImageFormatInfo2::builder()
            .ty(ImageType::TYPE_2D)
            .format(format.vk_format)
            .usage(usage)
            .tiling(ImageTiling::DRM_FORMAT_MODIFIER_EXT)
            .push_next(&mut external_image_format_info)
            .push_next(&mut modifier_info)
            .build();

        let mut external_image_format_props = ExternalImageFormatProperties::builder().build();
        let mut image_format_props = ImageFormatProperties2::builder()
            .push_next(&mut external_image_format_props)
            .build();

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
        let importable = external_image_format_props
            .external_memory_properties
            .external_memory_features
            .contains(ExternalMemoryFeatureFlags::IMPORTABLE);
        if !importable {
            return Ok(None);
        }

        Ok(Some(VulkanMaxExtents {
            width: image_format_props.image_format_properties.max_extent.width,
            height: image_format_props.image_format_properties.max_extent.height,
        }))
    }
}
