use {
    crate::{
        backend,
        client::{Client, ClientError, ClientId},
        format::XRGB8888,
        globals::{Global, GlobalName},
        ifs::{
            wl_buffer::WlBufferStorage, wl_surface::WlSurface,
            zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, zxdg_output_v1::ZxdgOutputV1,
        },
        leaks::Tracker,
        object::Object,
        rect::Rect,
        render::{Framebuffer, Texture},
        state::{ConnectorData, State},
        time::Time,
        tree::OutputNode,
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
            copyhashmap::CopyHashMap,
            linkedlist::LinkedList,
        },
        wire::{wl_output::*, WlOutputId, ZxdgOutputV1Id},
    },
    ahash::AHashMap,
    std::{
        cell::{Cell, RefCell},
        collections::hash_map::Entry,
        ops::Deref,
        rc::Rc,
    },
    thiserror::Error,
};

const SP_UNKNOWN: i32 = 0;
#[allow(dead_code)]
const SP_NONE: i32 = 1;
#[allow(dead_code)]
const SP_HORIZONTAL_RGB: i32 = 2;
#[allow(dead_code)]
const SP_HORIZONTAL_BGR: i32 = 3;
#[allow(dead_code)]
const SP_VERTICAL_RGB: i32 = 4;
#[allow(dead_code)]
const SP_VERTICAL_BGR: i32 = 5;

pub const TF_NORMAL: i32 = 0;
pub const TF_90: i32 = 1;
pub const TF_180: i32 = 2;
pub const TF_270: i32 = 3;
pub const TF_FLIPPED: i32 = 4;
pub const TF_FLIPPED_90: i32 = 5;
pub const TF_FLIPPED_180: i32 = 6;
pub const TF_FLIPPED_270: i32 = 7;

const MODE_CURRENT: u32 = 1;
#[allow(dead_code)]
const MODE_PREFERRED: u32 = 2;

pub struct WlOutputGlobal {
    pub name: GlobalName,
    pub state: Rc<State>,
    pub connector: Rc<ConnectorData>,
    pub pos: Cell<Rect>,
    pub output_id: Rc<OutputId>,
    pub mode: Cell<backend::Mode>,
    pub node: CloneCell<Option<Rc<OutputNode>>>,
    pub width_mm: i32,
    pub height_mm: i32,
    pub bindings: RefCell<AHashMap<ClientId, AHashMap<WlOutputId, Rc<WlOutput>>>>,
    pub unused_captures: LinkedList<Rc<ZwlrScreencopyFrameV1>>,
    pub pending_captures: LinkedList<Rc<ZwlrScreencopyFrameV1>>,
    pub destroyed: Cell<bool>,
    pub legacy_scale: Cell<i32>,
}

#[derive(Eq, PartialEq)]
pub struct OutputId {
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
}

impl WlOutputGlobal {
    pub fn clear(&self) {
        self.node.take();
        self.bindings.borrow_mut().clear();
    }

    pub fn new(
        name: GlobalName,
        state: &Rc<State>,
        connector: &Rc<ConnectorData>,
        x1: i32,
        mode: &backend::Mode,
        manufacturer: &str,
        product: &str,
        serial_number: &str,
        width_mm: i32,
        height_mm: i32,
    ) -> Self {
        Self {
            name,
            state: state.clone(),
            connector: connector.clone(),
            pos: Cell::new(Rect::new_sized(x1, 0, mode.width, mode.height).unwrap()),
            output_id: Rc::new(OutputId {
                manufacturer: manufacturer.to_string(),
                model: product.to_string(),
                serial_number: serial_number.to_string(),
            }),
            mode: Cell::new(*mode),
            node: Default::default(),
            width_mm,
            height_mm,
            bindings: Default::default(),
            unused_captures: Default::default(),
            pending_captures: Default::default(),
            destroyed: Cell::new(false),
            legacy_scale: Cell::new(1),
        }
    }

    pub fn position(&self) -> Rect {
        self.pos.get()
    }

    pub fn for_each_binding<F: FnMut(&Rc<WlOutput>)>(&self, client: ClientId, mut f: F) {
        let bindings = self.bindings.borrow_mut();
        if let Some(bindings) = bindings.get(&client) {
            for binding in bindings.values() {
                f(binding);
            }
        }
    }

    pub fn send_enter(&self, surface: &WlSurface) {
        self.for_each_binding(surface.client.id, |b| {
            surface.send_enter(b.id);
        })
    }

    pub fn send_leave(&self, surface: &WlSurface) {
        self.for_each_binding(surface.client.id, |b| {
            surface.send_leave(b.id);
        })
    }

    pub fn send_mode(&self) {
        let bindings = self.bindings.borrow_mut();
        for binding in bindings.values() {
            for binding in binding.values() {
                binding.send_geometry();
                binding.send_mode();
                binding.send_scale();
                binding.send_done();
                let xdg = binding.xdg_outputs.lock();
                for xdg in xdg.values() {
                    xdg.send_updates();
                }
                // binding.client.flush();
            }
        }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlOutputId,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), WlOutputError> {
        let obj = Rc::new(WlOutput {
            global: self.clone(),
            id,
            xdg_outputs: Default::default(),
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        self.bindings
            .borrow_mut()
            .entry(client.id)
            .or_default()
            .insert(id, obj.clone());
        obj.send_geometry();
        obj.send_mode();
        if obj.version >= SEND_SCALE_SINCE {
            obj.send_scale();
        }
        if obj.version >= SEND_DONE_SINCE {
            obj.send_done();
        }
        Ok(())
    }

    pub fn perform_screencopies(&self, fb: &Framebuffer, tex: &Texture) {
        if self.pending_captures.is_empty() {
            return;
        }
        let now = Time::now().unwrap();
        let mut captures = vec![];
        for capture in self.pending_captures.iter() {
            captures.push(capture.deref().clone());
            let wl_buffer = match capture.buffer.take() {
                Some(b) => b,
                _ => {
                    log::warn!("Capture frame is pending but has no buffer attached");
                    capture.send_failed();
                    continue;
                }
            };
            if wl_buffer.destroyed() {
                capture.send_failed();
                continue;
            }
            let rect = capture.rect;
            if let Some(WlBufferStorage::Shm { mem, .. }) = wl_buffer.storage.borrow_mut().deref() {
                let res = mem.access(|mem| {
                    fb.copy_to_shm(
                        rect.x1(),
                        rect.y1(),
                        rect.width(),
                        rect.height(),
                        XRGB8888,
                        mem,
                    );
                });
                if let Err(e) = res {
                    capture.client.error(e);
                }
                // capture.send_flags(FLAGS_Y_INVERT);
            } else {
                let fb = match wl_buffer.famebuffer.get() {
                    Some(fb) => fb,
                    _ => {
                        log::warn!("Capture buffer has no framebuffer");
                        capture.send_failed();
                        continue;
                    }
                };
                fb.copy_texture(&self.state, tex, -capture.rect.x1(), -capture.rect.y1());
            }
            if capture.with_damage.get() {
                capture.send_damage();
            }
            capture.send_ready(now.0.tv_sec as _, now.0.tv_nsec as _);
        }
        for capture in captures {
            capture.output_link.take();
        }
    }
}

global_base!(WlOutputGlobal, WlOutput, WlOutputError);

impl Global for WlOutputGlobal {
    fn singleton(&self) -> bool {
        false
    }

    fn version(&self) -> u32 {
        3
    }

    fn break_loops(&self) {
        self.bindings.borrow_mut().clear();
    }
}

dedicated_add_global!(WlOutputGlobal, outputs);

pub struct WlOutput {
    pub global: Rc<WlOutputGlobal>,
    pub id: WlOutputId,
    pub xdg_outputs: CopyHashMap<ZxdgOutputV1Id, Rc<ZxdgOutputV1>>,
    client: Rc<Client>,
    pub version: u32,
    tracker: Tracker<Self>,
}

pub const SEND_DONE_SINCE: u32 = 2;
pub const SEND_SCALE_SINCE: u32 = 2;

impl WlOutput {
    fn send_geometry(&self) {
        let pos = self.global.pos.get();
        let event = Geometry {
            self_id: self.id,
            x: pos.x1(),
            y: pos.y1(),
            physical_width: self.global.width_mm,
            physical_height: self.global.height_mm,
            subpixel: SP_UNKNOWN,
            make: &self.global.output_id.manufacturer,
            model: &self.global.output_id.model,
            transform: TF_NORMAL,
        };
        self.client.event(event);
    }

    fn send_mode(&self) {
        let mode = self.global.mode.get();
        let event = Mode {
            self_id: self.id,
            flags: MODE_CURRENT,
            width: mode.width,
            height: mode.height,
            refresh: mode.refresh_rate_millihz as _,
        };
        self.client.event(event);
    }

    fn send_scale(self: &Rc<Self>) {
        let event = Scale {
            self_id: self.id,
            factor: self.global.legacy_scale.get(),
        };
        self.client.event(event);
    }

    pub fn send_done(&self) {
        let event = Done { self_id: self.id };
        self.client.event(event);
    }

    fn remove_binding(&self) {
        if let Entry::Occupied(mut e) = self.global.bindings.borrow_mut().entry(self.client.id) {
            e.get_mut().remove(&self.id);
            if e.get().is_empty() {
                e.remove();
            }
        }
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), WlOutputError> {
        let _req: Release = self.client.parse(self, parser)?;
        self.xdg_outputs.clear();
        self.remove_binding();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    WlOutput;

    RELEASE => release,
}

impl Object for WlOutput {
    fn num_requests(&self) -> u32 {
        if self.version < 3 {
            0
        } else {
            RELEASE + 1
        }
    }

    fn break_loops(&self) {
        self.xdg_outputs.clear();
        self.remove_binding();
    }
}

dedicated_add_obj!(WlOutput, WlOutputId, outputs);

#[derive(Debug, Error)]
pub enum WlOutputError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(WlOutputError, ClientError);
efrom!(WlOutputError, MsgParserError);
