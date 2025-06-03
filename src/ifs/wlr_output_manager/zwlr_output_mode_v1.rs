use {
    crate::{
        backend::Mode,
        client::{Client, ClientError},
        ifs::wlr_output_manager::zwlr_output_head_v1::WlrOutputHeadId,
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwlrOutputModeV1Id, zwlr_output_mode_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ZwlrOutputModeV1 {
    pub(super) id: ZwlrOutputModeV1Id,
    pub(super) head_id: WlrOutputHeadId,
    pub(super) version: Version,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) mode: Mode,
    pub(super) preferred: bool,
    pub(super) initial_current: bool,
    pub(super) destroyed: Cell<bool>,
}

impl ZwlrOutputModeV1 {
    pub fn send(&self) {
        self.send_size(self.mode.width, self.mode.height);
        self.send_refresh(self.mode.refresh_rate_millihz as _);
        if self.preferred {
            self.send_preferred();
        }
    }

    fn send_size(&self, width: i32, height: i32) {
        self.client.event(Size {
            self_id: self.id,
            width,
            height,
        });
    }

    fn send_refresh(&self, refresh: i32) {
        self.client.event(Refresh {
            self_id: self.id,
            refresh,
        });
    }

    fn send_preferred(&self) {
        self.client.event(Preferred { self_id: self.id });
    }

    pub fn send_finished(&self) {
        self.client.event(Finished { self_id: self.id })
    }
}

impl ZwlrOutputModeV1RequestHandler for ZwlrOutputModeV1 {
    type Error = ZwlrOutputModeV1Error;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroyed.set(true);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwlrOutputModeV1;
    version = self.version;
}

impl Object for ZwlrOutputModeV1 {}

dedicated_add_obj!(ZwlrOutputModeV1, ZwlrOutputModeV1Id, zwlr_output_modes);

#[derive(Debug, Error)]
pub enum ZwlrOutputModeV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwlrOutputModeV1Error, ClientError);
