use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wp_cursor_shape_device_v1::WpCursorShapeDeviceV1,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
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
        version: u32,
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
    pub version: u32,
}

impl WpCursorShapeManagerV1 {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WpCursorShapeManagerV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_pointer(&self, parser: MsgParser<'_, '_>) -> Result<(), WpCursorShapeManagerV1Error> {
        let req: GetPointer = self.client.parse(self, parser)?;
        let pointer = self.client.lookup(req.pointer)?;
        let device = Rc::new(WpCursorShapeDeviceV1 {
            id: req.cursor_shape_device,
            client: self.client.clone(),
            seat: pointer.seat.global.clone(),
            tracker: Default::default(),
        });
        track!(self.client, device);
        self.client.add_client_obj(&device)?;
        Ok(())
    }

    fn get_tablet_tool_v2(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WpCursorShapeManagerV1Error> {
        let _req: GetTabletToolV2 = self.client.parse(self, parser)?;
        Err(WpCursorShapeManagerV1Error::TabletToolNotSupported)
    }
}

object_base! {
    self = WpCursorShapeManagerV1;

    DESTROY => destroy,
    GET_POINTER => get_pointer,
    GET_TABLET_TOOL_V2 => get_tablet_tool_v2,
}

impl Object for WpCursorShapeManagerV1 {}

simple_add_obj!(WpCursorShapeManagerV1);

#[derive(Debug, Error)]
pub enum WpCursorShapeManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("This compositor does not support tablet tools")]
    TabletToolNotSupported,
}
efrom!(WpCursorShapeManagerV1Error, ClientError);
efrom!(WpCursorShapeManagerV1Error, MsgParserError);
