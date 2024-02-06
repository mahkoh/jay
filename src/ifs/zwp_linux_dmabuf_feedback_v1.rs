use {
    crate::{
        client::{Client, ClientError},
        drm_feedback::DrmFeedback,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwp_linux_dmabuf_feedback_v1::*, ZwpLinuxDmabufFeedbackV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::{c, OwnedFd},
};

#[allow(dead_code)]
pub const SCANOUT: u32 = 1;

pub struct ZwpLinuxDmabufFeedbackV1 {
    pub id: ZwpLinuxDmabufFeedbackV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl ZwpLinuxDmabufFeedbackV1 {
    pub fn new(id: ZwpLinuxDmabufFeedbackV1Id, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
        }
    }

    pub fn send_feedback(&self, feedback: &DrmFeedback) {
        self.send_format_table(&feedback.fd, feedback.size);
        self.send_main_device(feedback.main_device);
        self.send_tranche_target_device(feedback.main_device);
        self.send_tranche_formats(&feedback.indices);
        self.send_tranche_flags(0);
        self.send_tranche_done();
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

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpLinuxDmabufFeedbackV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn detach(&self) {
        self.client
            .state
            .drm_feedback_consumers
            .remove(&(self.client.id, self.id));
    }
}

object_base! {
    self = ZwpLinuxDmabufFeedbackV1;

    DESTROY => destroy,
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
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(ZwpLinuxDmabufFeedbackV1Error, ClientError);
efrom!(ZwpLinuxDmabufFeedbackV1Error, MsgParserError);
