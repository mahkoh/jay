use {
    crate::{
        allocator::{Allocator, AllocatorError},
        async_engine::SpawnedFuture,
        backend::{
            AxisSource, Backend, BackendEvent, Connector, ConnectorEvent, ConnectorId,
            ConnectorKernelId, DrmDeviceId, InputDevice, InputDeviceAccelProfile,
            InputDeviceCapability, InputDeviceId, InputEvent, KeyState, Mode, MonitorInfo,
            ScrollAxis, TransformMatrix,
        },
        compositor::TestFuture,
        drm_feedback::DrmFeedback,
        fixed::Fixed,
        gfx_api::GfxError,
        gfx_apis::create_vulkan_allocator,
        ifs::wl_output::OutputId,
        it::{
            test_error::TestResult, test_gfx_api::TestGfxCtx, test_utils::test_expected_event::TEEH,
        },
        state::State,
        udmabuf::Udmabuf,
        utils::{
            clonecell::CloneCell, copyhashmap::CopyHashMap, errorfmt::ErrorFmt,
            on_change::OnChange, oserror::OsError, syncqueue::SyncQueue,
        },
        video::{
            drm::{ConnectorType, Drm},
            gbm::{GbmDevice, GbmError},
        },
    },
    bstr::ByteSlice,
    std::{any::Any, cell::Cell, error::Error, io, os::unix::ffi::OsStrExt, pin::Pin, rc::Rc},
    thiserror::Error,
    uapi::c,
};

#[derive(Debug, Error)]
pub enum TestBackendError {
    #[error("Could not read /dev/dri")]
    ReadDri(#[source] io::Error),
    #[error("There are no drm nodes in /dev/dri")]
    NoDrmNode,
    #[error("Could not open drm node {0}")]
    OpenDrmNode(String, #[source] OsError),
    #[error("Could not create a render context")]
    RenderContext(#[source] GfxError),
    #[error("Could not create a gbm device")]
    CreateGbmDevice(#[source] GbmError),
    #[error("Could not create any allocator")]
    CreateAllocator,
    #[error("Could not create a vulkan allocator")]
    CreateVulkanAllocator(#[source] AllocatorError),
}

pub struct TestBackend {
    pub state: Rc<State>,
    pub test_future: TestFuture,
    pub default_monitor_info: MonitorInfo,
    pub default_connector: Rc<TestConnector>,
    pub default_mouse: Rc<TestBackendMouse>,
    pub default_kb: Rc<TestBackendKb>,
    pub render_context_installed: Cell<bool>,
    pub idle: TEEH<bool>,
}

impl TestBackend {
    pub fn new(state: &Rc<State>, future: TestFuture) -> Self {
        state.set_backend_idle(false);
        let default_connector = Rc::new(TestConnector {
            id: state.connector_ids.next(),
            kernel_id: ConnectorKernelId {
                ty: ConnectorType::VGA,
                idx: 1,
            },
            events: Default::default(),
            feedback: Default::default(),
        });
        let default_mouse = Rc::new(TestBackendMouse {
            common: TestInputDeviceCommon {
                id: state.input_device_ids.next(),
                removed: Cell::new(false),
                events: Default::default(),
                on_change: Default::default(),
                capabilities: {
                    let chm = CopyHashMap::new();
                    chm.set(InputDeviceCapability::Pointer, ());
                    chm
                },
                name: Rc::new("default-mouse".to_string()),
                state: state.clone(),
            },
            transform_matrix: Cell::new([[1.0, 0.0], [0.0, 1.0]]),
            accel_speed: Cell::new(1.0),
            accel_profile: Cell::new(InputDeviceAccelProfile::Flat),
            left_handed: Cell::new(false),
        });
        let default_kb = Rc::new(TestBackendKb {
            common: TestInputDeviceCommon {
                id: state.input_device_ids.next(),
                removed: Cell::new(false),
                events: Default::default(),
                on_change: Default::default(),
                capabilities: {
                    let chm = CopyHashMap::new();
                    chm.set(InputDeviceCapability::Keyboard, ());
                    chm
                },
                name: Rc::new("default-keyboard".to_string()),
                state: state.clone(),
            },
        });
        let mode = Mode {
            width: 800,
            height: 600,
            refresh_rate_millihz: 60_000,
        };
        let default_monitor_info = MonitorInfo {
            modes: vec![mode],
            output_id: Rc::new(OutputId {
                connector: None,
                manufacturer: "jay".to_string(),
                model: "TestConnector".to_string(),
                serial_number: default_connector.id.to_string(),
            }),
            initial_mode: mode,
            width_mm: 80,
            height_mm: 60,
            non_desktop: false,
            vrr_capable: false,
        };
        Self {
            state: state.clone(),
            test_future: future,
            default_monitor_info,
            default_connector,
            default_mouse,
            default_kb,
            render_context_installed: Cell::new(false),
            idle: Rc::new(Default::default()),
        }
    }

    pub fn install_render_context(&self, need_drm: bool) -> TestResult {
        if self.render_context_installed.get() {
            return Ok(());
        }
        self.create_render_context(need_drm)?;
        self.render_context_installed.set(true);
        Ok(())
    }

    pub fn install_default(&self) -> TestResult {
        self.install_default2(false)
    }

    pub fn install_default2(&self, need_drm: bool) -> TestResult {
        self.install_render_context(need_drm)?;
        self.state
            .backend_events
            .push(BackendEvent::NewConnector(self.default_connector.clone()));
        self.default_connector
            .events
            .send_event(ConnectorEvent::Connected(self.default_monitor_info.clone()));
        self.state
            .backend_events
            .push(BackendEvent::NewInputDevice(self.default_kb.clone()));
        self.state
            .backend_events
            .push(BackendEvent::NewInputDevice(self.default_mouse.clone()));
        Ok(())
    }

    fn create_render_context(&self, need_drm: bool) -> Result<(), TestBackendError> {
        macro_rules! constructor {
            ($c:expr) => {
                constructor!($c, |a| Rc::new(a) as Rc<dyn Allocator>)
            };
            ($c:expr, nomap) => {
                constructor!($c, |a| a)
            };
            ($c:expr, $map:expr) => {
                (&|| $c.map($map).map_err(|e| Box::new(e) as Box<dyn Error>))
                    as &dyn Fn() -> Result<Rc<dyn Allocator>, Box<dyn Error>>
            };
        }
        let udmabuf = ("udmabuf", constructor!(Udmabuf::new()));
        let vulkan = ("Vulkan", constructor!(create_vk_allocator(), nomap));
        let gbm = ("GBM", constructor!(create_gbm_allocator()));
        let allocators = match need_drm {
            true => [vulkan, gbm, udmabuf],
            false => [udmabuf, vulkan, gbm],
        };
        let mut allocator = None::<Rc<dyn Allocator>>;
        for (name, f) in allocators {
            match f() {
                Ok(a) => {
                    allocator = Some(a);
                    break;
                }
                Err(e) => {
                    log::error!("Could not create {name} allocator: {}", ErrorFmt(&*e));
                }
            }
        }
        let allocator = allocator.ok_or(TestBackendError::CreateAllocator)?;
        let ctx = match TestGfxCtx::new(allocator) {
            Ok(ctx) => ctx,
            Err(e) => return Err(TestBackendError::RenderContext(e)),
        };
        self.state.set_render_ctx(Some(ctx));
        Ok(())
    }
}

fn create_gbm_allocator() -> Result<GbmDevice, TestBackendError> {
    create_drm_allocator(|drm| GbmDevice::new(&drm).map_err(TestBackendError::CreateGbmDevice))
}

fn create_vk_allocator() -> Result<Rc<dyn Allocator>, TestBackendError> {
    create_drm_allocator(|drm| {
        create_vulkan_allocator(&drm).map_err(TestBackendError::CreateVulkanAllocator)
    })
}

fn create_drm_allocator<T, F>(f: F) -> Result<T, TestBackendError>
where
    F: FnOnce(Drm) -> Result<T, TestBackendError>,
{
    let dri = match std::fs::read_dir("/dev/dri") {
        Ok(d) => d,
        Err(e) => return Err(TestBackendError::ReadDri(e)),
    };
    let mut files = vec![];
    for f in dri {
        let f = match f {
            Ok(f) => f,
            Err(e) => return Err(TestBackendError::ReadDri(e)),
        };
        files.push(f.path());
    }
    let node = 'node: {
        for f in &files {
            if let Some(file) = f.file_name() {
                if file.as_bytes().starts_with_str("renderD") {
                    break 'node f;
                }
            }
        }
        for f in &files {
            if let Some(file) = f.file_name() {
                if file.as_bytes().starts_with_str("card") {
                    break 'node f;
                }
            }
        }
        return Err(TestBackendError::NoDrmNode);
    };
    let file = match uapi::open(node.as_path(), c::O_RDWR | c::O_CLOEXEC, 0) {
        Ok(f) => Rc::new(f),
        Err(e) => {
            return Err(TestBackendError::OpenDrmNode(
                node.as_os_str().as_bytes().as_bstr().to_string(),
                e.into(),
            ))
        }
    };
    let drm = Drm::open_existing(file);
    f(drm)
}

impl Backend for TestBackend {
    fn run(self: Rc<Self>) -> SpawnedFuture<Result<(), Box<dyn std::error::Error>>> {
        let future = (self.test_future)(&self.state);
        self.state.eng.spawn(async move {
            let future: Pin<_> = future.into();
            future.await;
            Ok(())
        })
    }

    fn into_any(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn switch_to(&self, vtnr: u32) {
        let _ = vtnr;
    }

    fn set_idle(&self, idle: bool) {
        self.idle.push(idle);
    }

    fn supports_presentation_feedback(&self) -> bool {
        true
    }
}

pub struct TestConnector {
    pub id: ConnectorId,
    pub kernel_id: ConnectorKernelId,
    pub events: OnChange<ConnectorEvent>,
    pub feedback: CloneCell<Option<Rc<DrmFeedback>>>,
}

impl Connector for TestConnector {
    fn id(&self) -> ConnectorId {
        self.id
    }

    fn kernel_id(&self) -> ConnectorKernelId {
        self.kernel_id
    }

    fn event(&self) -> Option<ConnectorEvent> {
        self.events.events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.events.on_change.set(Some(cb));
    }

    fn damage(&self) {
        // nothing
    }

    fn drm_dev(&self) -> Option<DrmDeviceId> {
        None
    }

    fn set_mode(&self, _mode: Mode) {
        // todo
    }

    fn drm_feedback(&self) -> Option<Rc<DrmFeedback>> {
        self.feedback.get()
    }
}

pub struct TestMouseClick {
    pub mouse: Rc<TestBackendMouse>,
    pub button: u32,
}

impl Drop for TestMouseClick {
    fn drop(&mut self) {
        self.mouse.common.event(InputEvent::Button {
            time_usec: self.mouse.common.state.now_usec(),
            button: self.button,
            state: KeyState::Released,
        });
    }
}

pub struct TestBackendMouse {
    pub common: TestInputDeviceCommon,
    pub transform_matrix: Cell<TransformMatrix>,
    pub accel_speed: Cell<f64>,
    pub accel_profile: Cell<InputDeviceAccelProfile>,
    pub left_handed: Cell<bool>,
}

impl TestBackendMouse {
    pub fn rel(&self, dx: f64, dy: f64) {
        self.common.event(InputEvent::Motion {
            time_usec: self.common.state.now_usec(),
            dx: Fixed::from_f64(dx * self.accel_speed.get()),
            dy: Fixed::from_f64(dy * self.accel_speed.get()),
            dx_unaccelerated: Fixed::from_f64(dx),
            dy_unaccelerated: Fixed::from_f64(dy),
        })
    }

    pub fn abs(&self, connector: &TestConnector, x: f64, y: f64) {
        self.common.event(InputEvent::ConnectorPosition {
            time_usec: self.common.state.now_usec(),
            connector: connector.id,
            x: Fixed::from_f64(x),
            y: Fixed::from_f64(y),
        })
    }

    pub fn click(self: &Rc<Self>, button: u32) -> TestMouseClick {
        self.common.event(InputEvent::Button {
            time_usec: self.common.state.now_usec(),
            button,
            state: KeyState::Pressed,
        });
        TestMouseClick {
            mouse: self.clone(),
            button,
        }
    }

    pub fn scroll(&self, dy: i32) {
        self.common.event(InputEvent::AxisSource {
            source: AxisSource::Wheel,
        });
        self.common.event(InputEvent::Axis120 {
            dist: dy * 120,
            axis: ScrollAxis::Vertical,
            inverted: false,
        });
        self.common.event(InputEvent::AxisFrame {
            time_usec: self.common.state.now_usec(),
        });
    }

    pub fn scroll_px(&self, dy: i32) {
        self.scroll_px2(dy, false);
    }

    pub fn scroll_px2(&self, dy: i32, inverted: bool) {
        self.common.event(InputEvent::AxisSource {
            source: AxisSource::Finger,
        });
        self.common.event(InputEvent::AxisPx {
            dist: Fixed::from_int(dy),
            axis: ScrollAxis::Vertical,
            inverted,
        });
        self.common.event(InputEvent::AxisFrame {
            time_usec: self.common.state.now_usec(),
        });
    }
}

pub struct TestBackendKb {
    pub common: TestInputDeviceCommon,
}

pub struct PressedKey {
    pub kb: Rc<TestBackendKb>,
    pub key: u32,
}

impl Drop for PressedKey {
    fn drop(&mut self) {
        self.kb.common.event(InputEvent::Key {
            time_usec: self.kb.common.state.now_usec(),
            key: self.key,
            state: KeyState::Released,
        });
    }
}

impl TestBackendKb {
    pub fn press(self: &Rc<Self>, key: u32) -> PressedKey {
        self.common.event(InputEvent::Key {
            time_usec: self.common.state.now_usec(),
            key,
            state: KeyState::Pressed,
        });
        PressedKey {
            kb: self.clone(),
            key,
        }
    }
}

impl TestInputDevice for TestBackendKb {
    fn common(&self) -> &TestInputDeviceCommon {
        &self.common
    }
}

impl TestInputDevice for TestBackendMouse {
    fn common(&self) -> &TestInputDeviceCommon {
        &self.common
    }

    fn set_left_handed(&self, left_handed: bool) {
        self.left_handed.set(left_handed)
    }

    fn set_accel_profile(&self, profile: InputDeviceAccelProfile) {
        self.accel_profile.set(profile)
    }

    fn set_accel_speed(&self, speed: f64) {
        self.accel_speed.set(speed)
    }

    fn set_transform_matrix(&self, matrix: TransformMatrix) {
        self.transform_matrix.set(matrix);
    }
}

pub struct TestInputDeviceCommon {
    pub id: InputDeviceId,
    pub removed: Cell<bool>,
    pub events: SyncQueue<InputEvent>,
    pub on_change: CloneCell<Option<Rc<dyn Fn()>>>,
    pub capabilities: CopyHashMap<InputDeviceCapability, ()>,
    pub name: Rc<String>,
    pub state: Rc<State>,
}

impl TestInputDeviceCommon {
    pub fn event(&self, e: InputEvent) {
        self.events.push(e);
        if let Some(oc) = self.on_change.get() {
            oc();
        }
    }
}

trait TestInputDevice: InputDevice {
    fn common(&self) -> &TestInputDeviceCommon;

    fn set_left_handed(&self, left_handed: bool) {
        let _ = left_handed;
    }

    fn set_accel_profile(&self, profile: InputDeviceAccelProfile) {
        let _ = profile;
    }

    fn set_accel_speed(&self, speed: f64) {
        let _ = speed;
    }

    fn set_transform_matrix(&self, matrix: TransformMatrix) {
        let _ = matrix;
    }

    fn set_tap_enabled(&self, enabled: bool) {
        let _ = enabled;
    }

    fn set_drag_enabled(&self, enabled: bool) {
        let _ = enabled;
    }

    fn set_drag_lock_enabled(&self, enabled: bool) {
        let _ = enabled;
    }

    fn set_natural_scrolling_enabled(&self, enabled: bool) {
        let _ = enabled;
    }
}

impl<T: TestInputDevice> InputDevice for T {
    fn id(&self) -> InputDeviceId {
        self.common().id
    }

    fn removed(&self) -> bool {
        self.common().removed.get()
    }

    fn event(&self) -> Option<InputEvent> {
        self.common().events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.common().on_change.set(Some(cb));
    }

    fn grab(&self, _grab: bool) {
        // nothing
    }

    fn has_capability(&self, cap: InputDeviceCapability) -> bool {
        self.common().capabilities.contains(&cap)
    }

    fn set_left_handed(&self, left_handed: bool) {
        <Self as TestInputDevice>::set_left_handed(self, left_handed)
    }

    fn set_accel_profile(&self, profile: InputDeviceAccelProfile) {
        <Self as TestInputDevice>::set_accel_profile(self, profile)
    }

    fn set_accel_speed(&self, speed: f64) {
        <Self as TestInputDevice>::set_accel_speed(self, speed)
    }

    fn set_transform_matrix(&self, matrix: TransformMatrix) {
        <Self as TestInputDevice>::set_transform_matrix(self, matrix)
    }

    fn name(&self) -> Rc<String> {
        self.common().name.clone()
    }

    fn dev_t(&self) -> Option<c::dev_t> {
        None
    }

    fn set_tap_enabled(&self, enabled: bool) {
        <Self as TestInputDevice>::set_tap_enabled(self, enabled)
    }

    fn set_drag_enabled(&self, enabled: bool) {
        <Self as TestInputDevice>::set_drag_enabled(self, enabled)
    }

    fn set_drag_lock_enabled(&self, enabled: bool) {
        <Self as TestInputDevice>::set_drag_lock_enabled(self, enabled)
    }

    fn set_natural_scrolling_enabled(&self, enabled: bool) {
        <Self as TestInputDevice>::set_natural_scrolling_enabled(self, enabled)
    }
}
