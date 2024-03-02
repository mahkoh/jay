use {
    crate::{
        client::ClientId,
        it::{
            test_error::TestError, test_ifs::test_screenshot::TestJayScreenshot,
            test_object::TestObject, test_transport::TestTransport, testrun::ParseFull,
        },
        utils::{buffd::MsgParser, cell_ext::CellExt},
        wire::{
            jay_compositor::{self, *},
            jay_screenshot::Dmabuf,
            JayCompositorId,
        },
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestJayCompositor {
    pub id: JayCompositorId,
    pub tran: Rc<TestTransport>,
    pub client_id: Cell<Option<ClientId>>,
}

impl TestJayCompositor {
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

    pub async fn take_screenshot(&self) -> Result<Dmabuf, TestError> {
        let js = Rc::new(TestJayScreenshot {
            id: self.tran.id(),
            result: Cell::new(None),
        });
        self.tran.send(TakeScreenshot {
            self_id: self.id,
            id: js.id,
        })?;
        self.tran.add_obj(js.clone())?;
        self.tran.sync().await;
        match js.result.take() {
            Some(Ok(res)) => Ok(res),
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
}

test_object! {
    TestJayCompositor, JayCompositor;

    CLIENT_ID => handle_client_id,
}

impl TestObject for TestJayCompositor {}
