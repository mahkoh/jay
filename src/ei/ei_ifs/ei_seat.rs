use {
    crate::{
        backend::KeyState,
        ei::{
            EiContext,
            ei_client::{EiClient, EiClientError},
            ei_ifs::{
                ei_button::EiButton,
                ei_device::{EI_DEVICE_TYPE_VIRTUAL, EiDevice, EiDeviceInterface},
                ei_keyboard::EiKeyboard,
                ei_pointer::EiPointer,
                ei_pointer_absolute::EiPointerAbsolute,
                ei_scroll::EiScroll,
                ei_touchscreen::EiTouchscreen,
            },
            ei_object::{EiInterface, EiObject, EiVersion},
        },
        fixed::Fixed,
        ifs::wl_seat::{
            PhysicalKeyboardId, WlSeatGlobal,
            wl_pointer::{HORIZONTAL_SCROLL, PendingScroll, VERTICAL_SCROLL},
        },
        keyboard::{DynKeyboardState, KeyboardState, KeyboardStateId},
        leaks::Tracker,
        tree::Node,
        utils::{array, bitflags::BitflagsExt, clonecell::CloneCell},
        wire_ei::{
            EiSeatId,
            ei_seat::{
                Bind, Capability, Destroyed, Device, Done, EiSeatRequestHandler, Name, Release,
            },
        },
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub const EI_CAP_POINTER: u64 = 1 << 0;
pub const EI_CAP_POINTER_ABSOLUTE: u64 = 1 << 1;
pub const EI_CAP_SCROLL: u64 = 1 << 2;
pub const EI_CAP_BUTTON: u64 = 1 << 3;
pub const EI_CAP_KEYBOARD: u64 = 1 << 4;
pub const EI_CAP_TOUCHSCREEN: u64 = 1 << 5;

pub const EI_CAP_ALL: u64 = (1 << 6) - 1;

pub struct EiSeat {
    pub id: EiSeatId,
    pub client: Rc<EiClient>,
    pub tracker: Tracker<Self>,
    pub version: EiVersion,
    pub seat: Rc<WlSeatGlobal>,
    pub capabilities: Cell<u64>,
    pub kb_state_id: Cell<KeyboardStateId>,
    pub keyboard_id: PhysicalKeyboardId,

    pub device: CloneCell<Option<Rc<EiDevice>>>,
    pub pointer: CloneCell<Option<Rc<EiPointer>>>,
    pub pointer_absolute: CloneCell<Option<Rc<EiPointerAbsolute>>>,
    pub keyboard: CloneCell<Option<Rc<EiKeyboard>>>,
    pub button: CloneCell<Option<Rc<EiButton>>>,
    pub scroll: CloneCell<Option<Rc<EiScroll>>>,
    pub touchscreen: CloneCell<Option<Rc<EiTouchscreen>>>,
}

impl EiSeat {
    fn is_sender(&self) -> bool {
        self.context() == EiContext::Sender
    }

    pub fn regions_changed(self: &Rc<Self>) {
        if self.touchscreen.is_none() && self.pointer_absolute.is_none() {
            return;
        }
        let kb_state = self.get_kb_state();
        let kb_state = kb_state.borrow();
        if let Err(e) = self.recreate_all(false, &kb_state) {
            self.client.error(e);
        }
    }

    pub fn handle_keyboard_state_change(
        self: &Rc<Self>,
        old_id: KeyboardStateId,
        new: &KeyboardState,
    ) {
        if self.keyboard.is_none() {
            return;
        }
        if self.kb_state_id.get() != old_id {
            return;
        }
        self.kb_state_id.set(new.id);
        if let Err(e) = self.recreate_all(false, new) {
            self.client.error(e);
        }
    }

    pub fn handle_modifiers_changed(self: &Rc<Self>, state: &KeyboardState) {
        let old_id = self.kb_state_id.get();
        if old_id != state.id {
            if self.is_sender() {
                return;
            }
            self.handle_keyboard_state_change(old_id, state);
            return;
        }
        if let Some(kb) = self.keyboard.get() {
            kb.send_modifiers(state);
        }
    }

    pub fn handle_key(
        self: &Rc<Self>,
        time_usec: u64,
        key: u32,
        state: KeyState,
        kb_state: &KeyboardState,
    ) {
        if self.is_sender() {
            return;
        }
        let old_id = self.kb_state_id.get();
        if old_id != kb_state.id {
            self.handle_keyboard_state_change(old_id, kb_state);
        }
        if let Some(kb) = self.keyboard.get() {
            kb.send_key(key, state);
            kb.device.send_frame(self.client.serial(), time_usec);
        }
    }

    pub fn handle_motion_abs(&self, time_usec: u64, x: Fixed, y: Fixed) {
        if self.is_sender() {
            return;
        }
        if let Some(v) = self.pointer_absolute.get() {
            v.send_motion_absolute(x, y);
            v.device.send_frame(self.client.serial(), time_usec);
        }
    }

    pub fn handle_motion(&self, time_usec: u64, dx: Fixed, dy: Fixed) {
        if self.is_sender() {
            return;
        }
        if let Some(v) = self.pointer.get() {
            v.send_motion(dx, dy);
            v.device.send_frame(self.client.serial(), time_usec);
        }
    }

    pub fn handle_button(&self, time_usec: u64, button: u32, state: KeyState) {
        if self.is_sender() {
            return;
        }
        if let Some(b) = self.button.get() {
            b.send_button(button, state);
            b.device.send_frame(self.client.serial(), time_usec);
        }
    }

    pub fn handle_pending_scroll(&self, time_usec: u64, ps: &PendingScroll) {
        if self.is_sender() {
            return;
        }
        if let Some(b) = self.scroll.get() {
            b.send_scroll(
                ps.px[HORIZONTAL_SCROLL].get().unwrap_or_default(),
                ps.px[VERTICAL_SCROLL].get().unwrap_or_default(),
            );
            b.send_scroll_discrete(
                ps.v120[HORIZONTAL_SCROLL].get().unwrap_or_default(),
                ps.v120[VERTICAL_SCROLL].get().unwrap_or_default(),
            );
            b.send_scroll_stop(
                ps.stop[HORIZONTAL_SCROLL].get(),
                ps.stop[VERTICAL_SCROLL].get(),
            );
            b.device.send_frame(self.client.serial(), time_usec);
        }
    }

    pub fn handle_touch_down(&self, id: u32, x: Fixed, y: Fixed) {
        if self.is_sender() {
            return;
        }
        if let Some(b) = self.touchscreen.get() {
            b.send_down(id, x, y);
        }
    }

    pub fn handle_touch_motion(&self, id: u32, x: Fixed, y: Fixed) {
        if self.is_sender() {
            return;
        }
        if let Some(b) = self.touchscreen.get() {
            b.send_motion(id, x, y);
        }
    }

    pub fn handle_touch_up(&self, id: u32) {
        if self.is_sender() {
            return;
        }
        if let Some(b) = self.touchscreen.get() {
            b.send_up(id);
        }
    }

    pub fn handle_touch_cancel(&self, id: u32) {
        if self.is_sender() {
            return;
        }
        if let Some(b) = self.touchscreen.get() {
            if self.client.versions.ei_touchscreen() >= EiVersion(2) {
                b.send_cancel(id);
            } else {
                b.send_up(id);
            }
        }
    }

    pub fn handle_touch_frame(&self, time_usec: u64) {
        if self.is_sender() {
            return;
        }
        if let Some(b) = self.touchscreen.get() {
            b.device.send_frame(self.client.serial(), time_usec);
        }
    }

    pub fn send_capability(&self, interface: EiInterface, mask: u64) {
        self.client.event(Capability {
            self_id: self.id,
            mask,
            interface: interface.0,
        });
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_name(&self, name: &str) {
        self.client.event(Name {
            self_id: self.id,
            name,
        });
    }

    pub fn send_device(&self, device: &EiDevice) {
        self.client.event(Device {
            self_id: self.id,
            device: device.id,
            version: device.version.0,
        });
    }

    pub fn send_destroyed(&self) {
        self.client.event(Destroyed {
            self_id: self.id,
            serial: self.client.serial(),
        });
    }

    fn create_interface<T>(self: &Rc<Self>, field: &CloneCell<Option<Rc<T>>>, version: EiVersion)
    where
        T: EiDeviceInterface,
    {
        if version == EiVersion(0) {
            return;
        }
        let Some(device) = self.device.get() else {
            return;
        };
        let interface = T::new(&device, version);
        self.client.add_server_obj(&interface);
        device.send_interface(&*interface);
        field.set(Some(interface.clone()));
    }

    fn create_pointer(self: &Rc<Self>) {
        self.create_interface(&self.pointer, self.client.versions.ei_pointer());
    }

    fn create_button(self: &Rc<Self>) {
        self.create_interface(&self.button, self.client.versions.ei_button());
    }

    fn create_keyboard(self: &Rc<Self>) {
        self.create_interface(&self.keyboard, self.client.versions.ei_keyboard());
    }

    fn create_scroll(self: &Rc<Self>) {
        self.create_interface(&self.scroll, self.client.versions.ei_scroll());
    }

    fn create_pointer_absolute(self: &Rc<Self>) {
        self.create_interface(
            &self.pointer_absolute,
            self.client.versions.ei_pointer_absolute(),
        );
    }

    fn create_touchscreen(self: &Rc<Self>) {
        self.create_interface(&self.touchscreen, self.client.versions.ei_touchscreen());
    }

    fn get_kb_state(&self) -> Rc<dyn DynKeyboardState> {
        match self.context() {
            EiContext::Sender => self.seat.seat_kb_state(),
            EiContext::Receiver => self.seat.latest_kb_state(),
        }
    }

    fn recreate_all(
        self: &Rc<Self>,
        create_all: bool,
        kb_state: &KeyboardState,
    ) -> Result<(), EiClientError> {
        self.kb_state_id.set(kb_state.id);
        let have_outputs = self.client.state.root.outputs.is_not_empty();
        let create_pointer = create_all || self.pointer.is_some();
        let create_pointer_absolute =
            have_outputs && (create_all || self.pointer_absolute.is_some());
        let create_scroll = create_all || self.scroll.is_some();
        let create_button = create_all || self.button.is_some();
        let create_keyboard = create_all || self.keyboard.is_some();
        let create_touchscreen = have_outputs && (create_all || self.touchscreen.is_some());
        if let Some(device) = self.device.take() {
            device.destroy()?;
        }
        let version = self.client.versions.ei_device();
        if version == EiVersion(0) {
            return Ok(());
        }
        let device = Rc::new(EiDevice {
            id: self.client.new_id(),
            client: self.client.clone(),
            tracker: Default::default(),
            version,
            seat: self.clone(),
            button_changes: Default::default(),
            touch_changes: Default::default(),
            scroll_px: array::from_fn(|_| Default::default()),
            scroll_v120: array::from_fn(|_| Default::default()),
            scroll_stop: array::from_fn(|_| Default::default()),
            absolute_motion: Default::default(),
            relative_motion: Default::default(),
            key_changes: Default::default(),
        });
        track!(self.client, device);
        self.device.set(Some(device.clone()));
        self.client.add_server_obj(&device);
        self.send_device(&device);
        device.send_device_type(EI_DEVICE_TYPE_VIRTUAL);
        let caps = self.capabilities.get();
        macro_rules! apply {
            ($cap:expr, $create:ident) => {
                if $create && caps.contains($cap) {
                    self.$create();
                }
            };
        }
        apply!(EI_CAP_POINTER, create_pointer);
        apply!(EI_CAP_POINTER_ABSOLUTE, create_pointer_absolute);
        apply!(EI_CAP_SCROLL, create_scroll);
        apply!(EI_CAP_BUTTON, create_button);
        apply!(EI_CAP_KEYBOARD, create_keyboard);
        apply!(EI_CAP_TOUCHSCREEN, create_touchscreen);
        for output in self.client.state.root.outputs.lock().values() {
            device.send_region_mapping_id(&output.global.connector.name);
            device.send_region(
                output.node_absolute_position(),
                output.global.persistent.scale.get(),
            );
        }
        if let Some(kb) = self.keyboard.get() {
            kb.send_keymap(kb_state);
        }
        device.send_done();
        device.send_resumed(self.client.serial());
        if self.context() == EiContext::Receiver {
            device.send_start_emulating(self.client.serial(), 1);
        }
        if let Some(kb) = self.keyboard.get() {
            kb.send_modifiers(kb_state);
        }
        Ok(())
    }

    pub fn is_touch_input(&self) -> bool {
        self.capabilities.get().contains(EI_CAP_TOUCHSCREEN) && self.context() == EiContext::Sender
    }
}

impl EiSeatRequestHandler for EiSeat {
    type Error = EiSeatError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.seat.remove_ei_seat(self);
        self.send_destroyed();
        if let Some(device) = self.device.take() {
            device.destroy()?;
        }
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn bind(&self, req: Bind, slf: &Rc<Self>) -> Result<(), Self::Error> {
        let caps = req.capabilities;
        let unknown = caps & !EI_CAP_ALL;
        if unknown != 0 {
            return Err(EiSeatError::UnknownCapabilities(unknown));
        }
        self.capabilities.set(caps);
        let kb_state = self.get_kb_state();
        slf.recreate_all(true, &kb_state.borrow())?;
        self.seat.update_capabilities();
        Ok(())
    }
}

ei_object_base! {
    self = EiSeat;
    version = self.version;
}

impl EiObject for EiSeat {
    fn break_loops(&self) {
        self.seat.remove_ei_seat(self);
        self.device.take();
        self.pointer.take();
        self.pointer_absolute.take();
        self.keyboard.take();
        self.button.take();
        self.scroll.take();
        self.touchscreen.take();
    }
}

#[derive(Debug, Error)]
pub enum EiSeatError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
    #[error("Capabilities {0} are unknown")]
    UnknownCapabilities(u64),
}
efrom!(EiSeatError, EiClientError);
