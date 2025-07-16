use {
    crate::{
        ifs::head_management::HeadState,
        wire::{
            jay_head_ext_non_desktop_info_v1::{
                EffectiveDesktop, EffectiveNonDesktop, InherentDesktop, InherentNonDesktop,
                JayHeadExtNonDesktopInfoV1RequestHandler, OverrideDesktop, OverrideNonDesktop,
                Reset,
            },
            jay_head_manager_ext_non_desktop_info_v1::JayHeadManagerExtNonDesktopInfoV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_non_desktop_info_v1! {
    version = 1,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_state(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        if shared.override_non_desktop == tran.override_non_desktop {
            match (&shared.monitor_info, &tran.monitor_info) {
                (Some(s), Some(t)) if s.non_desktop == t.non_desktop => return,
                (None, None) => return,
                _ => {}
            }
        }
        self.send_state(shared);
    }

    pub(in super::super) fn send_state(&self, state: &HeadState) {
        self.client.event(Reset { self_id: self.id });
        let mut inherent_non_desktop = None;
        if let Some(monitor_info) = &state.monitor_info {
            inherent_non_desktop = Some(monitor_info.non_desktop);
            if monitor_info.non_desktop {
                self.client.event(InherentNonDesktop { self_id: self.id });
            } else {
                self.client.event(InherentDesktop { self_id: self.id });
            }
        }
        if let Some(overrd) = state.override_non_desktop {
            if overrd {
                self.client.event(OverrideNonDesktop { self_id: self.id });
            } else {
                self.client.event(OverrideDesktop { self_id: self.id });
            }
        }
        if let Some(nd) = state.override_non_desktop.or(inherent_non_desktop) {
            if nd {
                self.client.event(EffectiveNonDesktop { self_id: self.id });
            } else {
                self.client.event(EffectiveDesktop { self_id: self.id });
            }
        }
    }
}

impl JayHeadManagerExtNonDesktopInfoV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtNonDesktopInfoV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();
}

error!();
