use {
    super::{
        zwlr_output_configuration_head::ZwlrOutputConfigurationHeadV1,
        zwlr_output_mode_v1::ZwlrOutputModeV1,
    },
    crate::{
        backend,
        client::{Client, ClientError},
        fixed::Fixed,
        ifs::{
            output_manager::{
                zwlr_output_configuration_v1::OutputConfigurationId,
                zwlr_output_manager_v1::{OutputManagerId, ZwlrOutputManagerV1},
            },
            wl_output::{
                OutputGlobalOpt, TF_90, TF_180, TF_270, TF_FLIPPED, TF_FLIPPED_90, TF_FLIPPED_180,
                TF_FLIPPED_270, TF_NORMAL,
            },
        },
        leaks::Tracker,
        object::{Object, Version},
        scale,
        utils::{copyhashmap::CopyHashMap, opt::Opt},
        wire::{ZwlrOutputHeadV1Id, ZwlrOutputModeV1Id, zwlr_output_head_v1::*},
    },
    jay_config::video,
    std::{cell::Cell, rc::Rc},
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

pub struct ZwlrOutputHeadV1 {
    pub id: ZwlrOutputHeadV1Id,
    pub version: Version,
    pub client: Rc<Client>,
    pub manager_id: OutputManagerId,
    pub manager: Rc<Opt<ZwlrOutputManagerV1>>,
    pub output: Rc<OutputGlobalOpt>,
    pub configuration_heads:
        CopyHashMap<OutputConfigurationId, (Rc<bool>, Option<Rc<ZwlrOutputConfigurationHeadV1>>)>,
    pub current_mode: Rc<Opt<ZwlrOutputModeV1>>,
    pub modes: CopyHashMap<ZwlrOutputModeV1Id, Rc<ZwlrOutputModeV1>>,
    pub opt: Rc<Opt<ZwlrOutputHeadV1>>,
    pub tracker: Tracker<Self>,
}

impl ZwlrOutputHeadV1 {
    fn detach(&self) {
        if let Some(output) = self.output.node() {
            output.zwlr_output_heads.remove(&self.manager_id);
        }
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
        let transform = match transform {
            video::Transform::None => TF_NORMAL,
            video::Transform::Rotate90 => TF_90,
            video::Transform::Rotate180 => TF_180,
            video::Transform::Rotate270 => TF_270,
            video::Transform::Flip => TF_FLIPPED,
            video::Transform::FlipRotate90 => TF_FLIPPED_90,
            video::Transform::FlipRotate180 => TF_FLIPPED_180,
            video::Transform::FlipRotate270 => TF_FLIPPED_270,
        };
        self.client.event(Transform {
            self_id: self.id,
            transform,
        });
    }

    pub fn send_scale(&self, scale: Fixed) {
        self.client.event(Scale {
            self_id: self.id,
            scale: scale,
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

    pub fn send_adaptive_sync(&self, enabled: bool) {
        let state = if enabled {
            ADAPTIVE_SYNC_STATE_ENABLED
        } else {
            ADAPTIVE_SYNC_STATE_DISABLED
        };
        self.client.event(AdaptiveSync {
            self_id: self.id,
            state,
        });
    }

    pub fn publish_mode(
        self: &Rc<Self>,
        mode: backend::Mode,
        current: bool,
    ) -> Option<Rc<ZwlrOutputModeV1>> {
        let id: ZwlrOutputModeV1Id = match self.client.new_id() {
            Ok(i) => i,
            Err(e) => {
                self.client.error(e);
                return None;
            }
        };

        let refresh = match mode.refresh_rate_millihz {
            0 => None,
            _ => Some(mode.refresh_rate_millihz as i32),
        };

        let output_mode = Rc::new(ZwlrOutputModeV1 {
            id,
            head: self.opt.clone(),
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            refresh: Cell::new(refresh),
            width: Cell::new(mode.width),
            height: Cell::new(mode.height),
        });

        track!(self.client, output_mode);
        self.client.add_server_obj(&output_mode);
        self.modes.set(output_mode.id, output_mode.clone());
        self.send_mode(&output_mode);
        if current {
            self.send_current_mode(&output_mode);
            self.current_mode.set(Some(output_mode.clone()));
        }
        if let Some(refresh) = refresh {
            output_mode.send_refresh(refresh);
        }

        output_mode.send_size(mode.width, mode.height);
        Some(output_mode)
    }

    pub fn hande_new_transform(&self, transform: video::Transform) {
        if let Some(manager) = self.manager.get() {
            self.send_transform(transform);
            manager.schedule_done();
        }
    }

    pub fn handle_new_mode(&self, old: backend::Mode, new: backend::Mode) {
        if let Some(manager) = self.manager.get() {
            let mut manager_done = false;
            let modes = self.modes.lock();

            let old_output_mode = modes.values().find(|m| {
                let head_mode = backend::Mode {
                    width: m.width.get(),
                    height: m.height.get(),
                    refresh_rate_millihz: m.refresh.get().unwrap_or_default() as u32,
                };
                head_mode == old
            });
            let new_output_mode = modes.values().find(|m| {
                let head_mode = backend::Mode {
                    width: m.width.get(),
                    height: m.height.get(),
                    refresh_rate_millihz: m.refresh.get().unwrap_or_default() as u32,
                };
                head_mode == new
            });
            if let Some(output_mode) = new_output_mode {
                self.send_current_mode(&output_mode);
                manager_done = true;
            } else {
                if let Some(output_mode) = old_output_mode {
                    if let Some(current_mode) = self.current_mode.get() {
                        if current_mode.id == output_mode.id {
                            if (new.width, new.height) != (old.width, old.height) {
                                output_mode.send_size(new.width, new.height);
                            }
                            if old.refresh_rate_millihz != new.refresh_rate_millihz {
                                output_mode.send_refresh(new.refresh_rate_millihz as i32);
                            }
                            manager_done = true;
                        }
                    }
                }
            }
            if manager_done {
                manager.schedule_done();
            }
        }
    }

    pub fn handle_new_position(&self, x: i32, y: i32) {
        if let Some(manager) = self.manager.get() {
            self.send_position(x, y);
            manager.schedule_done();
        }
    }

    pub fn handle_new_adaptive_sync_state(&self, state: bool) {
        if self.version < ADAPTIVE_SYNC_SINCE {
            return;
        }
        if let Some(manager) = self.manager.get() {
            self.send_adaptive_sync(state);
            manager.schedule_done();
        }
    }

    pub fn handle_new_scale(&self, scale: scale::Scale) {
        if let Some(manager) = self.manager.get() {
            let scale = Fixed::from_f64(scale.to_f64());
            self.send_scale(scale);
            manager.schedule_done();
        }
    }

    pub fn handle_enabled_change(&self, enabled: bool) {
        if let Some(manager) = self.manager.get() {
            self.send_enabled(enabled);
            manager.schedule_done();
        }
    }

    pub fn handle_destroyed(&self) {
        if let Some(manager) = self.manager.get() {
            self.send_finished();
            manager.schedule_done();
        }
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
    fn break_loops(&self) {
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
