use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wp_cursor_shape_device_v1::{CursorShapeCursorUser, WpCursorShapeDeviceV1},
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_cursor_shape_manager_v1::*, WpCursorShapeDeviceV1Id, WpCursorShapeManagerV1Id},
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
        2
    }
}

pub struct WpCursorShapeManagerV1 {
    pub id: WpCursorShapeManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpCursorShapeManagerV1 {
    fn get(
        &self,
        id: WpCursorShapeDeviceV1Id,
        cursor_user: CursorShapeCursorUser,
    ) -> Result<(), WpCursorShapeManagerV1Error> {
        let device = Rc::new(WpCursorShapeDeviceV1 {
            id,
            client: self.client.clone(),
            cursor_user,
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, device);
        self.client.add_client_obj(&device)?;
        Ok(())
    }
}

impl WpCursorShapeManagerV1RequestHandler for WpCursorShapeManagerV1 {
    type Error = WpCursorShapeManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_pointer(&self, req: GetPointer, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pointer = self.client.lookup(req.pointer)?;
        self.get(
            req.cursor_shape_device,
            CursorShapeCursorUser::Seat(pointer.seat.global.clone()),
        )
    }

    fn get_tablet_tool_v2(&self, req: GetTabletToolV2, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let tool = self.client.lookup(req.tablet_tool)?;
        self.get(
            req.cursor_shape_device,
            CursorShapeCursorUser::TabletTool(tool.tool.clone()),
        )
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
}
efrom!(WpCursorShapeManagerV1Error, ClientError);
