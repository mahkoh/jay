use {
    crate::{
        backend::BackendTransferFunction,
        cmm::cmm_luminance::Luminance,
        ifs::head_management::HeadState,
        wire::{
            jay_head_ext_brightness_info_v1::{
                Brightness, DefaultBrightness, ImpliedDefaultBrightness,
                JayHeadExtBrightnessInfoV1RequestHandler,
            },
            jay_head_manager_ext_brightness_info_v1::JayHeadManagerExtBrightnessInfoV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_brightness_info_v1! {
    version = 1,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_implied_default_brightness(shared);
        self.send_brightness(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        if shared.transfer_function != tran.transfer_function {
            self.send_implied_default_brightness(shared);
        }
        if shared.brightness != tran.brightness {
            self.send_brightness(shared);
        }
    }

    pub(in super::super) fn send_implied_default_brightness(&self, shared: &HeadState) {
        let lux = match shared.transfer_function {
            BackendTransferFunction::Default => shared
                .monitor_info
                .as_ref()
                .and_then(|m| m.luminance.as_ref())
                .map(|l| l.max)
                .unwrap_or(Luminance::SRGB.white.0),
            BackendTransferFunction::Pq => Luminance::ST2084_PQ.white.0,
        };
        self.client.event(ImpliedDefaultBrightness {
            self_id: self.id,
            lux: (lux as f32).to_bits(),
        })
    }

    pub(in super::super) fn send_brightness(&self, shared: &HeadState) {
        match shared.brightness {
            None => self.client.event(DefaultBrightness { self_id: self.id }),
            Some(b) => self.client.event(Brightness {
                self_id: self.id,
                lux: (b as f32).to_bits(),
            }),
        }
    }
}

impl JayHeadManagerExtBrightnessInfoV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtBrightnessInfoV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();
}

error!();
