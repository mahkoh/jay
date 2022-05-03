use {
    crate::{
        ifs::wl_surface::WlSurface,
        it::{
            test_error::TestError, test_object::TestObject, test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{wl_surface::*, WlBufferId, WlSurfaceId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestSurface {
    pub id: WlSurfaceId,
    pub tran: Rc<TestTransport>,
    pub server: Rc<WlSurface>,
    pub destroyed: Cell<bool>,
}

impl TestSurface {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn attach(&self, buffer_id: WlBufferId) -> Result<(), TestError> {
        self.tran.send(Attach {
            self_id: self.id,
            buffer: buffer_id,
            x: 0,
            y: 0,
        })?;
        Ok(())
    }

    pub fn commit(&self) -> Result<(), TestError> {
        self.tran.send(Commit { self_id: self.id })?;
        Ok(())
    }

    fn handle_enter(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Enter::parse_full(parser)?;
        Ok(())
    }

    fn handle_leave(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Leave::parse_full(parser)?;
        Ok(())
    }
}

impl Drop for TestSurface {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestSurface, WlSurface;

    ENTER => handle_enter,
    LEAVE => handle_leave,
}

impl TestObject for TestSurface {}
