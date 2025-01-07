use {
    crate::{
        backend::KeyState,
        ifs::wl_seat::WlSeatGlobal,
        keyboard::{DynKeyboardState, KeyboardState, KeyboardStateId},
        utils::{oserror::OsError, syncqueue::SyncQueue, vecset::VecSet},
    },
    kbvm::{
        lookup::LookupTable,
        state_machine::{self, Direction, Event, StateMachine},
        xkb::{
            self,
            diagnostic::{Diagnostic, WriteToLog},
            Keymap,
        },
        Keycode,
    },
    std::{
        cell::{Cell, Ref, RefCell},
        io::Write,
        rc::Rc,
    },
    thiserror::Error,
    uapi::{c, OwnedFd},
};

#[derive(Debug, Error)]
pub enum KbvmError {
    #[error("could not parse the keymap")]
    CouldNotParseKeymap(#[source] Diagnostic),
    #[error("Could not create a keymap memfd")]
    KeymapMemfd(#[source] OsError),
}

pub struct KbvmContext {
    pub ctx: xkb::Context,
    pub ids: KbvmMapIds,
}

impl Default for KbvmContext {
    fn default() -> Self {
        let mut ctx = xkb::Context::builder();
        ctx.enable_environment(true);
        Self {
            ctx: ctx.build(),
            ids: Default::default(),
        }
    }
}

linear_ids!(KbvmMapIds, KbvmMapId, u64);

pub struct KbvmMap {
    pub id: KbvmMapId,
    pub state_machine: StateMachine,
    pub lookup_table: LookupTable,
    pub map: Rc<OwnedFd>,
    pub map_len: usize,
}

pub struct KbvmState {
    pub map: Rc<KbvmMap>,
    pub state: state_machine::State,
    pub kb_state: KeyboardState,
}

pub struct PhysicalKeyboardState {
    state: Rc<RefCell<KbvmState>>,
    inner: RefCell<PkInner>,
    events: SyncQueue<Event>,
    flushing: Cell<bool>,
}

#[derive(Default)]
struct PkInner {
    pressed_keys: VecSet<u32>,
    event_stash: Vec<Event>,
}

impl DynKeyboardState for RefCell<KbvmState> {
    fn borrow(&self) -> Ref<'_, KeyboardState> {
        Ref::map(self.borrow(), |v| &v.kb_state)
    }
}

impl KbvmContext {
    pub fn parse_keymap(&self, keymap: &[u8]) -> Result<Rc<KbvmMap>, KbvmError> {
        let map = self
            .ctx
            .keymap_from_bytes(WriteToLog, None, keymap)
            .map_err(KbvmError::CouldNotParseKeymap)?;
        let (memfd, len) = create_keymap_memfd(&map).map_err(KbvmError::KeymapMemfd)?;
        let builder = map.to_builder();
        Ok(Rc::new(KbvmMap {
            id: self.ids.next(),
            state_machine: builder.build_state_machine(),
            map: Rc::new(memfd),
            map_len: len + 1,
            lookup_table: builder.build_lookup_table(),
        }))
    }
}

fn create_keymap_memfd(map: &Keymap) -> Result<(OwnedFd, usize), OsError> {
    let str = format!("{}\n", map.format());
    let mut memfd = uapi::memfd_create("keymap", c::MFD_CLOEXEC | c::MFD_ALLOW_SEALING)?;
    memfd.write_all(str.as_bytes())?;
    memfd.write_all(&[0])?;
    uapi::lseek(memfd.raw(), 0, c::SEEK_SET)?;
    uapi::fcntl_add_seals(
        memfd.raw(),
        c::F_SEAL_SEAL | c::F_SEAL_GROW | c::F_SEAL_SHRINK | c::F_SEAL_WRITE,
    )?;
    Ok((memfd, str.len()))
}

impl KbvmMap {
    pub fn state(self: &Rc<Self>, id: KeyboardStateId) -> KbvmState {
        KbvmState {
            map: self.clone(),
            state: self.state_machine.create_state(),
            kb_state: KeyboardState {
                id,
                map: self.map.clone(),
                map_len: self.map_len,
                pressed_keys: Default::default(),
                mods: Default::default(),
            },
        }
    }
}

impl KbvmState {
    pub fn apply_events(&mut self, events: &SyncQueue<Event>) {
        let state = &mut self.kb_state;
        while let Some(event) = events.pop() {
            state.mods.apply_event(event);
            match event {
                Event::KeyDown(kc) => {
                    state.pressed_keys.insert(kc.to_evdev());
                }
                Event::KeyUp(kc) => {
                    state.pressed_keys.remove(&kc.to_evdev());
                }
                _ => {}
            }
        }
    }
}

impl PhysicalKeyboardState {
    pub fn new(state: &Rc<RefCell<KbvmState>>) -> Self {
        Self {
            state: state.clone(),
            inner: Default::default(),
            events: Default::default(),
            flushing: Cell::new(false),
        }
    }

    fn flush(&self, time_usec: u64, seat: &Rc<WlSeatGlobal>) {
        if self.flushing.replace(true) {
            return;
        }
        seat.key_events(time_usec, &self.events, &self.state);
        self.flushing.set(false);
    }

    pub fn update(&self, time_usec: u64, seat: &Rc<WlSeatGlobal>, key: u32, key_state: KeyState) {
        {
            let inner = &mut *self.inner.borrow_mut();
            match key_state {
                KeyState::Released => {
                    if !inner.pressed_keys.remove(&key) {
                        return;
                    }
                }
                KeyState::Pressed => {
                    if !inner.pressed_keys.insert(key) {
                        return;
                    }
                }
            }
            let state = &mut *self.state.borrow_mut();
            state.map.state_machine.handle_key(
                &mut state.state,
                &mut inner.event_stash,
                Keycode::from_evdev(key),
                match key_state {
                    KeyState::Released => Direction::Up,
                    KeyState::Pressed => Direction::Down,
                },
            );
            self.events.append(&mut inner.event_stash);
        }
        self.flush(time_usec, seat);
    }

    pub fn destroy(&self, time_usec: u64, seat: &Rc<WlSeatGlobal>) {
        {
            let inner = &mut *self.inner.borrow_mut();
            let state = &mut *self.state.borrow_mut();
            let sm = &state.map.state_machine;
            while let Some(key) = inner.pressed_keys.pop() {
                sm.handle_key(
                    &mut state.state,
                    &mut inner.event_stash,
                    Keycode::from_evdev(key),
                    Direction::Up,
                );
            }
            self.events.append(&mut inner.event_stash);
        }
        self.flush(time_usec, seat);
    }
}
