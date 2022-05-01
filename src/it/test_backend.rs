use {
    crate::{
        async_engine::SpawnedFuture,
        backend::{
            Backend, Connector, ConnectorEvent, ConnectorId, ConnectorKernelId, InputDevice,
            InputDeviceAccelProfile, InputDeviceCapability, InputDeviceId, InputEvent,
            TransformMatrix,
        },
        compositor::TestFuture,
        state::State,
        utils::{clonecell::CloneCell, copyhashmap::CopyHashMap, syncqueue::SyncQueue},
    },
    std::{any::Any, cell::Cell, error::Error, pin::Pin, rc::Rc},
};

pub struct TestBackend {
    pub state: Rc<State>,
    pub test_future: TestFuture,
}

impl Backend for TestBackend {
    fn run(self: Rc<Self>) -> SpawnedFuture<Result<(), Box<dyn Error>>> {
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
        todo!()
    }
}

pub struct TestInputDevice {
    pub id: InputDeviceId,
    pub remove: Cell<bool>,
    pub events: SyncQueue<InputEvent>,
    pub on_change: CloneCell<Option<Rc<dyn Fn()>>>,
    pub capabilities: CopyHashMap<InputDeviceCapability, ()>,
    pub transform_matrix: Cell<TransformMatrix>,
    pub name: Rc<String>,
    pub accel_speed: Cell<f64>,
    pub accel_profile: Cell<InputDeviceAccelProfile>,
    pub left_handed: Cell<bool>,
}

impl InputDevice for TestInputDevice {
    fn id(&self) -> InputDeviceId {
        self.id
    }

    fn removed(&self) -> bool {
        self.remove.get()
    }

    fn event(&self) -> Option<InputEvent> {
        self.events.pop()
    }

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.on_change.set(Some(cb));
    }

    fn grab(&self, _grab: bool) {
        // nothing
    }

    fn has_capability(&self, cap: InputDeviceCapability) -> bool {
        self.capabilities.contains(&cap)
    }

    fn set_left_handed(&self, left_handed: bool) {
        self.left_handed.set(left_handed);
    }

    fn set_accel_profile(&self, profile: InputDeviceAccelProfile) {
        self.accel_profile.set(profile);
    }

    fn set_accel_speed(&self, speed: f64) {
        self.accel_speed.set(speed)
    }

    fn set_transform_matrix(&self, matrix: TransformMatrix) {
        self.transform_matrix.set(matrix);
    }

    fn name(&self) -> Rc<String> {
        self.name.clone()
    }
}
