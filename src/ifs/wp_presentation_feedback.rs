use {
    crate::{
        client::Client,
        ifs::{wl_output::WlOutput, wl_surface::WlSurface},
        leaks::Tracker,
        object::{Object, Version},
        wire::{WlOutputId, WpPresentationFeedbackId, wp_presentation_feedback::*},
    },
    ahash::AHashMap,
    std::{convert::Infallible, rc::Rc},
};

pub struct PresentationFeedback {
    fb: Option<Rc<WpPresentationFeedback>>,
}

impl PresentationFeedback {
    pub fn new(fb: Rc<WpPresentationFeedback>) -> Self {
        Self { fb: Some(fb) }
    }

    pub fn presented(
        mut self,
        outputs: Option<&AHashMap<WlOutputId, Rc<WlOutput>>>,
        tv_sec: u64,
        tv_nsec: u32,
        mut refresh: u32,
        seq: u64,
        flags: u32,
        vrr: bool,
    ) {
        if let Some(fb) = self.fb.take() {
            if let Some(outputs) = outputs {
                for output in outputs.values() {
                    fb.send_sync_output(output);
                }
            }
            if vrr && fb.version < VRR_REFRESH_SINCE {
                refresh = 0;
            }
            fb.send_presented(tv_sec, tv_nsec, refresh, seq, flags);
            let _ = fb.client.remove_obj(&*fb);
        }
    }
}

impl Drop for PresentationFeedback {
    fn drop(&mut self) {
        if let Some(fb) = self.fb.take() {
            fb.send_discarded();
            let _ = fb.client.remove_obj(&*fb);
        }
    }
}

pub struct WpPresentationFeedback {
    pub id: WpPresentationFeedbackId,
    pub client: Rc<Client>,
    pub _surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

pub const KIND_VSYNC: u32 = 0x1;
pub const KIND_HW_CLOCK: u32 = 0x2;
pub const KIND_HW_COMPLETION: u32 = 0x4;
pub const KIND_ZERO_COPY: u32 = 0x8;

pub const VRR_REFRESH_SINCE: Version = Version(2);

impl WpPresentationFeedback {
    pub fn send_sync_output(&self, output: &WlOutput) {
        self.client.event(SyncOutput {
            self_id: self.id,
            output: output.id,
        });
    }

    fn send_presented(&self, tv_sec: u64, tv_nsec: u32, refresh: u32, seq: u64, flags: u32) {
        self.client.event(Presented {
            self_id: self.id,
            tv_sec,
            tv_nsec,
            refresh,
            seq,
            flags,
        });
    }

    fn send_discarded(&self) {
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
