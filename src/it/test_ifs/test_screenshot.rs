use crate::format::XRGB8888;
use crate::format::formats;
use crate::it::test_error::TestErrorError;
use crate::state::State;
use crate::video::dmabuf::DmaBuf;
use crate::video::dmabuf::DmaBufPlane;
use crate::video::dmabuf::PlaneVec;
use crate::wire::jay_screenshot::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use uapi::OwnedFd;

pub struct TestJayScreenshot {
    pub state: Rc<State>,
    pub drm_dev: Cell<Option<Rc<OwnedFd>>>,
    pub planes: RefCell<PlaneVec<DmaBufPlane>>,
    pub result: Cell<Option<Result<Rc<DmaBuf>, String>>>,
}

synthetic_event_handler!(TestJayScreenshot);

impl JayScreenshotEventHandler for TestJayScreenshot {
    type Error = TestErrorError;

    fn dmabuf(&self, ev: Dmabuf, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let mut planes = PlaneVec::new();
        planes.push(DmaBufPlane {
            offset: ev.offset,
            stride: ev.stride,
            fd: ev.fd,
        });
        self.result.set(Some(Ok(DmaBuf::new(
            &self.state.dma_buf_ids,
            ev.width as _,
            ev.height as _,
            XRGB8888,
            ev.modifier,
            planes,
        ))));
        Ok(())
    }

    fn error(&self, ev: Error<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.result.set(Some(Err(ev.msg.to_string())));
        Ok(())
    }

    fn drm_dev(&self, ev: DrmDev, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.drm_dev.set(Some(ev.drm_dev));
        Ok(())
    }

    fn plane(&self, ev: Plane, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.planes.borrow_mut().push(DmaBufPlane {
            offset: ev.offset,
            stride: ev.stride,
            fd: ev.fd,
        });
        Ok(())
    }

    fn dmabuf2(&self, ev: Dmabuf2, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.result.set(Some(Ok(DmaBuf::new(
            &self.state.dma_buf_ids,
            ev.width as _,
            ev.height as _,
            XRGB8888,
            ev.modifier,
            self.planes.take(),
        ))));
        Ok(())
    }

    fn dmabuf3(&self, ev: Dmabuf3, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(format) = formats().get(&ev.format).copied() else {
            bail!(
                "Compositor sent screenshot with unknown format {}",
                ev.format
            );
        };
        self.result.set(Some(Ok(DmaBuf::new(
            &self.state.dma_buf_ids,
            ev.width as _,
            ev.height as _,
            format,
            ev.modifier,
            self.planes.take(),
        ))));
        Ok(())
    }
}
