use {
    crate::{
        it::{
            test_error::TestError, test_ifs::test_xdg_toplevel::TestXdgToplevel,
            test_object::TestObject, test_transport::TestTransport,
        },
        wire::{xdg_toplevel_drag_v1::*, XdgToplevelDragV1Id},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestToplevelDrag {
    pub id: XdgToplevelDragV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
}

impl TestToplevelDrag {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn attach(
        &self,
        toplevel: &TestXdgToplevel,
        x_offset: i32,
        y_offset: i32,
    ) -> Result<(), TestError> {
        self.tran.send(Attach {
            self_id: self.id,
            toplevel: toplevel.core.id,
            x_offset,
            y_offset,
        })?;
        Ok(())
    }
}

impl Drop for TestToplevelDrag {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestToplevelDrag, XdgToplevelDragV1;
}

impl TestObject for TestToplevelDrag {}
