use crate::client::Client;
use crate::ifs::wl_output::WlOutput;
use crate::ifs::wl_surface::WlSurface;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::tree::PF_HW_CLOCK;
use crate::tree::PF_HW_COMPLETION;
use crate::tree::PF_VRR;
use crate::tree::PF_VSYNC;
use crate::tree::PF_ZERO_COPY;
use crate::tree::PresentFlags;
use crate::utils::bhash::BHashMap;
use crate::wire::WlOutputId;
use crate::wire::WpPresentationFeedbackId;
use crate::wire::wp_presentation_feedback::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

pub struct PresentationFeedback {
    fb: Option<Rc<WpPresentationFeedback>>,
}

impl PresentationFeedback {
    pub fn new(fb: Rc<WpPresentationFeedback>) -> Self {
        Self { fb: Some(fb) }
    }

    pub fn presented(
        mut self,
        outputs: Option<&BHashMap<WlOutputId, Rc<WlOutput>>>,
        tv_sec: u64,
        tv_nsec: u32,
        mut refresh: u32,
        seq: u64,
        flags: PresentFlags,
    ) {
        if let Some(fb) = self.fb.take() {
            if let Some(outputs) = outputs {
                for output in outputs.values() {
                    fb.send_sync_output(output);
                }
            }
            let mut flags2 = 0;
            if flags.contains(PF_VSYNC) {
                flags2 |= KIND_VSYNC;
            }
            if flags.contains(PF_HW_CLOCK) {
                flags2 |= KIND_HW_CLOCK;
            }
            if flags.contains(PF_HW_COMPLETION) {
                flags2 |= KIND_HW_COMPLETION;
            }
            if flags.contains(PF_ZERO_COPY) {
                flags2 |= KIND_ZERO_COPY;
            }
            if fb.version < VRR_REFRESH_SINCE {
                if flags.contains(PF_VRR) {
                    refresh = 0;
                }
            }
            fb.send_presented(tv_sec, tv_nsec, refresh, seq, flags2);
            fb.client.remove_obj(&*fb);
        }
    }
}

impl Drop for PresentationFeedback {
    fn drop(&mut self) {
        if let Some(fb) = self.fb.take() {
            fb.send_discarded();
            fb.client.remove_obj(&*fb);
        }
    }
}

#[derive(Object)]
pub struct WpPresentationFeedback {
    pub id: WpPresentationFeedbackId,
    pub client: Rc<Client>,
    pub _surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

const KIND_VSYNC: u32 = 0x1;
const KIND_HW_CLOCK: u32 = 0x2;
const KIND_HW_COMPLETION: u32 = 0x4;
const KIND_ZERO_COPY: u32 = 0x8;

const VRR_REFRESH_SINCE: Version = Version(2);

impl WpPresentationFeedback {
    fn send_sync_output(&self, output: &WlOutput) {
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
