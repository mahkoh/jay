use {
    crate::{
        client::Client,
        ifs::ipc::{
            IpcLocation, SourceData,
            data_control::{
                ext_data_control_device_v1::ExtDataControlIpc,
                private::{
                    DataControlSource, DataControlSourceData,
                    logic::{self, DataControlError},
                },
            },
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ExtDataControlSourceV1Id, ext_data_control_source_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
    uapi::OwnedFd,
};

pub struct ExtDataControlSourceV1 {
    pub id: ExtDataControlSourceV1Id,
    pub data: DataControlSourceData,
    pub tracker: Tracker<Self>,
}

impl DataControlSource for ExtDataControlSourceV1 {
    type Ipc = ExtDataControlIpc;

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

impl ExtDataControlSourceV1 {
    pub fn new(id: ExtDataControlSourceV1Id, client: &Rc<Client>, version: Version) -> Self {
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

impl ExtDataControlSourceV1RequestHandler for ExtDataControlSourceV1 {
    type Error = ExtDataControlSourceV1Error;

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
    self = ExtDataControlSourceV1;
    version = self.data.version;
}

impl Object for ExtDataControlSourceV1 {
    fn break_loops(self: Rc<Self>) {
        logic::data_source_break_loops(&*self);
    }
}

dedicated_add_obj!(
    ExtDataControlSourceV1,
    ExtDataControlSourceV1Id,
    ext_data_sources
);

#[derive(Debug, Error)]
pub enum ExtDataControlSourceV1Error {
    #[error(transparent)]
    DataControlError(#[from] DataControlError),
}
