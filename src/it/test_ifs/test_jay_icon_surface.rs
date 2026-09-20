use crate::client::Client;
use crate::it::test_error::TestErrorError;
use crate::wire::JayIconSurfaceFactoryV1Id;
use crate::wire::JayIconSurfaceV1Id;
use crate::wire::WlSurfaceId;
use crate::wire::XdgToplevelId;
use crate::wire::jay_icon_surface_factory_v1::JayIconSurfaceFactoryV1EventHandler;
use crate::wire::jay_icon_surface_factory_v1::Stopped;
use crate::wire::jay_icon_surface_factory_v1::Surface as IconSurfaceEvent;
use crate::wire::jay_icon_surface_v1::*;
use crate::wire::jay_wl_surface_factory_v1::JayWlSurfaceFactoryV1EventHandler;
use crate::wire::jay_wl_surface_factory_v1::Surface as WlSurfaceEvent;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TestIconSurfaceFactory {
    pub client: Rc<Client>,
    pub factory: Cell<JayIconSurfaceFactoryV1Id>,
    pub pending_wl_surface: Rc<Cell<WlSurfaceId>>,
    pub surfaces: RefCell<Vec<Rc<TestIconSurface>>>,
    pub stopped: Cell<bool>,
}

pub struct TestIconSurface {
    pub client: Rc<Client>,
    pub id: JayIconSurfaceV1Id,
    pub wl_surface: WlSurfaceId,
    pub configure_size: Cell<Option<(i32, i32)>>,
    pub configure: Cell<Option<u64>>,
    pub finished: Cell<bool>,
}

pub trait TestIconSurfaceFactoryExt {
    fn create_icon_surface_factory(
        self: &Rc<Self>,
        toplevel: XdgToplevelId,
    ) -> Rc<TestIconSurfaceFactory>;
}

impl TestIconSurfaceFactoryExt for Client {
    fn create_icon_surface_factory(
        self: &Rc<Self>,
        toplevel: XdgToplevelId,
    ) -> Rc<TestIconSurfaceFactory> {
        let wl_surface_factory = self.send_jay_wl_surface_factory_manager_v1_create_factory();
        let subject = self.send_jay_toplevel_icon_subject_manager_v1_create_subject(toplevel);
        let pending_wl_surface = Rc::new(Cell::new(WlSurfaceId::NONE));
        let wl_surface_handler = Rc::new(TestWlSurfaceFactory {
            pending: pending_wl_surface.clone(),
        });
        self.set_synthetic_event_handler(wl_surface_factory, &wl_surface_handler);
        let factory = Rc::new(TestIconSurfaceFactory {
            client: self.clone(),
            factory: Cell::new(JayIconSurfaceFactoryV1Id::NONE),
            pending_wl_surface,
            surfaces: Default::default(),
            stopped: Default::default(),
        });
        let id = self.send_jay_icon_surface_manager_v1_create_factory(subject, wl_surface_factory);
        factory.factory.set(id);
        self.set_synthetic_event_handler(id, &factory);
        factory
    }
}

struct TestWlSurfaceFactory {
    pending: Rc<Cell<WlSurfaceId>>,
}

synthetic_event_handler!(TestWlSurfaceFactory);

impl JayWlSurfaceFactoryV1EventHandler for TestWlSurfaceFactory {
    type Error = TestErrorError;

    fn surface(&self, ev: WlSurfaceEvent, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pending.set(ev.id);
        Ok(())
    }
}

synthetic_event_handler!(TestIconSurfaceFactory);

impl JayIconSurfaceFactoryV1EventHandler for TestIconSurfaceFactory {
    type Error = TestErrorError;

    fn stopped(&self, _ev: Stopped, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.stopped.set(true);
        Ok(())
    }

    fn surface(&self, ev: IconSurfaceEvent, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = Rc::new(TestIconSurface {
            client: self.client.clone(),
            id: ev.id,
            wl_surface: self.pending_wl_surface.get(),
            configure_size: Default::default(),
            configure: Default::default(),
            finished: Default::default(),
        });
        self.client.set_synthetic_event_handler(ev.id, &surface);
        self.surfaces.borrow_mut().push(surface);
        Ok(())
    }
}

synthetic_event_handler!(TestIconSurface);

impl JayIconSurfaceV1EventHandler for TestIconSurface {
    type Error = TestErrorError;

    fn finished(&self, _ev: Finished, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.finished.set(true);
        Ok(())
    }

    fn configure(&self, ev: Configure, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client
            .send_jay_icon_surface_v1_ack_configure(self.id, ev.serial);
        self.configure.set(Some(ev.serial));
        Ok(())
    }

    fn configure_size(&self, ev: ConfigureSize, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.configure_size.set(Some((ev.width, ev.height)));
        Ok(())
    }
}
