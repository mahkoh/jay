use crate::cli::ScreenshotFormat;
use crate::cli::screenshot::buf_to_bytes;
use crate::client::Client;
use crate::cmm::cmm_eotf::Eotf;
use crate::globals::GlobalBase;
use crate::globals::GlobalName;
use crate::globals::Singleton;
use crate::it::test_error::TestError;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_buffer::TestBuffer;
use crate::it::test_ifs::test_callback::TestCallback;
use crate::it::test_ifs::test_keyboard::TestKeyboard;
use crate::it::test_ifs::test_pointer::TestPointer;
use crate::it::test_ifs::test_registry::TestRegistry;
use crate::it::test_ifs::test_screenshot::TestJayScreenshot;
use crate::it::test_ifs::test_seat::TestSeat;
use crate::it::test_ifs::test_session::TestSession;
use crate::it::test_ifs::test_surface::TestSurface;
use crate::it::test_ifs::test_toplevel_session::TestToplevelSession;
use crate::it::test_ifs::test_viewport::TestViewport;
use crate::it::test_ifs::test_xdg_surface::TestXdgSurface;
use crate::it::test_utils::test_surface_ext::TestSurfaceExt;
use crate::it::test_utils::test_window::TestWindow;
use crate::it::testrun::TestRun;
use crate::object::Interface;
use crate::theme::Color;
use crate::video::dmabuf::DmaBuf;
use crate::wire::ObjectId;
use crate::wire::WlSeat;
use crate::wire::WlSurfaceId;
use std::cell::Cell;
use std::ops::Deref;
use std::rc::Rc;
use std::task::Poll;
use uapi::OwnedFd;

pub struct TestClient {
    pub run: Rc<TestRun>,
    pub _socket: OwnedFd,
    pub client: Rc<Client>,
}

pub struct DefaultSeat {
    pub seat: Rc<TestSeat>,
    pub kb: Rc<TestKeyboard>,
    pub pointer: Rc<TestPointer>,
}

impl Deref for TestClient {
    type Target = Rc<Client>;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl TestClient {
    #[expect(unused)]
    async fn save_screenshot(&self, name: &str, include_cursor: bool) -> Result<(), TestError> {
        let qoi = self.client.screenshot_qoi(include_cursor).await?;
        let path = format!("{}/screenshot_{}.qoi", self.run.out_dir, name);
        std::fs::write(path, qoi)?;
        Ok(())
    }

    pub async fn compare_screenshot(
        &self,
        name: &str,
        include_cursor: bool,
    ) -> Result<(), TestError> {
        let actual = self.client.screenshot_qoi(include_cursor).await?;
        let expected_path = format!("{}/screenshot_{}.qoi", self.run.in_dir, name);
        let expected = std::fs::read(expected_path)?;
        if actual != expected {
            let actual_out_path = format!("{}/screenshot_{}_actual.qoi", self.run.out_dir, name);
            let expected_out_path =
                format!("{}/screenshot_{}_expected.qoi", self.run.out_dir, name);
            let _ = std::fs::write(actual_out_path, actual);
            let _ = std::fs::write(expected_out_path, expected);
            bail!("Screenshots differ");
        }
        Ok(())
    }
}

pub trait TestClientExt {
    async fn sync(self: &Rc<Self>);
    fn bind<T: From<ObjectId>>(self: &Rc<Self>, singleton: Singleton) -> T;
    fn bind_global<T: From<ObjectId>>(
        self: &Rc<Self>,
        name: GlobalName,
        interface: Interface,
        version: u32,
    ) -> T;
    fn create_single_pixel_buffer(self: &Rc<Self>, color: Color) -> Rc<TestBuffer>;
    async fn get_default_seat(self: &Rc<Self>) -> TestResult<DefaultSeat>;
    async fn screenshot_qoi(self: &Rc<Self>, include_cursor: bool) -> Result<Vec<u8>, TestError>;
    async fn create_surface(self: &Rc<Self>) -> TestResult<Rc<TestSurface>>;
    fn get_viewport(self: &Rc<Self>, surface: &TestSurface) -> Rc<TestViewport>;
    async fn create_surface_ext(self: &Rc<Self>) -> Result<TestSurfaceExt, TestError>;
    fn create_xdg_surface(self: &Rc<Self>, surface: WlSurfaceId) -> Rc<TestXdgSurface>;
    async fn create_window_no_commit(self: &Rc<Self>) -> Result<Rc<TestWindow>, TestError>;
    async fn create_window(self: &Rc<Self>) -> Result<Rc<TestWindow>, TestError>;
    async fn restore_window(
        self: &Rc<Self>,
        session: &TestSession,
        name: &str,
    ) -> Result<(Rc<TestWindow>, Rc<TestToplevelSession>), TestError>;
    async fn take_screenshot(
        self: &Rc<Self>,
        include_cursor: bool,
    ) -> Result<(Rc<DmaBuf>, Option<Rc<OwnedFd>>), TestError>;
    fn new_registry(self: &Rc<Self>) -> Rc<TestRegistry>;
}

impl TestClientExt for Client {
    async fn sync(self: &Rc<Self>) {
        let id = self.send_wl_display_sync();
        let cb = Rc::new(TestCallback::default());
        self.set_synthetic_event_handler(id, &cb);
        self.state.eng.yield_now().await;
        futures_util::future::poll_fn(move |ctx| {
            if cb.done.get() {
                Poll::Ready(())
            } else {
                cb.handler.set(Some({
                    let waker = ctx.waker().clone();
                    Box::new(move || waker.wake())
                }));
                Poll::Pending
            }
        })
        .await;
        self.state.idle().await;
    }

    fn bind<T: From<ObjectId>>(self: &Rc<Self>, singleton: Singleton) -> T {
        let info = self.state.globals.singletons[singleton];
        self.bind_global(info.name, singleton.interface(), info.version)
    }

    fn bind_global<T: From<ObjectId>>(
        self: &Rc<Self>,
        name: GlobalName,
        interface: Interface,
        version: u32,
    ) -> T {
        let registry = self.get_synthetic_registry();
        self.send_wl_registry_bind(registry, name.raw(), interface.name(), version)
            .into()
    }

    fn create_single_pixel_buffer(self: &Rc<Self>, color: Color) -> Rc<TestBuffer> {
        let map = |c: f32| (c as f64 * u32::MAX as f64) as u32;
        let [r, g, b, a] = color.to_array(Eotf::Gamma22);
        let id = self.send_wp_single_pixel_buffer_manager_v1_create_u32_rgba_buffer(
            map(r),
            map(g),
            map(b),
            map(a),
        );
        let buffer = Rc::new(TestBuffer {
            id,
            released: Cell::new(true),
        });
        self.set_synthetic_event_handler(id, &buffer);
        buffer
    }

    async fn get_default_seat(self: &Rc<Self>) -> TestResult<DefaultSeat> {
        let seat = 'get_seat: {
            for seat in self.state.globals.seats.lock().values() {
                if seat.seat_name() == "default" {
                    break 'get_seat seat.clone();
                }
            }
            bail!("Default seat not found");
        };
        let id = self.bind_global(seat.name(), WlSeat, 9);
        let tseat = Rc::new(TestSeat {
            id,
            client: self.clone(),
            caps: Cell::new(0),
            name: Default::default(),
        });
        self.set_synthetic_event_handler(id, &tseat);
        let pointer = tseat.get_pointer();
        let tkb = tseat.get_keyboard();
        self.sync().await;
        Ok(DefaultSeat {
            seat: tseat,
            kb: tkb,
            pointer,
        })
    }

    async fn screenshot_qoi(self: &Rc<Self>, include_cursor: bool) -> Result<Vec<u8>, TestError> {
        let (dmabuf, dev) = self.take_screenshot(include_cursor).await?;
        let qoi = buf_to_bytes(
            &self.state.eventfd_cache,
            dev.as_ref(),
            &dmabuf,
            ScreenshotFormat::Qoi,
            false,
        )?;
        Ok(qoi)
    }

    async fn create_surface(self: &Rc<Self>) -> TestResult<Rc<TestSurface>> {
        let id = self.send_wl_compositor_create_surface();
        self.sync().await;
        let surface = Rc::new(TestSurface {
            id,
            client: self.clone(),
            server: self.lookup(id)?,
            preferred_buffer_scale: Rc::new(Default::default()),
            preferred_buffer_transform: Rc::new(Default::default()),
        });
        self.set_synthetic_event_handler(id, &surface);
        Ok(surface)
    }

    fn get_viewport(self: &Rc<Self>, surface: &TestSurface) -> Rc<TestViewport> {
        let id = self.send_wp_viewporter_get_viewport(surface.id);
        Rc::new(TestViewport {
            id,
            client: self.clone(),
        })
    }

    async fn create_surface_ext(self: &Rc<Self>) -> Result<TestSurfaceExt, TestError> {
        let surface = self.create_surface().await?;
        let viewport = self.get_viewport(&surface);
        Ok(TestSurfaceExt {
            client: self.clone(),
            surface,
            viewport,
            color: Cell::new(Color::SOLID_BLACK),
        })
    }

    fn create_xdg_surface(self: &Rc<Self>, surface: WlSurfaceId) -> Rc<TestXdgSurface> {
        let id = self.send_xdg_wm_base_get_xdg_surface(surface);
        let xdg = Rc::new(TestXdgSurface {
            id,
            client: self.clone(),
            last_serial: Default::default(),
        });
        self.set_synthetic_event_handler(id, &xdg);
        xdg
    }

    async fn create_window_no_commit(self: &Rc<Self>) -> Result<Rc<TestWindow>, TestError> {
        let surface = self.create_surface_ext().await?;
        let xdg = self.create_xdg_surface(surface.surface.id);
        let tl = xdg.create_toplevel().await?;
        Ok(Rc::new(TestWindow { surface, xdg, tl }))
    }

    async fn create_window(self: &Rc<Self>) -> Result<Rc<TestWindow>, TestError> {
        let win = self.create_window_no_commit().await?;
        win.surface.surface.commit();
        self.sync().await;
        Ok(win)
    }

    async fn restore_window(
        self: &Rc<Self>,
        session: &TestSession,
        name: &str,
    ) -> Result<(Rc<TestWindow>, Rc<TestToplevelSession>), TestError> {
        let win = self.create_window_no_commit().await?;
        let session = session.restore_toplevel(&win, name);
        win.surface.surface.commit();
        win.tl.core.configured().await;
        Ok((win, session))
    }

    async fn take_screenshot(
        self: &Rc<Self>,
        include_cursor: bool,
    ) -> Result<(Rc<DmaBuf>, Option<Rc<OwnedFd>>), TestError> {
        self.sync().await;
        let id = self.send_jay_compositor_take_screenshot2(include_cursor);
        let js = Rc::new(TestJayScreenshot {
            state: self.state.clone(),
            drm_dev: Default::default(),
            planes: Default::default(),
            result: Default::default(),
        });
        self.set_synthetic_event_handler(id, &js);
        self.sync().await;
        match js.result.take() {
            Some(Ok(res)) => Ok((res, js.drm_dev.take())),
            Some(Err(res)) => bail!("Compositor could not take a screenshot: {}", res),
            None => bail!("Compositor did not send a screenshot"),
        }
    }

    fn new_registry(self: &Rc<Self>) -> Rc<TestRegistry> {
        let id = self.send_wl_display_get_registry();
        let slf = Rc::new(TestRegistry {
            id,
            client: self.clone(),
            globals: Default::default(),
            seats: Default::default(),
        });
        self.set_synthetic_event_handler(id, &slf);
        slf
    }
}
