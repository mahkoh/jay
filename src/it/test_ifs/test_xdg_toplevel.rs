use {
    crate::{
        ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel,
        it::{
            test_error::{TestError, TestResult},
            test_object::TestObject,
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        tree::{ContainerNode, ToplevelNodeBase},
        utils::buffd::MsgParser,
        wire::{xdg_toplevel::*, XdgToplevelId},
    },
    ahash::AHashSet,
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

pub struct TestXdgToplevel {
    pub id: XdgToplevelId,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub server: Rc<XdgToplevel>,

    pub width: Cell<i32>,
    pub height: Cell<i32>,
    pub states: RefCell<AHashSet<u32>>,

    pub close_requested: Cell<bool>,
}

impl TestXdgToplevel {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    pub fn container_parent(&self) -> TestResult<Rc<ContainerNode>> {
        let parent = match self.server.tl_data().parent.get() {
            Some(p) => p,
            _ => bail!("toplevel has no parent"),
        };
        match parent.node_into_container() {
            Some(p) => Ok(p),
            _ => bail!("toplevel parent is not a container"),
        }
    }

    fn handle_configure(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Configure::parse_full(parser)?;
        self.width.set(ev.width);
        self.height.set(ev.height);
        *self.states.borrow_mut() = ev.states.iter().copied().collect();
        Ok(())
    }

    fn handle_close(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Close::parse_full(parser)?;
        self.close_requested.set(true);
        Ok(())
    }

    fn handle_configure_bounds(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = ConfigureBounds::parse_full(parser)?;
        Ok(())
    }
}

impl Drop for TestXdgToplevel {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestXdgToplevel, XdgToplevel;

    CONFIGURE => handle_configure,
    CLOSE => handle_close,
    CONFIGURE_BOUNDS => handle_configure_bounds,
}

impl TestObject for TestXdgToplevel {}
