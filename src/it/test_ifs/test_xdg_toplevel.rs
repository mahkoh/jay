use crate::client::Client;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::it::test_error::TestErrorError;
use crate::it::test_error::TestResult;
use crate::it::test_utils::test_window::TestWindow;
use crate::tree::ContainerNode;
use crate::tree::ContainingNode;
use crate::tree::FloatNode;
use crate::tree::ToplevelNodeBase;
use crate::utils::bhash::BHashSet;
use crate::wire::XdgToplevelId;
use crate::wire::xdg_toplevel::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::future::poll_fn;
use std::rc::Rc;
use std::task::Poll;
use std::task::Waker;

pub struct TestXdgToplevelCore {
    pub id: XdgToplevelId,
    pub client: Rc<Client>,

    pub configured: Cell<bool>,
    pub configured_waiter: Cell<Option<Waker>>,

    pub width: Cell<i32>,
    pub height: Cell<i32>,
    pub states: RefCell<BHashSet<u32>>,

    pub close_requested: Cell<bool>,
}

pub struct TestXdgToplevel {
    pub core: Rc<TestXdgToplevelCore>,
    pub server: Rc<XdgToplevel>,
}

impl TestXdgToplevel {
    fn parent(&self) -> TestResult<Rc<dyn ContainingNode>> {
        match self.server.tl_data().parent.get() {
            Some(p) => Ok(p),
            _ => bail!("toplevel has no parent"),
        }
    }

    pub fn container_parent(&self) -> TestResult<Rc<ContainerNode>> {
        let parent = self.parent()?;
        match parent.node_into_container() {
            Some(p) => Ok(p),
            _ => bail!("toplevel parent is not a container"),
        }
    }

    pub fn float_parent(&self) -> TestResult<Rc<FloatNode>> {
        let parent = self.parent()?;
        match parent.node_into_float() {
            Some(p) => Ok(p),
            _ => bail!("toplevel parent is not a float"),
        }
    }
}

impl TestXdgToplevelCore {
    pub fn destroy(&self) {
        self.client.send_xdg_toplevel_destroy(self.id);
    }

    pub fn set_title(&self, title: &str) {
        self.client.send_xdg_toplevel_set_title(self.id, title);
    }

    pub fn set_parent(&self, parent: &TestWindow) {
        self.client
            .send_xdg_toplevel_set_parent(self.id, parent.tl.server.id);
    }

    pub async fn configured(&self) {
        poll_fn(|ctx| {
            if self.configured.get() {
                return Poll::Ready(());
            }
            self.configured_waiter.set(Some(ctx.waker().clone()));
            Poll::Pending
        })
        .await;
    }
}

synthetic_event_handler!(TestXdgToplevelCore);

impl XdgToplevelEventHandler for TestXdgToplevelCore {
    type Error = TestErrorError;

    fn configure(&self, ev: Configure<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.width.set(ev.width);
        self.height.set(ev.height);
        *self.states.borrow_mut() = ev.states.iter().copied().collect();
        self.configured.set(true);
        if let Some(waker) = self.configured_waiter.take() {
            waker.wake();
        }
        Ok(())
    }

    fn close(&self, _ev: Close, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.close_requested.set(true);
        Ok(())
    }

    fn configure_bounds(&self, _ev: ConfigureBounds, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn wm_capabilities(&self, _ev: WmCapabilities<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}
