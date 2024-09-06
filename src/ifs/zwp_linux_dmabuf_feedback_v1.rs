use {
    crate::{
        client::{Client, ClientError},
        drm_feedback::{DrmFeedback, DrmFeedbackId},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_linux_dmabuf_feedback_v1::*, ZwpLinuxDmabufFeedbackV1Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
    uapi::{c, OwnedFd},
};

pub const SCANOUT: u32 = 1;

pub struct ZwpLinuxDmabufFeedbackV1 {
    pub id: ZwpLinuxDmabufFeedbackV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub last_feedback: Cell<Option<DrmFeedbackId>>,
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
            last_feedback: Default::default(),
            surface: surface.cloned(),
            version,
        }
    }

    pub fn send_feedback(&self, feedback: &DrmFeedback) {
        if self.last_feedback.replace(Some(feedback.id)) == Some(feedback.id) {
            return;
        }
        self.send_format_table(&feedback.shared.fd, feedback.shared.size);
        self.send_main_device(feedback.shared.main_device);
        for tranch in &feedback.tranches {
            self.send_tranche_target_device(tranch.device);
            self.send_tranche_formats(&tranch.indices);
            self.send_tranche_flags(if tranch.scanout { SCANOUT } else { 0 });
            self.send_tranche_done();
        }
        self.send_done();
    }

    fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    fn send_format_table(&self, fd: &Rc<OwnedFd>, size: usize) {
        self.client.event(FormatTable {
            self_id: self.id,
            fd: fd.clone(),
            size: size as _,
        });
    }

    fn send_main_device(&self, dev: c::dev_t) {
        self.client.event(MainDevice {
            self_id: self.id,
            device: dev,
        });
    }

    fn send_tranche_done(&self) {
        self.client.event(TrancheDone { self_id: self.id });
    }

    fn send_tranche_target_device(&self, dev: c::dev_t) {
        self.client.event(TrancheTargetDevice {
            self_id: self.id,
            device: dev,
        });
    }

    fn send_tranche_formats(&self, indices: &[u16]) {
        self.client.event(TrancheFormats {
            self_id: self.id,
            indices,
        });
    }

    fn send_tranche_flags(&self, flags: u32) {
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
        self.client
            .state
            .drm_feedback_consumers
            .remove(&(self.client.id, self.id));
        if let Some(surface) = &self.surface {
            surface.drm_feedback.remove(&self.id);
        }
    }
}

object_base! {
    self = ZwpLinuxDmabufFeedbackV1;
    version = self.version;
}

impl Object for ZwpLinuxDmabufFeedbackV1 {
    fn break_loops(&self) {
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
