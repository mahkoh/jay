use {
    crate::{
        wire::{zwlr_screencopy_manager_v1::*, ZwlrScreencopyManagerV1Id},
        wl_usr::{
            usr_ifs::{
                usr_wl_output::UsrWlOutput, usr_zwlr_screencopy_frame::UsrZwlrScreencopyFrame,
            },
            usr_object::UsrObject,
            UsrCon,
        },
    },
    std::rc::Rc,
};
pub struct UsrZwlrScreencopyManager {
    pub id: ZwlrScreencopyManagerV1Id,
    pub con: Rc<UsrCon>,
}

impl UsrZwlrScreencopyManager {
    pub fn request_capture_output(&self, frame: &UsrZwlrScreencopyFrame, output: &UsrWlOutput) {
        self.con.request(CaptureOutput {
            self_id: self.id,
            frame: frame.id,
            overlay_cursor: 0,
            output: output.id,
        });
    }
}

impl Drop for UsrZwlrScreencopyManager {
    fn drop(&mut self) {
        self.con.request(Destroy { self_id: self.id });
    }
}

usr_object_base! {
    UsrZwlrScreencopyManager, ZwlrScreencopyManagerV1;
}

impl UsrObject for UsrZwlrScreencopyManager {}
