use {
    crate::{
        async_engine::SpawnedFuture,
        backend::{
            Backend, BackendEvent, Connector, ConnectorEvent, ConnectorId, ConnectorKernelId,
            InputDevice, InputDeviceAccelProfile, InputDeviceCapability, InputDeviceId, InputEvent,
            KeyState, Mode, MonitorInfo, TransformMatrix,
        },
        compositor::TestFuture,
        fixed::Fixed,
        render::{RenderContext, RenderError},
        state::State,
        time::Time,
        utils::{
            clonecell::CloneCell, copyhashmap::CopyHashMap, errorfmt::ErrorFmt, oserror::OsError,
            syncqueue::SyncQueue,
        },
        video::drm::{ConnectorType, Drm},
    },
    bstr::ByteSlice,
    std::{any::Any, cell::Cell, io, os::unix::ffi::OsStrExt, pin::Pin, rc::Rc},
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
    RenderContext(#[source] RenderError),
}

pub struct TestBackend {
    pub state: Rc<State>,
    pub test_future: TestFuture,
    pub default_connector: Rc<TestConnector>,
    pub default_mouse: Rc<TestBackendMouse>,
    pub default_kb: Rc<TestBackendKb>,
}

impl TestBackend {
    pub fn new(state: &Rc<State>, future: TestFuture) -> Self {
        let default_connector = Rc::new(TestConnector {
            id: state.connector_ids.next(),
            kernel_id: ConnectorKernelId {
                ty: ConnectorType::VGA,
                idx: 1,
            },
            events: Default::default(),
            on_change: Default::default(),
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
            },
        });
        Self {
            state: state.clone(),
            test_future: future,
            default_connector,
            default_mouse,
            default_kb,
        }
    }

    pub fn install_default(&self) {
        self.state
            .backend_events
            .push(BackendEvent::NewConnector(self.default_connector.clone()));
        let mode = Mode {
            width: 800,
            height: 600,
            refresh_rate_millihz: 60_000,
        };
        self.default_connector
            .events
            .push(ConnectorEvent::Connected(MonitorInfo {
                modes: vec![mode],
                manufacturer: "jay".to_string(),
                product: "TestConnector".to_string(),
                serial_number: self.default_connector.id.to_string(),
                initial_mode: mode,
                width_mm: 80,
                height_mm: 60,
            }));
        self.state
            .backend_events
            .push(BackendEvent::NewInputDevice(self.default_kb.clone()));
        self.state
            .backend_events
            .push(BackendEvent::NewInputDevice(self.default_mouse.clone()));
    }

    fn create_render_context(&self) -> Result<(), TestBackendError> {
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
            Ok(f) => f,
            Err(e) => {
                return Err(TestBackendError::OpenDrmNode(
                    node.as_os_str().as_bytes().as_bstr().to_string(),
                    e.into(),
                ))
            }
        };
        let drm = Drm::open_existing(file);
        let ctx = match RenderContext::from_drm_device(&drm) {
            Ok(ctx) => ctx,
            Err(e) => return Err(TestBackendError::RenderContext(e)),
        };
        self.state.set_render_ctx(&Rc::new(ctx));
        Ok(())
    }
}

impl Backend for TestBackend {
    fn run(self: Rc<Self>) -> SpawnedFuture<Result<(), Box<dyn std::error::Error>>> {
        let future = (self.test_future)(&self.state);
        let slf = self.clone();
        self.state.eng.spawn(async move {
            if let Err(e) = slf.create_render_context() {
                log::error!("Could not create render context: {}", ErrorFmt(e));
            }
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

    fn set_idle(&self, _idle: bool) {}

    fn supports_idle(&self) -> bool {
        true
    }

    fn supports_presentation_feedback(&self) -> bool {
        true
    }
}

pub struct TestConnector {
    pub id: ConnectorId,
    pub kernel_id: ConnectorKernelId,
    pub events: SyncQueue<ConnectorEvent>,
    pub on_change: CloneCell<Option<Rc<dyn Fn()>>>,
}

impl Connector for TestConnector {
    fn id(&self) -> ConnectorId {
        self.id
    }

    fn kernel_id(&self) -> ConnectorKernelId {
        self.kernel_id
    }

    fn event(&self) -> Option<ConnectorEvent> {
        self.events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.on_change.set(Some(cb));
    }

    fn damage(&self) {
        // nothing
    }
}

pub struct TestMouseClick {
    pub mouse: Rc<TestBackendMouse>,
    pub button: u32,
}

impl Drop for TestMouseClick {
    fn drop(&mut self) {
        self.mouse
            .common
            .event(InputEvent::Button(self.button, KeyState::Released));
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
            time_usec: Time::now_unchecked().usec(),
            dx: Fixed::from_f64(dx * self.accel_speed.get()),
            dy: Fixed::from_f64(dy * self.accel_speed.get()),
            dx_unaccelerated: Fixed::from_f64(dx),
            dy_unaccelerated: Fixed::from_f64(dy),
        })
    }

    pub fn click(self: &Rc<Self>, button: u32) -> TestMouseClick {
        self.common
            .event(InputEvent::Button(button, KeyState::Pressed));
        TestMouseClick {
            mouse: self.clone(),
            button,
        }
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
        self.kb
            .common
            .event(InputEvent::Key(self.key, KeyState::Released));
    }
}

impl TestBackendKb {
    pub fn press(self: &Rc<Self>, key: u32) -> PressedKey {
        self.common.event(InputEvent::Key(key, KeyState::Pressed));
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
}
