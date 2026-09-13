use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_seat::zwp_relative_pointer_v1::ZwpRelativePointerV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::ZwpRelativePointerManagerV1Id;
use crate::wire::zwp_relative_pointer_manager_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

pub struct ZwpRelativePointerManagerV1Global {
    name: GlobalName,
}

#[derive(Object)]
pub struct ZwpRelativePointerManagerV1 {
    id: ZwpRelativePointerManagerV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl ZwpRelativePointerManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpRelativePointerManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(ZwpRelativePointerManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

global_base!(
    ZwpRelativePointerManagerV1Global,
    ZwpRelativePointerManagerV1,
);

impl Global for ZwpRelativePointerManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ZwpRelativePointerManagerV1Global);

impl ZwpRelativePointerManagerV1RequestHandler for ZwpRelativePointerManagerV1 {
    type Error = LookupError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn get_relative_pointer(
        &self,
        req: GetRelativePointer,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let pointer = self.client.lookup(req.pointer)?;
        let rp = Rc::new(ZwpRelativePointerV1 {
            id: req.id,
            client: self.client.clone(),
            seat: pointer.seat.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, rp);
        self.client.add_client_obj(&rp);
        pointer.seat.relative_pointers.set(req.id, rp);
        Ok(())
    }
}
