use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wp_cursor_shape_device_v1::WpCursorShapeDeviceV1,
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_cursor_shape_manager_v1::*, WpCursorShapeManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpCursorShapeManagerV1Global {
    pub name: GlobalName,
}

impl WpCursorShapeManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpCursorShapeManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpCursorShapeManagerV1Error> {
        let mgr = Rc::new(WpCursorShapeManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, mgr);
        client.add_client_obj(&mgr)?;
        Ok(())
    }
}

global_base!(
    WpCursorShapeManagerV1Global,
    WpCursorShapeManagerV1,
    WpCursorShapeManagerV1Error
);

simple_add_global!(WpCursorShapeManagerV1Global);

impl Global for WpCursorShapeManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

pub struct WpCursorShapeManagerV1 {
    pub id: WpCursorShapeManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpCursorShapeManagerV1RequestHandler for WpCursorShapeManagerV1 {
    type Error = WpCursorShapeManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_pointer(&self, req: GetPointer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pointer = self.client.lookup(req.pointer)?;
        let device = Rc::new(WpCursorShapeDeviceV1 {
            id: req.cursor_shape_device,
            client: self.client.clone(),
            seat: pointer.seat.global.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, device);
        self.client.add_client_obj(&device)?;
        Ok(())
    }

    fn get_tablet_tool_v2(
        &self,
        _req: GetTabletToolV2,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Err(WpCursorShapeManagerV1Error::TabletToolNotSupported)
    }
}

object_base! {
    self = WpCursorShapeManagerV1;
    version = self.version;
}

impl Object for WpCursorShapeManagerV1 {}

simple_add_obj!(WpCursorShapeManagerV1);

#[derive(Debug, Error)]
pub enum WpCursorShapeManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("This compositor does not support tablet tools")]
    TabletToolNotSupported,
}
efrom!(WpCursorShapeManagerV1Error, ClientError);
