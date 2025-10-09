use {
    crate::{
        backend::{BackendColorSpace, BackendEotfs},
        ifs::head_management::{HeadOp, HeadState},
        video::drm::{
            DRM_MODE_COLORIMETRY_BT2020_RGB, DRM_MODE_COLORIMETRY_DEFAULT, HDMI_EOTF_SMPTE_ST2084,
            HDMI_EOTF_TRADITIONAL_GAMMA_SDR,
        },
        wire::{
            jay_head_ext_drm_color_space_setter_v1::{
                JayHeadExtDrmColorSpaceSetterV1RequestHandler, Reset, SetColorimetry, SetHdmiEotf,
                SupportedColorimetry, SupportedHdmiEotf,
            },
            jay_head_manager_ext_drm_color_space_setter_v1::JayHeadManagerExtDrmColorSpaceSetterV1RequestHandler,
        },
    },
    isnt::std_1::primitive::IsntSliceExt,
    std::rc::Rc,
};

impl_drm_color_space_setter_v1! {
    version = 1,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_supported(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        if shared.monitor_info != tran.monitor_info {
            self.send_supported(shared);
        }
    }

    pub(in super::super) fn send_supported(&self, state: &HeadState) {
        self.client.event(Reset { self_id: self.id });
        let Some(mi) = &state.monitor_info else {
            return;
        };
        self.send_supported_eotf(HDMI_EOTF_TRADITIONAL_GAMMA_SDR);
        for tf in &mi.eotfs {
            self.send_supported_eotf(tf.to_drm());
        }
        self.send_supported_colorimetry(DRM_MODE_COLORIMETRY_DEFAULT);
        for cs in &mi.color_spaces {
            self.send_supported_colorimetry(cs.to_drm());
        }
    }

    fn send_supported_eotf(&self, eotf: u8) {
        self.client.event(SupportedHdmiEotf {
            self_id: self.id,
            eotf: eotf as u32,
        });
    }

    fn send_supported_colorimetry(&self, colorimetry: u64) {
        self.client.event(SupportedColorimetry {
            self_id: self.id,
            colorimetry: colorimetry as u32,
        });
    }
}

impl JayHeadManagerExtDrmColorSpaceSetterV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtDrmColorSpaceSetterV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();

    fn set_hdmi_eotf(&self, req: SetHdmiEotf, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        const DEFAULT: u32 = HDMI_EOTF_TRADITIONAL_GAMMA_SDR as u32;
        const PQ: u32 = HDMI_EOTF_SMPTE_ST2084 as u32;
        let eotf = match req.eotf {
            DEFAULT => BackendEotfs::Default,
            PQ => BackendEotfs::Pq,
            _ => return Err(ErrorName::UnknownEotf(req.eotf)),
        };
        if eotf != BackendEotfs::Default {
            let state = &*self.common.transaction_state.borrow();
            let Some(mi) = &state.monitor_info else {
                return Err(ErrorName::UnsupportedEotf(req.eotf));
            };
            if mi.eotfs.not_contains(&eotf) {
                return Err(ErrorName::UnsupportedEotf(req.eotf));
            }
        }
        self.common.push_op(HeadOp::SetEotf(eotf))?;
        Ok(())
    }

    fn set_colorimetry(&self, req: SetColorimetry, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let cs = match req.colorimetry as u64 {
            DRM_MODE_COLORIMETRY_DEFAULT => BackendColorSpace::Default,
            DRM_MODE_COLORIMETRY_BT2020_RGB => BackendColorSpace::Bt2020,
            _ => return Err(ErrorName::UnknownColorimetry(req.colorimetry)),
        };
        if cs != BackendColorSpace::Default {
            let state = &*self.common.transaction_state.borrow();
            let Some(mi) = &state.monitor_info else {
                return Err(ErrorName::UnsupportedColorimetry(req.colorimetry));
            };
            if mi.color_spaces.not_contains(&cs) {
                return Err(ErrorName::UnsupportedColorimetry(req.colorimetry));
            }
        }
        self.common.push_op(HeadOp::SetColorSpace(cs))?;
        Ok(())
    }
}

error! {
    #[error("Unknown EOTF {0}")]
    UnknownEotf(u32),
    #[error("Unknown colorimetry {0}")]
    UnknownColorimetry(u32),
    #[error("Unsupported EOTF {0}")]
    UnsupportedEotf(u32),
    #[error("Unsupported colorimetry {0}")]
    UnsupportedColorimetry(u32),
}
