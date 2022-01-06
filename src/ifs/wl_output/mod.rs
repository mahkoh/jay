mod types;

use crate::backend::Output;
use crate::client::{AddObj, Client, ClientId, DynEventFormatter, WlEvent};
use crate::globals::{Global, GlobalName};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use crate::utils::copyhashmap::CopyHashMap;
use ahash::AHashMap;
use std::cell::Cell;
use std::iter;
use std::rc::Rc;
pub use types::*;

id!(WlOutputId);

const RELEASE: u32 = 0;

const GEOMETRY: u32 = 0;
const MODE: u32 = 1;
const DONE: u32 = 2;
const SCALE: u32 = 3;

const SP_UNKNOWN: i32 = 0;
const SP_NONE: i32 = 1;
const SP_HORIZONTAL_RGB: i32 = 2;
const SP_HORIZONTAL_BGR: i32 = 3;
const SP_VERTICAL_RGB: i32 = 4;
const SP_VERTICAL_BGR: i32 = 5;

const TF_NORMAL: i32 = 0;
const TF_90: i32 = 1;
const TF_180: i32 = 2;
const TF_270: i32 = 3;
const TF_FLIPPED: i32 = 4;
const TF_FLIPPED_90: i32 = 5;
const TF_FLIPPED_180: i32 = 6;
const TF_FLIPPED_270: i32 = 7;

const MODE_CURRENT: u32 = 1;
const MODE_PREFERRED: u32 = 2;

pub struct WlOutputGlobal {
    name: GlobalName,
    output: Rc<dyn Output>,
    width: Cell<u32>,
    height: Cell<u32>,
    bindings: CopyHashMap<(ClientId, WlOutputId), Rc<WlOutputObj>>,
}

impl WlOutputGlobal {
    pub fn new(name: GlobalName, output: &Rc<dyn Output>) -> Self {
        Self {
            name,
            output: output.clone(),
            width: Cell::new(output.width()),
            height: Cell::new(output.height()),
            bindings: Default::default(),
        }
    }

    pub async fn update_properties(&self) {
        let width = self.output.width();
        let height = self.output.height();

        let mut changed = false;
        changed |= self.width.replace(width) != width;
        changed |= self.height.replace(height) != height;

        if changed {
            let mut clients = AHashMap::new();
            {
                let bindings = self.bindings.lock();
                for binding in bindings.values() {
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
                        if binding.client.event2_locked(event) {
                            clients.insert(binding.client.id, binding.client.clone());
                        }
                    }
                }
            }
            for client in clients.values() {
                let _ = client.check_queue_size().await;
            }
        }
    }

    async fn bind_(
        self: Rc<Self>,
        id: WlOutputId,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), WlOutputError> {
        let obj = Rc::new(WlOutputObj {
            global: self.clone(),
            id,
            client: client.clone(),
        });
        client.add_client_obj(&obj)?;
        self.bindings.set((client.id, id), obj.clone());
        client.event(obj.geometry()).await?;
        client.event(obj.mode()).await?;
        client.event(obj.scale()).await?;
        client.event(obj.done()).await?;
        Ok(())
    }
}

bind!(WlOutputGlobal);

impl Global for WlOutputGlobal {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn interface(&self) -> Interface {
        Interface::WlOutput
    }

    fn version(&self) -> u32 {
        3
    }

    fn pre_remove(&self) {
        //
    }

    fn break_loops(&self) {
        self.bindings.clear();
    }
}

pub struct WlOutputObj {
    global: Rc<WlOutputGlobal>,
    id: WlOutputId,
    client: Rc<Client>,
}

impl WlOutputObj {
    fn geometry(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Geometry {
            obj: self.clone(),
            x: 0,
            y: 0,
            physical_width: self.global.width.get() as _,
            physical_height: self.global.height.get() as _,
            subpixel: SP_UNKNOWN,
            make: String::new(),
            model: String::new(),
            transform: TF_NORMAL,
        })
    }

    fn mode(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Mode {
            obj: self.clone(),
            flags: MODE_CURRENT,
            width: self.global.width.get() as _,
            height: self.global.height.get() as _,
            refresh: 60_000_000,
        })
    }

    fn scale(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Scale {
            obj: self.clone(),
            factor: 1,
        })
    }

    fn done(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Done { obj: self.clone() })
    }

    async fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.client.parse(self, parser)?;
        self.global.bindings.remove(&(self.client.id, self.id));
        self.client.remove_obj(self).await?;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlOutputError> {
        match request {
            RELEASE => self.release(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlOutputObj);

impl Object for WlOutputObj {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlOutput
    }

    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }

    fn break_loops(&self) {
        self.global.bindings.remove(&(self.client.id, self.id));
    }
}
