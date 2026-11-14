use crate::format::formats;
use crate::it::test_error::TestErrorError;
use crate::state::State;
use crate::video::LINEAR_MODIFIER;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmaBufPlane;
use crate::video::dmabuf::PlaneVec;
use crate::wire::jay_screenshot::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestJayScreenshot {
    pub state: Rc<State>,
    pub result: Cell<Option<Result<Rc<DmaBuf>, String>>>,
}

synthetic_event_handler!(TestJayScreenshot);

impl JayScreenshotEventHandler for TestJayScreenshot {
    type Error = TestErrorError;

    fn unused1(&self, _ev: Unused1, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        unimplemented!()
    }

    fn error(&self, ev: Error<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.result.set(Some(Err(ev.msg.to_string())));
        Ok(())
    }

    fn unused2(&self, _ev: Unused2, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        unimplemented!()
    }

    fn unused3(&self, _ev: Unused3, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        unimplemented!()
    }

    fn unused4(&self, _ev: Unused4, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        unimplemented!()
    }

    fn unused5(&self, _ev: Unused5, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        unimplemented!()
    }

    fn udmabuf(&self, ev: Udmabuf, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(format) = formats().get(&ev.format).copied() else {
            bail!(
                "Compositor sent screenshot with unknown format {}",
                ev.format
            );
        };
        let mut planes = PlaneVec::new();
        planes.push(DmaBufPlane {
            offset: 0,
            stride: ev.stride,
            fd: ev.fd,
        });
        self.result.set(Some(Ok(DmaBuf::new(
            &self.state.dma_buf_ids,
            ev.width,
            ev.height,
            format,
            LINEAR_MODIFIER,
            planes,
        ))));
        Ok(())
    }
}
