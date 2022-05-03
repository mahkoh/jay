use {
    crate::{
        it::{
            test_error::TestError,
            test_ifs::test_xdg_surface::TestXdgSurface,
            test_object::{Deleted, TestObject},
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{xdg_wm_base::*, WlSurfaceId, XdgWmBaseId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestXdgWmBase {
    pub id: XdgWmBaseId,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub deleted: Deleted,
}

impl TestXdgWmBase {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.deleted.check()?;
            self.tran.send(Destroy { self_id: self.id });
        }
        Ok(())
    }

    pub async fn create_xdg_surface(
        &self,
        surface: WlSurfaceId,
    ) -> Result<Rc<TestXdgSurface>, TestError> {
        let id = self.tran.id();
        self.deleted.check()?;
        self.tran.send(GetXdgSurface {
            self_id: self.id,
            id,
            surface,
        });
        self.tran.sync().await;
        let client = self.tran.get_client()?;
        let server = client.lookup(id)?;
        let xdg = Rc::new(TestXdgSurface {
            id,
            tran: self.tran.clone(),
            server,
            destroyed: Cell::new(false),
            last_serial: Cell::new(0),
            deleted: Default::default(),
        });
        self.tran.add_obj(xdg.clone())?;
        Ok(xdg)
    }

    fn handle_ping(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Ping::parse_full(parser)?;
        Ok(())
    }
}

test_object! {
    TestXdgWmBase, XdgWmBase;

    PING => handle_ping,
}

impl TestObject for TestXdgWmBase {}

impl Drop for TestXdgWmBase {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}
