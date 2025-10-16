use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            wl_seat::{
                WlSeatGlobal,
                text_input::{
                    InputMethod, MAX_TEXT_SIZE, TextDisconnectReason, TextInputConnection,
                    zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2,
                },
            },
            wl_surface::zwp_input_popup_surface_v2::{
                ZwpInputPopupSurfaceV2, ZwpInputPopupSurfaceV2Error,
            },
        },
        keyboard::KeyboardStateId,
        leaks::Tracker,
        object::{Object, Version},
        utils::{clonecell::CloneCell, numcell::NumCell, smallmap::SmallMap},
        wire::{ZwpInputMethodV2Id, ZwpInputPopupSurfaceV2Id, zwp_input_method_v2::*},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    thiserror::Error,
};

pub struct ZwpInputMethodV2 {
    pub id: ZwpInputMethodV2Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub seat: Rc<WlSeatGlobal>,
    pub popups: SmallMap<ZwpInputPopupSurfaceV2Id, Rc<ZwpInputPopupSurfaceV2>, 1>,
    pub connection: CloneCell<Option<Rc<TextInputConnection>>>,
    pub inert: bool,
    pub num_done: NumCell<u32>,
    pub pending: RefCell<Pending>,
}

#[derive(Default)]
pub struct Pending {
    commit_string: Option<String>,
    delete_surrounding_text: Option<(u32, u32)>,
    preedit_string: Option<(String, i32, i32)>,
}

impl ZwpInputMethodV2 {
    fn detach(&self) {
        if let Some(con) = self.connection.get() {
            con.disconnect(TextDisconnectReason::InputMethodDestroyed);
        }
        self.popups.clear();
        if !self.inert {
            self.seat.remove_input_method();
        }
    }

    pub fn activate(&self) {
        self.pending.take();
        self.send_activate();
    }

    pub fn send_activate(&self) {
        self.client.event(Activate { self_id: self.id });
    }

    pub fn send_deactivate(&self) {
        self.client.event(Deactivate { self_id: self.id });
    }

    pub fn send_surrounding_text(&self, text: &str, cursor: u32, anchor: u32) {
        self.client.event(SurroundingText {
            self_id: self.id,
            text,
            cursor,
            anchor,
        });
    }

    pub fn send_text_change_cause(&self, cause: u32) {
        self.client.event(TextChangeCause {
            self_id: self.id,
            cause,
        });
    }

    pub fn send_content_type(&self, hint: u32, purpose: u32) {
        self.client.event(ContentType {
            self_id: self.id,
            hint,
            purpose,
        });
    }

    pub fn send_done(&self) {
        self.num_done.fetch_add(1);
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_unavailable(&self) {
        self.client.event(Unavailable { self_id: self.id });
    }
}

impl InputMethod for ZwpInputMethodV2 {
    fn set_connection(&self, con: Option<&Rc<TextInputConnection>>) {
        self.connection.set(con.cloned());
    }

    fn popups(&self) -> &SmallMap<ZwpInputPopupSurfaceV2Id, Rc<ZwpInputPopupSurfaceV2>, 1> {
        &self.popups
    }

    fn activate(&self) {
        self.activate();
    }

    fn deactivate(&self) {
        self.send_deactivate();
    }

    fn content_type(&self, hint: u32, purpose: u32) {
        self.send_content_type(hint, purpose);
    }

    fn text_change_cause(&self, cause: u32) {
        self.send_text_change_cause(cause);
    }

    fn surrounding_text(&self, text: &str, cursor: u32, anchor: u32) {
        self.send_surrounding_text(text, cursor, anchor);
    }

    fn done(self: Rc<Self>, _seat: &WlSeatGlobal) {
        (*self).send_done();
    }

    fn is_simple(&self) -> bool {
        false
    }

    fn cancel_simple(&self, _seat: &WlSeatGlobal) {
        unreachable!();
    }

    fn enable_unicode_input(&self) {
        // nothing
    }
}

impl ZwpInputMethodV2RequestHandler for ZwpInputMethodV2 {
    type Error = ZwpInputMethodV2Error;

    fn commit_string(&self, req: CommitString<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.text.len() > MAX_TEXT_SIZE {
            return Err(ZwpInputMethodV2Error::TooLarge);
        }
        self.pending.borrow_mut().commit_string = Some(req.text.to_string());
        Ok(())
    }

    fn set_preedit_string(
        &self,
        req: SetPreeditString<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if req.text.len() > MAX_TEXT_SIZE {
            return Err(ZwpInputMethodV2Error::TooLarge);
        }
        self.pending.borrow_mut().preedit_string =
            Some((req.text.to_string(), req.cursor_begin, req.cursor_end));
        Ok(())
    }

    fn delete_surrounding_text(
        &self,
        req: DeleteSurroundingText,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.pending.borrow_mut().delete_surrounding_text =
            Some((req.before_length, req.after_length));
        Ok(())
    }

    fn commit(&self, req: Commit, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.serial != self.num_done.get() {
            return Ok(());
        }
        let pending = self.pending.take();
        let Some(con) = self.connection.get() else {
            return Ok(());
        };
        if let Some(dst) = pending.delete_surrounding_text {
            con.text_input.send_delete_surrounding_text(dst.0, dst.1);
        }
        if let Some(dst) = pending.preedit_string {
            con.text_input
                .send_preedit_string(Some(&dst.0), dst.1, dst.2);
        }
        if let Some(dst) = pending.commit_string {
            con.text_input.send_commit_string(Some(&dst));
        }
        con.text_input.send_done();
        Ok(())
    }

    fn get_input_popup_surface(
        &self,
        req: GetInputPopupSurface,
        slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let popup = Rc::new(ZwpInputPopupSurfaceV2 {
            id: req.id,
            client: self.client.clone(),
            input_method: slf.clone(),
            surface,
            version: self.version,
            tracker: Default::default(),
            positioning_scheduled: Cell::new(false),
            was_on_screen: Default::default(),
        });
        track!(self.client, popup);
        self.client.add_client_obj(&popup)?;
        popup.install()?;
        Ok(())
    }

    fn grab_keyboard(&self, req: GrabKeyboard, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.seat.input_method_grab.is_some() {
            return Err(ZwpInputMethodV2Error::HasGrab);
        }
        let grab = Rc::new(ZwpInputMethodKeyboardGrabV2 {
            id: req.keyboard,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            input_method: slf.clone(),
            kb_state_id: Cell::new(KeyboardStateId::from_raw(0)),
        });
        track!(self.client, grab);
        self.client.add_client_obj(&grab)?;
        grab.send_repeat_info();
        self.seat.input_method_grab.set(Some(grab));
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpInputMethodV2;
    version = self.version;
}

impl Object for ZwpInputMethodV2 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwpInputMethodV2);

#[derive(Debug, Error)]
pub enum ZwpInputMethodV2Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    ZwpInputPopupSurfaceV2Error(#[from] ZwpInputPopupSurfaceV2Error),
    #[error("Text is larger than {} bytes", MAX_TEXT_SIZE)]
    TooLarge,
    #[error("Seat already has a grab")]
    HasGrab,
}
efrom!(ZwpInputMethodV2Error, ClientError);
