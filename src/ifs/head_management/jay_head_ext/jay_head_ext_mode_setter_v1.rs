use {
    crate::{
        backend::CONCAP_MODE_SETTING,
        ifs::head_management::{HeadCommon, HeadOp, HeadState},
        state::ConnectorData,
        wire::{
            jay_head_ext_mode_setter_v1::{
                JayHeadExtModeSetterV1RequestHandler, Mode, Reset, SetMode,
            },
            jay_head_manager_ext_mode_setter_v1::JayHeadManagerExtModeSetterV1RequestHandler,
        },
    },
    std::rc::Rc,
};

impl_mode_setter_v1! {
    version = 1,
    filter = filter,
    after_announce = after_announce,
    after_transaction = after_transaction,
}

impl MgrName {
    fn filter(&self, connector: &ConnectorData, _common: &Rc<HeadCommon>) -> bool {
        connector.connector.caps().contains(CONCAP_MODE_SETTING)
    }
}

impl HeadName {
    fn after_announce(&self, shared: &HeadState) {
        self.send_modes(shared);
    }

    fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
        match (&shared.monitor_info, &tran.monitor_info) {
            (Some(s), Some(t)) if s != t => {}
            _ => return,
        }
        self.send_modes(shared);
    }

    pub(in super::super) fn send_modes(&self, state: &HeadState) {
        self.client.event(Reset { self_id: self.id });
        if let Some(mi) = &state.monitor_info {
            for mode in &mi.modes {
                self.client.event(Mode {
                    self_id: self.id,
                    width: mode.width,
                    height: mode.height,
                    refresh_mhz: mode.refresh_rate_millihz,
                })
            }
        }
    }
}

impl JayHeadManagerExtModeSetterV1RequestHandler for MgrName {
    type Error = ErrorName;

    mgr_common_req!();
}

impl JayHeadExtModeSetterV1RequestHandler for HeadName {
    type Error = ErrorName;

    head_common_req!();

    fn set_mode(&self, req: SetMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.common.assert_in_transaction()?;
        let num_modes = self
            .common
            .snapshot_state
            .borrow()
            .monitor_info
            .as_deref()
            .map(|i| i.modes.len())
            .unwrap_or(0);
        let idx = req.idx as usize;
        if idx >= num_modes {
            return Err(JayHeadExtModeSetterV1Error::ModeOutOfBounds);
        }
        self.common.push_op(HeadOp::SetMode(idx))?;
        Ok(())
    }
}

error! {
    #[error("The mode is out of bounds")]
    ModeOutOfBounds,
}
