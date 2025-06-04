use {
    super::zwlr_output_head_v1::ZwlrOutputHeadV1,
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        utils::opt::Opt,
        wire::{ZwlrOutputModeV1Id, zwlr_output_mode_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ZwlrOutputModeV1 {
    pub id: ZwlrOutputModeV1Id,
    pub version: Version,
    pub client: Rc<Client>,
    pub head: Rc<Opt<ZwlrOutputHeadV1>>,
    pub refresh: Cell<Option<i32>>,
    pub width: Cell<i32>,
    pub height: Cell<i32>,
    pub tracker: Tracker<Self>,
}

impl ZwlrOutputModeV1 {
    fn detach(&self) {
        if let Some(head) = self.head.get() {
            head.modes.remove(&self.id);
        }
    }
}
impl ZwlrOutputModeV1 {
    pub fn send_size(&self, width: i32, height: i32) {
        self.width.set(width);
        self.height.set(height);
        self.client.event(Size {
            self_id: self.id,
            width,
            height,
        });
    }

    pub fn send_refresh(&self, refresh: i32) {
        self.refresh.set(Some(refresh));
        self.client.event(Refresh {
            self_id: self.id,
            refresh,
        });
    }

    pub fn send_preferred(&self) {
        self.client.event(Preferred { self_id: self.id });
    }

    pub fn send_finished(&self) {
        self.client.event(Finished { self_id: self.id })
    }
}

impl ZwlrOutputModeV1RequestHandler for ZwlrOutputModeV1 {
    type Error = ZwlrOutputModeV1Error;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.send_finished();
        self.detach();
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
