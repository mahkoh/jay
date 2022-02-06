
use crate::backend::Output;
use crate::client::{Client, ClientError, ClientId, DynEventFormatter, WlEvent};
use crate::globals::{Global, GlobalName};
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use ahash::AHashMap;
use std::cell::{Cell, RefCell};
use std::collections::hash_map::Entry;
use std::iter;
use std::rc::Rc;
use thiserror::Error;
use crate::wire::wl_output::*;
use crate::utils::buffd::MsgParserError;
use crate::wire::WlOutputId;

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

const TF_NORMAL: i32 = 0;
#[allow(dead_code)]
const TF_90: i32 = 1;
#[allow(dead_code)]
const TF_180: i32 = 2;
#[allow(dead_code)]
const TF_270: i32 = 3;
#[allow(dead_code)]
const TF_FLIPPED: i32 = 4;
#[allow(dead_code)]
const TF_FLIPPED_90: i32 = 5;
#[allow(dead_code)]
const TF_FLIPPED_180: i32 = 6;
#[allow(dead_code)]
const TF_FLIPPED_270: i32 = 7;

const MODE_CURRENT: u32 = 1;
#[allow(dead_code)]
const MODE_PREFERRED: u32 = 2;

pub struct WlOutputGlobal {
    name: GlobalName,
    output: Rc<dyn Output>,
    pub x: Cell<i32>,
    pub y: Cell<i32>,
    width: Cell<i32>,
    height: Cell<i32>,
    pub bindings: RefCell<AHashMap<ClientId, AHashMap<WlOutputId, Rc<WlOutput>>>>,
}

impl WlOutputGlobal {
    pub fn new(name: GlobalName, output: &Rc<dyn Output>) -> Self {
        Self {
            name,
            output: output.clone(),
            x: Cell::new(0),
            y: Cell::new(0),
            width: Cell::new(output.width()),
            height: Cell::new(output.height()),
            bindings: Default::default(),
        }
    }

    pub fn update_properties(&self) {
        let width = self.output.width();
        let height = self.output.height();

        let mut changed = false;
        changed |= self.width.replace(width) != width;
        changed |= self.height.replace(height) != height;

        if changed {
            let bindings = self.bindings.borrow_mut();
            for binding in bindings.values() {
                for binding in binding.values() {
                    let events = [
                        binding.geometry(),
                        binding.mode(),
                        binding.scale(),
                        binding.done(),
                    ];
                    let events = events
                        .into_iter()
                        .map(|e| WlEvent::Event(e))
                        .chain(iter::once(WlEvent::Flush));
                    for event in events {
                        binding.client.event2(event);
                    }
                }
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
            client: client.clone(),
            version,
        });
        client.add_client_obj(&obj)?;
        self.bindings
            .borrow_mut()
            .entry(client.id)
            .or_default()
            .insert(id, obj.clone());
        client.event(obj.geometry());
        client.event(obj.mode());
        if obj.send_scale() {
            client.event(obj.scale());
        }
        if obj.send_done() {
            client.event(obj.done());
        }
        Ok(())
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
    global: Rc<WlOutputGlobal>,
    pub id: WlOutputId,
    client: Rc<Client>,
    version: u32,
}

impl WlOutput {
    fn send_done(&self) -> bool {
        self.version >= 2
    }

    fn send_scale(&self) -> bool {
        self.version >= 2
    }

    fn geometry(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(GeometryOut {
            self_id: self.id,
            x: 0,
            y: 0,
            physical_width: self.global.width.get() as _,
            physical_height: self.global.height.get() as _,
            subpixel: SP_UNKNOWN,
            make: "i4".to_string(),
            model: "i4".to_string(),
            transform: TF_NORMAL,
        })
    }

    fn mode(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Mode {
            self_id: self.id,
            flags: MODE_CURRENT,
            width: self.global.width.get() as _,
            height: self.global.height.get() as _,
            refresh: 60_000_000,
        })
    }

    fn scale(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Scale {
            self_id: self.id,
            factor: 1,
        })
    }

    fn done(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Done { self_id: self.id })
    }

    fn remove_binding(&self) {
        if let Entry::Occupied(mut e) = self.global.bindings.borrow_mut().entry(self.client.id) {
            e.get_mut().remove(&self.id);
            if e.get().is_empty() {
                e.remove();
            }
        }
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.client.parse(self, parser)?;
        self.remove_binding();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    WlOutput, WlOutputError;

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
        self.remove_binding();
    }
}

simple_add_obj!(WlOutput);

#[derive(Debug, Error)]
pub enum WlOutputError {
    #[error("Could not handle `release` request")]
    ReleaseError(#[from] ReleaseError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlOutputError, ClientError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ClientError);
efrom!(ReleaseError, ParseError, MsgParserError);
