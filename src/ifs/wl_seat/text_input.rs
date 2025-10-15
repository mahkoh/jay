use {
    crate::{
        backend::KeyState,
        ifs::{
            wl_seat::{WlSeatGlobal, text_input::zwp_text_input_v3::ZwpTextInputV3},
            wl_surface::{WlSurface, zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2},
        },
        keyboard::KeyboardState,
        utils::smallmap::SmallMap,
        wire::ZwpInputPopupSurfaceV2Id,
    },
    std::rc::Rc,
};

pub mod zwp_input_method_keyboard_grab_v2;
pub mod zwp_input_method_manager_v2;
pub mod zwp_input_method_v2;
pub mod zwp_text_input_manager_v3;
pub mod zwp_text_input_v3;

const MAX_TEXT_SIZE: usize = 4000;

pub struct TextInputConnection {
    pub seat: Rc<WlSeatGlobal>,
    pub text_input: Rc<ZwpTextInputV3>,
    pub input_method: Rc<dyn InputMethod>,
    pub surface: Rc<WlSurface>,
}

pub trait InputMethod {
    fn set_connection(&self, con: Option<&Rc<TextInputConnection>>);
    fn popups(&self) -> &SmallMap<ZwpInputPopupSurfaceV2Id, Rc<ZwpInputPopupSurfaceV2>, 1>;
    fn activate(&self);
    fn deactivate(&self);
    fn content_type(&self, hint: u32, purpose: u32);
    fn text_change_cause(&self, cause: u32);
    fn surrounding_text(&self, text: &str, cursor: u32, anchor: u32);
    fn done(self: Rc<Self>, seat: &WlSeatGlobal);
}

pub trait InputMethodKeyboardGrab {
    fn on_key(&self, time_usec: u64, key: u32, state: KeyState, kb_state: &KeyboardState) -> bool;
    fn on_modifiers(&self, kb_state: &KeyboardState) -> bool;
    fn on_repeat_info(&self);
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TextConnectReason {
    TextInputEnabled,
    InputMethodCreated,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TextDisconnectReason {
    FocusLost,
    TextInputDisabled,
    InputMethodDestroyed,
}

impl WlSeatGlobal {
    fn create_text_input_connection(self: &Rc<Self>, text_connect_reason: TextConnectReason) {
        let Some(im) = self.input_method.get() else {
            return;
        };
        let Some(ti) = self.text_input.get() else {
            return;
        };
        let Some(surface) = self.keyboard_node.get().node_into_surface() else {
            log::warn!("Seat has text input but keyboard node is not a surface");
            return;
        };
        if surface.client.id != ti.client.id {
            log::warn!("Seat's text input belongs to different client than the keyboard node");
            return;
        }
        let con = Rc::new(TextInputConnection {
            seat: self.clone(),
            text_input: ti.clone(),
            input_method: im.clone(),
            surface: surface.clone(),
        });
        con.connect(text_connect_reason);
    }
}

impl TextInputConnection {
    fn connect(self: &Rc<Self>, reason: TextConnectReason) {
        self.input_method.set_connection(Some(self));
        self.text_input.connection.set(Some(self.clone()));
        self.surface
            .text_input_connections
            .insert(self.seat.id, self.clone());

        self.input_method.activate();
        if reason == TextConnectReason::InputMethodCreated {
            self.text_input.send_all_to(&*self.input_method);
            self.input_method.clone().done(&self.seat);
        }
    }

    pub fn disconnect(&self, reason: TextDisconnectReason) {
        self.text_input.connection.take();
        self.input_method.set_connection(None);
        self.surface.text_input_connections.remove(&self.seat.id);

        if reason != TextDisconnectReason::InputMethodDestroyed {
            self.input_method.deactivate();
            self.input_method.clone().done(&self.seat);
            for (_, popup) in self.input_method.popups() {
                popup.update_visible();
            }
        }
        if reason != TextDisconnectReason::TextInputDisabled {
            self.text_input.send_preedit_string(None, 0, 0);
            self.text_input.send_commit_string(None);
            self.text_input.send_done();
        }
    }
}
