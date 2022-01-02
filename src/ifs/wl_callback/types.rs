use crate::client::EventFormatter;
use crate::ifs::wl_callback::{WlCallback, DONE};
use crate::object::Object;
use crate::utils::buffd::WlFormatter;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;

pub(super) struct Done {
    pub obj: Rc<WlCallback>,
}
impl EventFormatter for Done {
    fn format(self: Box<Self>, fmt: &mut WlFormatter<'_>) {
        fmt.header(self.obj.id, DONE).uint(0);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Done {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "done(callback_data: 0)")
    }
}
