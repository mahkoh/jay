use {
    crate::{
        backend::KeyState,
        ifs::{
            wl_seat::{
                WlSeatGlobal,
                text_input::{simple_im::SimpleIm, zwp_text_input_v3::ZwpTextInputV3},
            },
            wl_surface::{WlSurface, zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2},
        },
        keyboard::KeyboardState,
        utils::smallmap::SmallMap,
        wire::ZwpInputPopupSurfaceV2Id,
    },
    std::rc::Rc,
};

pub mod simple_im;
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
    fn is_simple(&self) -> bool;
    fn cancel_simple(&self, seat: &WlSeatGlobal);
    fn enable_unicode_input(&self);
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
    pub fn enable_unicode_input(&self) {
        if let Some(im) = self.input_method.get() {
            im.enable_unicode_input();
        }
    }

    pub fn set_simple_im_enabled(self: &Rc<Self>, enabled: bool) {
        if self.simple_im_enabled.replace(enabled) == enabled {
            return;
        }
        if enabled {
            if self.input_method.is_none()
                && let Some(im) = self.simple_im.get()
            {
                self.set_input_method(im);
            }
        } else {
            if let Some(im) = self.input_method.get()
                && im.is_simple()
            {
                self.input_method.take();
                im.cancel_simple(self);
            }
        }
    }

    pub fn simple_im_enabled(&self) -> bool {
        self.simple_im_enabled.get()
    }

    pub fn reload_simple_im(self: &Rc<Self>) {
        let im = SimpleIm::new(&self.state.kb_ctx.ctx);
        self.simple_im.set(im.clone());
        if self.simple_im_enabled.get() && self.can_set_new_im() {
            if let Some(im) = im {
                self.set_input_method(im);
            } else if let Some(old) = self.input_method.take() {
                old.cancel_simple(self);
            }
        }
    }

    fn can_set_new_im(&self) -> bool {
        match self.input_method.get() {
            None => true,
            Some(im) => im.is_simple(),
        }
    }

    fn cannot_set_new_im(&self) -> bool {
        !self.can_set_new_im()
    }

    fn set_input_method(self: &Rc<Self>, im: Rc<dyn InputMethod>) {
        if let Some(old) = self.input_method.take() {
            old.cancel_simple(self);
        }
        self.input_method.set(Some(im));
        self.create_text_input_connection(TextConnectReason::InputMethodCreated);
    }

    fn remove_input_method(self: &Rc<Self>) {
        self.input_method.take();
        if self.simple_im_enabled.get()
            && let Some(im) = self.simple_im.get()
        {
            self.set_input_method(im);
        }
    }

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
