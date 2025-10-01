use {
    crate::{
        format::XRGB8888,
        it::{test_error::TestError, test_object::TestObject, testrun::ParseFull},
        state::State,
        utils::buffd::MsgParser,
        video::dmabuf::{DmaBuf, DmaBufPlane, PlaneVec},
        wire::{JayScreenshotId, jay_screenshot::*},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    uapi::OwnedFd,
};

pub struct TestJayScreenshot {
    pub id: JayScreenshotId,
    pub state: Rc<State>,
    pub drm_dev: Cell<Option<Rc<OwnedFd>>>,
    pub planes: RefCell<PlaneVec<DmaBufPlane>>,
    pub result: Cell<Option<Result<DmaBuf, String>>>,
}

impl TestJayScreenshot {
    fn handle_dmabuf(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Dmabuf::parse_full(parser)?;
        let mut planes = PlaneVec::new();
        planes.push(DmaBufPlane {
            offset: ev.offset,
            stride: ev.stride,
            fd: ev.fd,
        });
        self.result.set(Some(Ok(DmaBuf {
            id: self.state.dma_buf_ids.next(),
            width: ev.width as _,
            height: ev.height as _,
            format: XRGB8888,
            modifier: ev.modifier,
            planes,
            is_disjoint: Default::default(),
        })));
        Ok(())
    }

    fn handle_error(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Error::parse_full(parser)?;
        self.result.set(Some(Err(ev.msg.to_string())));
        Ok(())
    }

    fn handle_drm_dev(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = DrmDev::parse_full(parser)?;
        self.drm_dev.set(Some(ev.drm_dev));
        Ok(())
    }

    fn handle_plane(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Plane::parse_full(parser)?;
        self.planes.borrow_mut().push(DmaBufPlane {
            offset: ev.offset,
            stride: ev.stride,
            fd: ev.fd,
        });
        Ok(())
    }

    fn handle_dmabuf2(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Dmabuf2::parse_full(parser)?;
        self.result.set(Some(Ok(DmaBuf {
            id: self.state.dma_buf_ids.next(),
            width: ev.width as _,
            height: ev.height as _,
            format: XRGB8888,
            modifier: ev.modifier,
            planes: self.planes.take(),
            is_disjoint: Default::default(),
        })));
        Ok(())
    }
}

test_object! {
    TestJayScreenshot, JayScreenshot;

    DMABUF => handle_dmabuf,
    ERROR => handle_error,
    DRM_DEV => handle_drm_dev,
    PLANE => handle_plane,
    DMABUF2 => handle_dmabuf2,
}

impl TestObject for TestJayScreenshot {}
