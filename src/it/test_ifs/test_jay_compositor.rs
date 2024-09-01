use {
    crate::{
        client::ClientId,
        it::{
            test_error::{TestError, TestResult},
            test_ifs::test_screenshot::TestJayScreenshot,
            test_object::TestObject,
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::{buffd::MsgParser, cell_ext::CellExt},
        video::dmabuf::DmaBuf,
        wire::{
            jay_compositor::{self, *},
            JayCompositorId,
        },
    },
    std::{cell::Cell, rc::Rc},
    uapi::OwnedFd,
};

pub struct TestJayCompositor {
    pub id: JayCompositorId,
    pub tran: Rc<TestTransport>,
    pub client_id: Cell<Option<ClientId>>,
}

impl TestJayCompositor {
    pub fn new(tran: &Rc<TestTransport>) -> Self {
        Self {
            id: tran.id(),
            tran: tran.clone(),
            client_id: Cell::new(None),
        }
    }

    pub async fn get_client_id(&self) -> Result<ClientId, TestError> {
        if self.client_id.is_none() {
            self.tran.send(GetClientId { self_id: self.id })?;
        }
        self.tran.sync().await;
        match self.client_id.get() {
            Some(c) => Ok(c),
            _ => bail!("Compositor did not send a client id"),
        }
    }

    pub fn enable_symmetric_delete(&self) -> TestResult {
        self.tran.send(EnableSymmetricDelete { self_id: self.id })?;
        Ok(())
    }

    pub async fn take_screenshot(
        &self,
        include_cursor: bool,
    ) -> Result<(DmaBuf, Option<Rc<OwnedFd>>), TestError> {
        let js = Rc::new(TestJayScreenshot {
            id: self.tran.id(),
            state: self.tran.run.state.clone(),
            drm_dev: Default::default(),
            planes: Default::default(),
            result: Default::default(),
        });
        self.tran.send(TakeScreenshot2 {
            self_id: self.id,
            id: js.id,
            include_cursor: include_cursor as _,
        })?;
        self.tran.add_obj(js.clone())?;
        self.tran.sync().await;
        match js.result.take() {
            Some(Ok(res)) => Ok((res, js.drm_dev.take())),
            Some(Err(res)) => bail!("Compositor could not take a screenshot: {}", res),
            None => bail!("Compositor did not send a screenshot"),
        }
    }

    fn handle_client_id(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = jay_compositor::ClientId::parse_full(parser)?;
        self.client_id.set(Some(ClientId::from_raw(ev.client_id)));
        self.tran.client_id.set(ClientId::from_raw(ev.client_id));
        Ok(())
    }

    fn handle_seat(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Seat::parse_full(parser)?;
        Ok(())
    }

    fn handle_capabilities(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Capabilities::parse_full(parser)?;
        Ok(())
    }
}

test_object! {
    TestJayCompositor, JayCompositor;

    CLIENT_ID => handle_client_id,
    SEAT => handle_seat,
    CAPABILITIES => handle_capabilities,
}

impl TestObject for TestJayCompositor {}
