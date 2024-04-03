use {
    crate::{
        ifs::wl_surface::WlSurface,
        it::{
            test_error::{TestError, TestResult},
            test_ifs::test_region::TestRegion,
            test_object::TestObject,
            test_transport::TestTransport,
            test_utils::test_expected_event::TEEH,
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
    pub preferred_buffer_scale: TEEH<i32>,
    pub preferred_buffer_transform: TEEH<u32>,
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

    pub fn offset(&self, dx: i32, dy: i32) -> Result<(), TestError> {
        self.tran.send(Offset {
            self_id: self.id,
            x: dx,
            y: dy,
        })?;
        Ok(())
    }

    pub fn set_input_region(&self, region: &TestRegion) -> TestResult {
        self.tran.send(SetInputRegion {
            self_id: self.id,
            region: region.id,
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

    fn handle_preferred_buffer_scale(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = PreferredBufferScale::parse_full(parser)?;
        self.preferred_buffer_scale.push(ev.factor);
        Ok(())
    }

    fn handle_preferred_buffer_transform(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), TestError> {
        let ev = PreferredBufferTransform::parse_full(parser)?;
        self.preferred_buffer_transform.push(ev.transform);
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
    PREFERRED_BUFFER_SCALE => handle_preferred_buffer_scale,
    PREFERRED_BUFFER_TRANSFORM => handle_preferred_buffer_transform,
}

impl TestObject for TestSurface {}
