use {
    crate::{
        format::{Format, FORMATS},
        gfx_apis::vulkan::{instance::VulkanInstance, VulkanError},
        video::Modifier,
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
};

#[derive(Debug)]
pub struct VulkanFormat {
    pub format: &'static Format,
    pub modifiers: AHashMap<Modifier, VulkanModifier>,
    pub shm: Option<VulkanShmFormat>,
    pub features: FormatFeatureFlags,
}

#[derive(Debug)]
pub struct VulkanFormatFeatures {
    pub linear_sampling: bool,
}

#[derive(Debug)]
pub struct VulkanModifier {
    pub modifier: Modifier,
    pub planes: usize,
    pub features: FormatFeatureFlags,
    pub render_max_extents: Option<VulkanMaxExtents>,
    pub texture_max_extents: Option<VulkanMaxExtents>,
}

#[derive(Copy, Clone, Debug)]
pub struct VulkanMaxExtents {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct VulkanShmFormat {
    pub max_extents: VulkanMaxExtents,
}

const FRAMEBUFFER_FEATURES: FormatFeatureFlags = FormatFeatureFlags::from_raw(
    0 | FormatFeatureFlags::COLOR_ATTACHMENT.as_raw()
        | FormatFeatureFlags::COLOR_ATTACHMENT_BLEND.as_raw(),
);
const YCBCR_TEX_FEATURES: FormatFeatureFlags = FormatFeatureFlags::from_raw(
    0 | FormatFeatureFlags::SAMPLED_IMAGE.as_raw()
        | FormatFeatureFlags::SAMPLED_IMAGE_YCBCR_CONVERSION_LINEAR_FILTER.as_raw()
        | FormatFeatureFlags::MIDPOINT_CHROMA_SAMPLES.as_raw(),
);
const TEX_FEATURES: FormatFeatureFlags = FormatFeatureFlags::from_raw(
    0 | FormatFeatureFlags::SAMPLED_IMAGE.as_raw()
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
const TEX_USAGE: ImageUsageFlags = ImageUsageFlags::from_raw(0 | ImageUsageFlags::SAMPLED.as_raw());
const SHM_USAGE: ImageUsageFlags =
    ImageUsageFlags::from_raw(0 | ImageUsageFlags::TRANSFER_DST.as_raw() | TEX_USAGE.as_raw());

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
        let modifiers = self.load_drm_format(phy_dev, format, &modifier_props)?;
        if shm.is_some() || modifiers.is_not_empty() {
            dst.insert(
                format.drm,
                VulkanFormat {
                    format,
                    modifiers,
                    shm,
                    features: format_properties.format_properties.optimal_tiling_features,
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
    ) -> Result<Option<VulkanShmFormat>, VulkanError> {
        if !props
            .format_properties
            .optimal_tiling_features
            .contains(SHM_FEATURES)
        {
            return Ok(None);
        }
        let format_info = PhysicalDeviceImageFormatInfo2::builder()
            .ty(ImageType::TYPE_2D)
            .format(format.vk_format)
            .tiling(ImageTiling::OPTIMAL)
            .usage(SHM_USAGE);
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
        Ok(Some(VulkanShmFormat {
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
            let render_max_extents = self.get_max_extents(
                phy_dev,
                format,
                FRAMEBUFFER_FEATURES,
                FRAMEBUFFER_USAGE,
                &modifier,
            )?;
            let features = match format.is_ycbcr {
                true => YCBCR_TEX_FEATURES,
                false => TEX_FEATURES,
            };
            let texture_max_extents =
                self.get_max_extents(phy_dev, format, features, TEX_USAGE, &modifier)?;
            mods.insert(
                modifier.drm_format_modifier,
                VulkanModifier {
                    modifier: modifier.drm_format_modifier,
                    planes: modifier.drm_format_modifier_plane_count as _,
                    features: modifier.drm_format_modifier_tiling_features,
                    render_max_extents,
                    texture_max_extents,
                },
            );
        }
        Ok(mods)
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
