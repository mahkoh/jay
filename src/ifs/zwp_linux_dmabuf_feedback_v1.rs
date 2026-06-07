use {
    crate::{
        client::{Client, ClientError},
        dmabuf_feedback::DmaBufFeedbackId,
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwpLinuxDmabufFeedbackV1Id, zwp_linux_dmabuf_feedback_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
    uapi::{OwnedFd, c},
};

pub const FB_SCANOUT: u32 = 1;
pub const FB_SAMPLING: u32 = 2;

pub struct ZwpLinuxDmabufFeedbackV1 {
    pub id: ZwpLinuxDmabufFeedbackV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub last_format_table: Cell<Option<DmaBufFeedbackId>>,
    pub surface: Option<Rc<WlSurface>>,
    pub version: Version,
}

impl ZwpLinuxDmabufFeedbackV1 {
    pub fn new(
        id: ZwpLinuxDmabufFeedbackV1Id,
        client: &Rc<Client>,
        surface: Option<&Rc<WlSurface>>,
        version: Version,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            last_format_table: Default::default(),
            surface: surface.cloned(),
            version,
        }
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_format_table(&self, fd: &Rc<OwnedFd>, size: u32) {
        self.client.event(FormatTable {
            self_id: self.id,
            fd: fd.clone(),
            size,
        });
    }

    pub fn send_main_device(&self, dev: c::dev_t) {
        self.client.event(MainDevice {
            self_id: self.id,
            device: dev,
        });
    }

    pub fn send_tranche_done(&self) {
        self.client.event(TrancheDone { self_id: self.id });
    }

    pub fn send_tranche_target_device(&self, dev: c::dev_t) {
        self.client.event(TrancheTargetDevice {
            self_id: self.id,
            device: dev,
        });
    }

    pub fn send_tranche_formats(&self, indices: &[u16]) {
        self.client.event(TrancheFormats {
            self_id: self.id,
            indices,
        });
    }

    pub fn send_tranche_flags(&self, flags: u32) {
        self.client.event(TrancheFlags {
            self_id: self.id,
            flags,
        });
    }
}

impl ZwpLinuxDmabufFeedbackV1RequestHandler for ZwpLinuxDmabufFeedbackV1 {
    type Error = ZwpLinuxDmabufFeedbackV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

impl ZwpLinuxDmabufFeedbackV1 {
    fn detach(&self) {
        if let Some(surface) = &self.surface {
            surface.dmabuf_feedback.remove(&self.id);
        } else {
            self.client
                .state
                .dmabuf_feedback
                .default
                .remove(&(self.client.id, self.id));
        }
    }
}

object_base! {
    self = ZwpLinuxDmabufFeedbackV1;
    version = self.version;
}

impl Object for ZwpLinuxDmabufFeedbackV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}

simple_add_obj!(ZwpLinuxDmabufFeedbackV1);

#[derive(Debug, Error)]
pub enum ZwpLinuxDmabufFeedbackV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpLinuxDmabufFeedbackV1Error, ClientError);
