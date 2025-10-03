use {
    crate::{
        backend::{self, ConnectorId},
        client::{Client, ClientError},
        fixed::Fixed,
        ifs::wlr_output_manager::{
            zwlr_output_manager_v1::{WlrOutputManagerId, ZwlrOutputManagerV1},
            zwlr_output_mode_v1::ZwlrOutputModeV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        scale,
        state::OutputData,
        tree::VrrMode,
        utils::transform_ext::TransformExt,
        wire::{ZwlrOutputHeadV1Id, zwlr_output_head_v1::*},
    },
    ahash::AHashMap,
    jay_config::video,
    std::rc::Rc,
    thiserror::Error,
};

pub const MAKE_SINCE: Version = Version(2);
pub const MODEL_SINCE: Version = Version(2);
pub const SERIAL_NUMBER_SINCE: Version = Version(2);
#[expect(dead_code)]
pub const RELEASE_SINCE: Version = Version(3);
pub const ADAPTIVE_SYNC_SINCE: Version = Version(4);

pub const HEAD_DISABLED: i32 = 0;
pub const HEAD_ENABLED: i32 = 1;

pub const ADAPTIVE_SYNC_STATE_DISABLED: u32 = 0;
pub const ADAPTIVE_SYNC_STATE_ENABLED: u32 = 1;

linear_ids!(WlrOutputHeadIds, WlrOutputHeadId, u64);

pub struct ZwlrOutputHeadV1 {
    pub(super) id: ZwlrOutputHeadV1Id,
    pub(super) version: Version,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) output: Rc<OutputData>,
    pub(super) manager_id: WlrOutputManagerId,
    pub(super) manager: Rc<ZwlrOutputManagerV1>,
    pub(super) head_id: WlrOutputHeadId,
    pub(super) connector_id: ConnectorId,
    pub(super) modes: AHashMap<backend::Mode, Rc<ZwlrOutputModeV1>>,
}

impl ZwlrOutputHeadV1 {
    fn detach(&self) {
        self.output
            .connector
            .wlr_output_heads
            .remove(&self.manager_id);
    }
}

impl ZwlrOutputHeadV1 {
    pub fn send_name(&self, name: &str) {
        self.client.event(Name {
            self_id: self.id,
            name,
        });
    }

    pub fn send_description(&self, description: &str) {
        self.client.event(Description {
            self_id: self.id,
            description,
        });
    }

    pub fn send_physical_size(&self, width: i32, height: i32) {
        self.client.event(PhysicalSize {
            self_id: self.id,
            width,
            height,
        });
    }

    pub fn send_mode(&self, mode: &ZwlrOutputModeV1) {
        self.client.event(Mode {
            self_id: self.id,
            mode: mode.id,
        });
    }

    pub fn send_enabled(&self, enabled: bool) {
        let enabled = if enabled { HEAD_ENABLED } else { HEAD_DISABLED };
        self.client.event(Enabled {
            self_id: self.id,
            enabled,
        });
    }

    pub fn send_current_mode(&self, mode: &ZwlrOutputModeV1) {
        self.client.event(CurrentMode {
            self_id: self.id,
            mode: mode.id,
        });
    }

    pub fn send_position(&self, x: i32, y: i32) {
        self.client.event(Position {
            self_id: self.id,
            x,
            y,
        });
    }

    pub fn send_transform(&self, transform: video::Transform) {
        self.client.event(Transform {
            self_id: self.id,
            transform: transform.to_wl(),
        });
    }

    pub fn send_scale(&self, scale: scale::Scale) {
        let scale = Fixed::from_f64(scale.to_f64());
        self.client.event(Scale {
            self_id: self.id,
            scale,
        });
    }

    pub fn send_finished(&self) {
        self.client.event(Finished { self_id: self.id })
    }

    pub fn send_make(&self, make: &str) {
        self.client.event(Make {
            self_id: self.id,
            make,
        });
    }

    pub fn send_model(&self, model: &str) {
        self.client.event(Model {
            self_id: self.id,
            model,
        });
    }

    pub fn send_serial_number(&self, serial_number: &str) {
        self.client.event(SerialNumber {
            self_id: self.id,
            serial_number,
        });
    }

    pub fn send_adaptive_sync(&self, mode: &VrrMode) {
        let state = if *mode == VrrMode::Always {
            ADAPTIVE_SYNC_STATE_ENABLED
        } else {
            ADAPTIVE_SYNC_STATE_DISABLED
        };
        self.client.event(AdaptiveSync {
            self_id: self.id,
            state,
        });
    }

    pub fn announce_modes(&self, modes: &[Rc<ZwlrOutputModeV1>]) {
        for mode in modes {
            self.send_mode(mode);
            mode.send();
            if mode.initial_current {
                self.send_current_mode(mode);
            }
        }
    }

    pub fn hande_transform_change(&self, transform: video::Transform) {
        self.send_transform(transform);
        self.manager.schedule_done();
    }

    pub fn handle_mode_change(&self, new: backend::Mode) {
        let Some(mode) = self.modes.get(&new) else {
            return;
        };
        if mode.destroyed.get() {
            return;
        }
        self.send_current_mode(mode);
        self.manager.schedule_done();
    }

    pub fn handle_position_change(&self, x: i32, y: i32) {
        self.send_position(x, y);
        self.manager.schedule_done();
    }

    pub fn handle_vrr_mode_change(&self, mode: &VrrMode) {
        if self.version < ADAPTIVE_SYNC_SINCE {
            return;
        }
        self.send_adaptive_sync(mode);
        self.manager.schedule_done();
    }

    pub fn handle_new_scale(&self, scale: scale::Scale) {
        self.send_scale(scale);
        self.manager.schedule_done();
    }

    pub fn handle_disconnected(&self) {
        self.send_finished();
        for mode in self.modes.values() {
            if !mode.destroyed.get() {
                mode.send_finished();
            }
        }
        self.manager.schedule_done();
    }
}

impl ZwlrOutputHeadV1RequestHandler for ZwlrOutputHeadV1 {
    type Error = ZwlrOutputHeadV1Error;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.send_finished();
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwlrOutputHeadV1;
    version = self.version;
}

impl Object for ZwlrOutputHeadV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}

dedicated_add_obj!(ZwlrOutputHeadV1, ZwlrOutputHeadV1Id, zwlr_output_heads);

#[derive(Debug, Error)]
pub enum ZwlrOutputHeadV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwlrOutputHeadV1Error, ClientError);
