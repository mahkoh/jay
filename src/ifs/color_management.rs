pub mod wp_color_management_output_v1;
pub mod wp_color_management_surface_feedback_v1;
pub mod wp_color_management_surface_v1;
pub mod wp_color_manager_v1;
pub mod wp_image_description_creator_icc_v1;
pub mod wp_image_description_creator_params_v1;
pub mod wp_image_description_info_v1;
pub mod wp_image_description_v1;

#[expect(dead_code)]
mod consts {
    pub(super) const RENDER_INTENT_PERCEPTUAL: u32 = 0;
    pub(super) const RENDER_INTENT_RELATIVE: u32 = 1;
    pub(super) const RENDER_INTENT_SATURATION: u32 = 2;
    pub(super) const RENDER_INTENT_ABSOLUTE: u32 = 3;
    pub(super) const RENDER_INTENT_RELATIVE_BPC: u32 = 4;

    pub(super) const FEATURE_ICC_V2_V4: u32 = 0;
    pub(super) const FEATURE_PARAMETRIC: u32 = 1;
    pub(super) const FEATURE_SET_PRIMARIES: u32 = 2;
    pub(super) const FEATURE_SET_TF_POWER: u32 = 3;
    pub(super) const FEATURE_SET_LUMINANCES: u32 = 4;
    pub(super) const FEATURE_SET_MASTERING_DISPLAY_PRIMARIES: u32 = 5;
    pub(super) const FEATURE_EXTENDED_TARGET_VOLUME: u32 = 6;
    pub(super) const FEATURE_WINDOWS_SCRGB: u32 = 7;

    pub(super) const PRIMARIES_SRGB: u32 = 1;
    pub(super) const PRIMARIES_PAL_M: u32 = 2;
    pub(super) const PRIMARIES_PAL: u32 = 3;
    pub(super) const PRIMARIES_NTSC: u32 = 4;
    pub(super) const PRIMARIES_GENERIC_FILM: u32 = 5;
    pub(super) const PRIMARIES_BT2020: u32 = 6;
    pub(super) const PRIMARIES_CIE1931_XYZ: u32 = 7;
    pub(super) const PRIMARIES_DCI_P3: u32 = 8;
    pub(super) const PRIMARIES_DISPLAY_P3: u32 = 9;
    pub(super) const PRIMARIES_ADOBE_RGB: u32 = 10;

    pub(super) const TRANSFER_FUNCTION_BT1886: u32 = 1;
    pub(super) const TRANSFER_FUNCTION_GAMMA22: u32 = 2;
    pub(super) const TRANSFER_FUNCTION_GAMMA28: u32 = 3;
    pub(super) const TRANSFER_FUNCTION_ST240: u32 = 4;
    pub(super) const TRANSFER_FUNCTION_EXT_LINEAR: u32 = 5;
    pub(super) const TRANSFER_FUNCTION_LOG_100: u32 = 6;
    pub(super) const TRANSFER_FUNCTION_LOG_316: u32 = 7;
    pub(super) const TRANSFER_FUNCTION_XVYCC: u32 = 8;
    pub(super) const TRANSFER_FUNCTION_SRGB: u32 = 9;
    pub(super) const TRANSFER_FUNCTION_EXT_SRGB: u32 = 10;
    pub(super) const TRANSFER_FUNCTION_ST2084_PQ: u32 = 11;
    pub(super) const TRANSFER_FUNCTION_ST428: u32 = 12;
    pub(super) const TRANSFER_FUNCTION_HLG: u32 = 13;

    pub(super) const CAUSE_LOW_VERSION: u32 = 0;
    pub(super) const CAUSE_UNSUPPORTED: u32 = 1;
    pub(super) const CAUSE_OPERATING_SYSTEM: u32 = 2;
    pub(super) const CAUSE_NO_OUTPUT: u32 = 3;
}
