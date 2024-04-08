use {
    crate::{
        client::Client,
        ifs::{wl_output::WlOutput, wl_surface::WlSurface},
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_presentation_feedback::*, WpPresentationFeedbackId},
    },
    std::{convert::Infallible, rc::Rc},
    thiserror::Error,
};

pub struct WpPresentationFeedback {
    pub id: WpPresentationFeedbackId,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

pub const KIND_VSYNC: u32 = 0x1;
#[allow(dead_code)]
pub const KIND_HW_CLOCK: u32 = 0x2;
pub const KIND_HW_COMPLETION: u32 = 0x4;
#[allow(dead_code)]
pub const KIND_ZERO_COPY: u32 = 0x8;

impl WpPresentationFeedback {
    pub fn send_sync_output(&self, output: &WlOutput) {
        self.client.event(SyncOutput {
            self_id: self.id,
            output: output.id,
        });
    }

    pub fn send_presented(&self, tv_sec: u64, tv_nsec: u32, refresh: u32, seq: u64, flags: u32) {
        self.client.event(Presented {
            self_id: self.id,
            tv_sec_hi: (tv_sec >> 32) as u32,
            tv_sec_lo: tv_sec as u32,
            tv_nsec,
            refresh,
            seq_hi: (seq >> 32) as u32,
            seq_lo: seq as u32,
            flags,
        });
    }

    pub fn send_discarded(&self) {
        self.client.event(Discarded { self_id: self.id });
    }
}

impl WpPresentationFeedbackRequestHandler for WpPresentationFeedback {
    type Error = Infallible;
}

object_base! {
    self = WpPresentationFeedback;
    version = self.version;
}

impl Object for WpPresentationFeedback {}

simple_add_obj!(WpPresentationFeedback);

#[derive(Debug, Error)]
pub enum WpPresentationFeedbackError {}
