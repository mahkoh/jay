use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_seat::zwp_relative_pointer_v1::ZwpRelativePointerV1,
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwpRelativePointerManagerV1Id, zwp_relative_pointer_manager_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpRelativePointerManagerV1Global {
    pub name: GlobalName,
}

pub struct ZwpRelativePointerManagerV1 {
    pub id: ZwpRelativePointerManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
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
    ) -> Result<(), ZwpRelativePointerManagerV1Error> {
        let obj = Rc::new(ZwpRelativePointerManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(
    ZwpRelativePointerManagerV1Global,
    ZwpRelativePointerManagerV1,
    ZwpRelativePointerManagerV1Error
);

impl Global for ZwpRelativePointerManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ZwpRelativePointerManagerV1Global);

impl ZwpRelativePointerManagerV1RequestHandler for ZwpRelativePointerManagerV1 {
    type Error = ZwpRelativePointerManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
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
        self.client.add_client_obj(&rp)?;
        pointer.seat.relative_pointers.set(req.id, rp);
        Ok(())
    }
}

object_base! {
    self = ZwpRelativePointerManagerV1;
    version = self.version;
}

impl Object for ZwpRelativePointerManagerV1 {}

simple_add_obj!(ZwpRelativePointerManagerV1);

#[derive(Debug, Error)]
pub enum ZwpRelativePointerManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpRelativePointerManagerV1Error, ClientError);
