use {
    crate::{
        backend::KeyState,
        ifs::{
            wl_seat::{
                WlSeatGlobal,
                text_input::{
                    InputMethod, InputMethodKeyboardGrab, TextDisconnectReason, TextInputConnection,
                },
            },
            wl_surface::zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2,
        },
        keyboard::KeyboardState,
        utils::{clonecell::CloneCell, smallmap::SmallMap},
        wire::ZwpInputPopupSurfaceV2Id,
    },
    kbvm::{
        Keycode, ModifierMask, syms,
        xkb::{
            self,
            compose::{self, FeedResult},
            diagnostic::WriteToLog,
        },
    },
    std::{
        cell::{Cell, RefCell},
        fmt::Write,
        rc::Rc,
    },
};

pub struct SimpleIm {
    con: CloneCell<Option<Rc<TextInputConnection>>>,
    popups: SmallMap<ZwpInputPopupSurfaceV2Id, Rc<ZwpInputPopupSurfaceV2>, 1>,
    active: Cell<bool>,
    activate: Cell<Option<bool>>,
    table: compose::ComposeTable,
    initial_state: compose::State,
    states: RefCell<Vec<State>>,
    unicode_input: RefCell<UnicodeInput>,
    unicode_input_enabled: Cell<bool>,
}

struct State {
    state: compose::State,
    char: char,
}

#[derive(Default)]
struct UnicodeInput {
    text: String,
    cp: u32,
    cursor: i32,
    chars: usize,
}

impl SimpleIm {
    pub fn new(ctx: &xkb::Context) -> Option<Rc<Self>> {
        let table = ctx.compose_table_builder().build(WriteToLog)?;
        Some(Rc::new(Self {
            con: Default::default(),
            popups: Default::default(),
            active: Default::default(),
            activate: Default::default(),
            states: Default::default(),
            initial_state: table.create_state(),
            table,
            unicode_input: Default::default(),
            unicode_input_enabled: Default::default(),
        }))
    }
}

impl InputMethod for SimpleIm {
    fn set_connection(&self, con: Option<&Rc<TextInputConnection>>) {
        self.con.set(con.cloned());
    }

    fn popups(&self) -> &SmallMap<ZwpInputPopupSurfaceV2Id, Rc<ZwpInputPopupSurfaceV2>, 1> {
        &self.popups
    }

    fn activate(&self) {
        self.activate.set(Some(true));
    }

    fn deactivate(&self) {
        self.activate.set(Some(false));
    }

    fn content_type(&self, _hint: u32, _purpose: u32) {
        // nothing
    }

    fn text_change_cause(&self, _cause: u32) {
        // nothing
    }

    fn surrounding_text(&self, _text: &str, _cursor: u32, _anchor: u32) {
        // nothing
    }

    fn done(self: Rc<Self>, seat: &WlSeatGlobal) {
        let Some(active) = self.activate.take() else {
            return;
        };
        self.active.set(active);
        if active {
            self.states.borrow_mut().clear();
            self.unicode_input_enabled.set(false);
            seat.input_method_grab.set(Some(self));
        } else {
            seat.input_method_grab.take();
        }
    }

    fn is_simple(&self) -> bool {
        true
    }

    fn cancel_simple(&self, seat: &WlSeatGlobal) {
        seat.input_method_grab.take();
        if let Some(con) = self.con.get() {
            con.disconnect(TextDisconnectReason::InputMethodDestroyed);
        }
    }

    fn enable_unicode_input(&self) {
        if !self.active.get() {
            return;
        }
        let Some(con) = self.con.get() else {
            return;
        };
        if self.unicode_input_enabled.replace(true) {
            return;
        }
        self.states.borrow_mut().clear();
        let ui = &mut *self.unicode_input.borrow_mut();
        ui.cp = 0;
        ui.chars = 0;
        ui.flush_preedit(&con);
    }
}

impl UnicodeInput {
    fn update_text(&mut self) {
        self.text.clear();
        if self.chars == 0 {
            let _ = write!(self.text, "U+");
            self.cursor = self.text.len() as _;
            return;
        }
        let _ = write!(self.text, "U+{:x}", self.cp);
        self.cursor = self.text.len() as _;
        if let Some(char) = char::from_u32(self.cp) {
            let _ = write!(self.text, " = {}", char);
        }
    }

    fn flush_preedit(&mut self, con: &TextInputConnection) {
        self.update_text();
        con.text_input
            .send_preedit_string(Some(&self.text), self.cursor, self.cursor);
        con.text_input.send_done();
    }
}

impl InputMethodKeyboardGrab for SimpleIm {
    fn on_key(&self, _time_usec: u64, key: u32, state: KeyState, kb_state: &KeyboardState) -> bool {
        if state == KeyState::Released {
            return true;
        }
        let Some(con) = self.con.get() else {
            return true;
        };
        let mut buf = [0; 4];
        let mut forward_to_node = true;
        if self.unicode_input_enabled.get() {
            forward_to_node = false;
        }
        let states = &mut *self.states.borrow_mut();
        let ui = &mut self.unicode_input.borrow_mut();
        let lookup = kb_state.map.lookup_table.lookup(
            kb_state.mods.group,
            kb_state.mods.mods,
            Keycode::from_evdev(key),
        );
        let mods = lookup.remaining_mods();
        let is_control = mods.contains(ModifierMask::CONTROL);
        for sym in lookup {
            let sym = sym.keysym();
            if self.unicode_input_enabled.get() {
                let is_terminator = matches!(
                    sym,
                    syms::Return | syms::KP_Enter | syms::space | syms::KP_Space
                );
                if (is_terminator || (sym == syms::j && is_control))
                    && ui.chars > 0
                    && let Some(char) = char::from_u32(ui.cp)
                {
                    self.unicode_input_enabled.set(false);
                    let s = char.encode_utf8(&mut buf);
                    con.text_input.send_preedit_string(None, 0, 0);
                    con.text_input.send_commit_string(Some(s));
                    con.text_input.send_done();
                } else if sym == syms::Escape
                    || (sym == syms::c && is_control)
                    || (ui.chars == 0 && sym == syms::BackSpace)
                    || (ui.chars == 0 && matches!(sym, syms::w | syms::d) && is_control)
                {
                    self.unicode_input_enabled.set(false);
                    con.text_input.send_preedit_string(None, 0, 0);
                    con.text_input.send_done();
                } else if sym == syms::BackSpace && ui.chars > 0 {
                    ui.chars -= 1;
                    ui.cp >>= 4;
                    ui.flush_preedit(&con);
                } else if sym == syms::w && is_control {
                    ui.chars = 0;
                    ui.cp = 0;
                    ui.flush_preedit(&con);
                } else if let Some(c) = sym.char()
                    && ui.chars < 6
                    && !is_control
                {
                    let c = match c {
                        '0'..='9' => c as u32 - '0' as u32,
                        'a'..='f' => c as u32 - 'a' as u32 + 10,
                        'A'..='F' => c as u32 - 'A' as u32 + 10,
                        _ => continue,
                    };
                    ui.cp = (ui.cp << 4) | c;
                    if ui.cp != 0 {
                        ui.chars += 1;
                        ui.flush_preedit(&con);
                    }
                }
                continue;
            }
            let mut new_state = states
                .last()
                .map(|s| s.state.clone())
                .unwrap_or_else(|| self.initial_state.clone());
            let Some(fr) = self.table.feed(&mut new_state, sym) else {
                continue;
            };
            forward_to_node = false;
            let mut send_preedit = |char: char| {
                let s = char.encode_utf8(&mut buf);
                let len = s.len() as i32;
                con.text_input.send_preedit_string(Some(s), len, len);
            };
            match fr {
                FeedResult::Pending => {
                    let char = sym.char().unwrap_or('·');
                    states.push(State {
                        state: new_state,
                        char,
                    });
                    send_preedit(char);
                    con.text_input.send_done();
                }
                FeedResult::Aborted
                    if sym == syms::Escape || (matches!(sym, syms::c | syms::w) && is_control) =>
                {
                    states.clear();
                    con.text_input.send_preedit_string(None, 0, 0);
                    con.text_input.send_done();
                }
                FeedResult::Aborted if sym == syms::BackSpace => {
                    states.pop();
                    if let Some(state) = states.last() {
                        send_preedit(state.char);
                    } else {
                        con.text_input.send_preedit_string(None, 0, 0);
                    }
                    con.text_input.send_done();
                }
                FeedResult::Aborted => {
                    // nothing
                }
                FeedResult::Composed { string, keysym } => {
                    states.clear();
                    let s = if string.is_some() {
                        string
                    } else if let Some(sym) = keysym
                        && let Some(char) = sym.char()
                    {
                        Some(char.encode_utf8(&mut buf) as &str)
                    } else {
                        None
                    };
                    con.text_input.send_preedit_string(None, 0, 0);
                    con.text_input.send_commit_string(s);
                    con.text_input.send_done();
                }
            }
        }
        forward_to_node
    }

    fn on_modifiers(&self, _kb_state: &KeyboardState) -> bool {
        true
    }

    fn on_repeat_info(&self) {
        // nothing
    }
}
