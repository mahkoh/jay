use crate::object::Version;
use crate::wire::ZwlrScreencopyManagerV1Id;
use crate::wire::zwlr_screencopy_manager_v1::*;
use crate::wl_usr::UsrCon;
use crate::wl_usr::usr_ifs::usr_wl_output::UsrWlOutput;
use crate::wl_usr::usr_ifs::usr_zwlr_screencopy_frame::UsrZwlrScreencopyFrame;
use crate::wl_usr::usr_object::UsrObject;
use std::convert::Infallible;
use std::rc::Rc;

pub struct UsrZwlrScreencopyManager {
    pub id: ZwlrScreencopyManagerV1Id,
    pub con: Rc<UsrCon>,
    pub version: Version,
}

impl UsrZwlrScreencopyManager {
    #[expect(dead_code)]
    pub fn capture_output(&self, output: &UsrWlOutput) -> Rc<UsrZwlrScreencopyFrame> {
        let frame = Rc::new(UsrZwlrScreencopyFrame {
            id: self.con.id(),
            con: self.con.clone(),
            owner: Default::default(),
            version: self.version,
        });
        self.con.request(CaptureOutput {
            self_id: self.id,
            frame: frame.id,
            overlay_cursor: 0,
            output: output.id,
        });
        self.con.add_object(frame.clone());
        frame
    }
}

impl ZwlrScreencopyManagerV1EventHandler for UsrZwlrScreencopyManager {
    type Error = Infallible;
}

usr_object_base! {
    self = UsrZwlrScreencopyManager = ZwlrScreencopyManagerV1;
    version = self.version;
}

impl UsrObject for UsrZwlrScreencopyManager {
    fn destroy(&self) {
        self.con.request(Destroy { self_id: self.id });
    }
}
