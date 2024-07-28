use {
    crate::{
        cli::screenshot::ScreenshotWithDevice,
        format::formats,
        it::{
            test_error::TestError, test_object::TestObject, test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        video::dmabuf::{DmaBuf, DmaBufPlane, PlaneVec},
        wire::{jay_screenshot::*, JayScreenshotId},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

pub struct TestJayScreenshot {
    pub tran: Rc<TestTransport>,
    pub id: JayScreenshotId,
    pub result: Cell<Option<Result<ScreenshotWithDevice, String>>>,
    pub planes: RefCell<PlaneVec<DmaBufPlane>>,
}

impl TestJayScreenshot {
    fn destroy(&self) -> Result<(), TestError> {
        self.tran.send(Destroy { self_id: self.id })
    }

    fn handle_dmabuf(&self, _parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        bail!("got dmabuf message")
    }

    fn handle_error(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Error::parse_full(parser)?;
        self.result.set(Some(Err(ev.msg.to_string())));
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

    fn handle_format(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Format::parse_full(parser)?;
        let Some(format) = formats().get(&ev.format) else {
            bail!("Unknown screenshot format {}", ev.format);
        };
        let res = ScreenshotWithDevice {
            dev: ev.drm_dev,
            buf: DmaBuf {
                id: self.tran.run.state.dma_buf_ids.next(),
                width: ev.width,
                height: ev.height,
                format,
                modifier: ev.modifier_lo as u64 | ((ev.modifier_hi as u64) << 32),
                planes: self.planes.borrow_mut().take(),
            },
        };
        self.result.set(Some(Ok(res)));
        Ok(())
    }
}

test_object! {
    TestJayScreenshot, JayScreenshot;

    DMABUF => handle_dmabuf,
    ERROR => handle_error,
    PLANE => handle_plane,
    FORMAT => handle_format,
}

impl TestObject for TestJayScreenshot {}

impl Drop for TestJayScreenshot {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}
