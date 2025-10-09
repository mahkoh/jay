use {
    crate::{
        client::Client,
        ifs::ipc::{
            IpcLocation, SourceData,
            data_control::{
                private::{
                    DataControlSource, DataControlSourceData,
                    logic::{self, DataControlError},
                },
                zwlr_data_control_device_v1::WlrDataControlIpc,
            },
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwlrDataControlSourceV1Id, zwlr_data_control_source_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
    uapi::OwnedFd,
};

pub struct ZwlrDataControlSourceV1 {
    pub id: ZwlrDataControlSourceV1Id,
    pub data: DataControlSourceData,
    pub tracker: Tracker<Self>,
}

impl DataControlSource for ZwlrDataControlSourceV1 {
    type Ipc = WlrDataControlIpc;

    fn data(&self) -> &DataControlSourceData {
        &self.data
    }

    fn send_cancelled(&self) {
        self.send_cancelled();
    }

    fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        self.send_send(mime_type, fd);
    }
}

impl ZwlrDataControlSourceV1 {
    pub fn new(id: ZwlrDataControlSourceV1Id, client: &Rc<Client>, version: Version) -> Self {
        Self {
            id,
            data: DataControlSourceData {
                data: SourceData::new(client),
                version,
                location: Cell::new(IpcLocation::Clipboard),
                used: Cell::new(false),
            },
            tracker: Default::default(),
        }
    }

    pub fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        self.data.data.client.event(Send {
            self_id: self.id,
            mime_type,
            fd,
        })
    }

    pub fn send_cancelled(&self) {
        self.data.data.client.event(Cancelled { self_id: self.id })
    }
}

impl ZwlrDataControlSourceV1RequestHandler for ZwlrDataControlSourceV1 {
    type Error = ZwlrDataControlSourceV1Error;

    fn offer(&self, req: Offer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        logic::data_source_offer(self, req.mime_type)?;
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        logic::data_source_destroy(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwlrDataControlSourceV1;
    version = self.data.version;
}

impl Object for ZwlrDataControlSourceV1 {
    fn break_loops(self: Rc<Self>) {
        logic::data_source_break_loops(&*self);
    }
}

dedicated_add_obj!(
    ZwlrDataControlSourceV1,
    ZwlrDataControlSourceV1Id,
    zwlr_data_sources
);

#[derive(Debug, Error)]
pub enum ZwlrDataControlSourceV1Error {
    #[error(transparent)]
    DataControlError(#[from] DataControlError),
}
