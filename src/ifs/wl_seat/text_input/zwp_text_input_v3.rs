use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            wl_seat::{
                WlSeatGlobal,
                text_input::{
                    MAX_TEXT_SIZE, TextConnectReason, TextDisconnectReason, TextInputConnection,
                    zwp_input_method_v2::ZwpInputMethodV2,
                },
            },
            wl_surface::WlSurface,
        },
        leaks::Tracker,
        object::{Object, Version},
        rect::Rect,
        utils::{clonecell::CloneCell, numcell::NumCell},
        wire::{ZwpTextInputV3Id, zwp_text_input_v3::*},
    },
    std::{cell::RefCell, collections::hash_map::Entry, mem, rc::Rc},
    thiserror::Error,
};

pub struct ZwpTextInputV3 {
    pub id: ZwpTextInputV3Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    seat: Rc<WlSeatGlobal>,
    num_commits: NumCell<u32>,

    state: RefCell<State>,
    pending: RefCell<Pending>,

    pub connection: CloneCell<Option<Rc<TextInputConnection>>>,
}

impl ZwpTextInputV3 {
    pub fn cursor_rect(&self) -> Rect {
        self.state.borrow().cursor_rectangle
    }

    pub fn new(
        id: ZwpTextInputV3Id,
        client: &Rc<Client>,
        seat: &Rc<WlSeatGlobal>,
        version: Version,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            seat: seat.clone(),
            num_commits: Default::default(),
            state: Default::default(),
            pending: Default::default(),
            connection: Default::default(),
        }
    }

    fn detach(&self) {
        self.do_disable();
        {
            let tis = &mut *self.seat.text_inputs.borrow_mut();
            if let Entry::Occupied(mut oe) = tis.entry(self.client.id) {
                oe.get_mut().remove(&self.id);
                if oe.get().is_empty() {
                    oe.remove();
                }
            }
        }
    }

    pub fn send_all_to(&self, im: &ZwpInputMethodV2) {
        let state = &*self.state.borrow();
        {
            let (a, b, c) = &state.surrounding_text;
            im.send_surrounding_text(a, *b, *c);
        }
        im.send_content_type(state.content_type.0, state.content_type.1);
    }

    pub fn send_enter(&self, surface: &WlSurface) {
        self.client.event(Enter {
            self_id: self.id,
            surface: surface.id,
        });
    }

    pub fn send_leave(&self, surface: &WlSurface) {
        self.client.event(Leave {
            self_id: self.id,
            surface: surface.id,
        });
    }

    pub fn send_preedit_string(&self, text: Option<&str>, cursor_begin: i32, cursor_end: i32) {
        self.client.event(PreeditString {
            self_id: self.id,
            text,
            cursor_begin,
            cursor_end,
        });
    }

    pub fn send_commit_string(&self, text: Option<&str>) {
        self.client.event(CommitString {
            self_id: self.id,
            text,
        });
    }

    pub fn send_delete_surrounding_text(&self, before_length: u32, after_length: u32) {
        self.client.event(DeleteSurroundingText {
            self_id: self.id,
            before_length,
            after_length,
        });
    }

    pub fn send_done(&self) {
        self.client.event(Done {
            self_id: self.id,
            serial: self.num_commits.get(),
        });
    }

    fn do_enable(self: &Rc<Self>) {
        if self.seat.text_input.is_some() {
            return;
        }
        let Some(surface) = self.seat.keyboard_node.get().node_into_surface() else {
            return;
        };
        if surface.client.id != self.client.id {
            return;
        }
        self.seat.text_input.set(Some(self.clone()));
        self.seat
            .create_text_input_connection(TextConnectReason::TextInputEnabled);
    }

    fn do_disable(&self) {
        if let Some(con) = self.connection.take() {
            con.disconnect(TextDisconnectReason::TextInputDisabled);
            self.seat.text_input.take();
        }
    }
}

#[derive(Default)]
struct State {
    enabled: bool,
    surrounding_text: (String, u32, u32),
    text_change_cause: u32,
    content_type: (u32, u32),
    cursor_rectangle: Rect,
}

#[derive(Default)]
struct Pending {
    enabled: Option<bool>,
    cursor_rect: Option<Rect>,
    content_type: Option<(u32, u32)>,
    text_change_cause: Option<u32>,
    surrounding_text: Option<(String, u32, u32)>,
}

impl ZwpTextInputV3RequestHandler for ZwpTextInputV3 {
    type Error = ZwpTextInputV3Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn enable(&self, _req: Enable, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pending.borrow_mut().enabled = Some(true);
        Ok(())
    }

    fn disable(&self, _req: Disable, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pending.borrow_mut().enabled = Some(false);
        Ok(())
    }

    fn set_surrounding_text(
        &self,
        req: SetSurroundingText<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        if req.text.len() > MAX_TEXT_SIZE {
            return Err(ZwpTextInputV3Error::TooLarge);
        }
        if !req.text.is_char_boundary(req.cursor as usize) {
            return Err(ZwpTextInputV3Error::CursorNotCharBoundary);
        }
        if !req.text.is_char_boundary(req.anchor as usize) {
            return Err(ZwpTextInputV3Error::AnchorNotCharBoundary);
        }
        self.pending.borrow_mut().surrounding_text =
            Some((req.text.to_string(), req.cursor as _, req.anchor as _));
        Ok(())
    }

    fn set_text_change_cause(
        &self,
        req: SetTextChangeCause,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.pending.borrow_mut().text_change_cause = Some(req.cause);
        Ok(())
    }

    fn set_content_type(&self, req: SetContentType, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pending.borrow_mut().content_type = Some((req.hint, req.purpose));
        Ok(())
    }

    fn set_cursor_rectangle(
        &self,
        req: SetCursorRectangle,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let Some(rect) = Rect::new_sized(req.x, req.y, req.width, req.height) else {
            return Err(ZwpTextInputV3Error::InvalidRectangle);
        };
        self.pending.borrow_mut().cursor_rect = Some(rect);
        Ok(())
    }

    fn commit(&self, _req: Commit, slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.num_commits.fetch_add(1);
        let pending = self.pending.take();
        let state = &mut *self.state.borrow_mut();
        let mut sent_any = false;
        if let Some(val) = pending.enabled {
            sent_any = true;
            if val {
                mem::take(state);
                if let Some(con) = self.connection.get() {
                    con.input_method.activate();
                } else {
                    slf.do_enable();
                }
            } else {
                self.do_disable();
            }
            state.enabled = val;
        }
        let con = self.connection.get();
        if let Some(val) = pending.cursor_rect {
            if state.cursor_rectangle != val
                && let Some(con) = &con
            {
                for (_, popup) in &con.input_method.popups {
                    popup.schedule_positioning();
                }
            }
            state.cursor_rectangle = val;
        }
        if let Some(val) = pending.content_type {
            if let Some(con) = &con {
                sent_any = true;
                con.input_method.send_content_type(val.0, val.1);
            }
            state.content_type = val;
        }
        if let Some(val) = pending.text_change_cause {
            if let Some(con) = &con {
                sent_any = true;
                con.input_method.send_text_change_cause(val);
            }
            state.text_change_cause = val;
        }
        if let Some(val) = pending.surrounding_text {
            if let Some(con) = &con {
                sent_any = true;
                con.input_method.send_surrounding_text(&val.0, val.1, val.2);
            }
            state.surrounding_text = val;
        }
        if sent_any && let Some(con) = &con {
            con.input_method.send_done();
        }
        Ok(())
    }
}

object_base! {
    self = ZwpTextInputV3;
    version = self.version;
}

impl Object for ZwpTextInputV3 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}

simple_add_obj!(ZwpTextInputV3);

#[derive(Debug, Error)]
pub enum ZwpTextInputV3Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Rectangle is invalid")]
    InvalidRectangle,
    #[error("The cursor is not at a char boundary")]
    CursorNotCharBoundary,
    #[error("The anchor is not at a char boundary")]
    AnchorNotCharBoundary,
    #[error("Text is larger than {} bytes", MAX_TEXT_SIZE)]
    TooLarge,
}
efrom!(ZwpTextInputV3Error, ClientError);
