use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::ipc::{
            zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1,
            zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1,
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwp_primary_selection_device_manager_v1::*, ZwpPrimarySelectionDeviceManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpPrimarySelectionDeviceManagerV1Global {
    name: GlobalName,
}

pub struct ZwpPrimarySelectionDeviceManagerV1 {
    pub id: ZwpPrimarySelectionDeviceManagerV1Id,
    pub client: Rc<Client>,
    pub version: u32,
    pub tracker: Tracker<Self>,
}

impl ZwpPrimarySelectionDeviceManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpPrimarySelectionDeviceManagerV1Id,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), ZwpPrimarySelectionDeviceManagerV1Error> {
        let obj = Rc::new(ZwpPrimarySelectionDeviceManagerV1 {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl ZwpPrimarySelectionDeviceManagerV1 {
    fn create_source(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpPrimarySelectionDeviceManagerV1Error> {
        let req: CreateSource = self.client.parse(self, parser)?;
        let res = Rc::new(ZwpPrimarySelectionSourceV1::new(req.id, &self.client));
        track!(self.client, res);
        self.client.add_client_obj(&res)?;
        Ok(())
    }

    fn get_data_device(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), ZwpPrimarySelectionDeviceManagerV1Error> {
        let req: GetDevice = self.client.parse(&**self, parser)?;
        let seat = self.client.lookup(req.seat)?;
        let dev = Rc::new(ZwpPrimarySelectionDeviceV1::new(req.id, self, &seat));
        track!(self.client, dev);
        seat.add_primary_selection_device(&dev);
        self.client.add_client_obj(&dev)?;
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwpPrimarySelectionDeviceManagerV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }
}

global_base!(
    ZwpPrimarySelectionDeviceManagerV1Global,
    ZwpPrimarySelectionDeviceManagerV1,
    ZwpPrimarySelectionDeviceManagerV1Error
);

impl Global for ZwpPrimarySelectionDeviceManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ZwpPrimarySelectionDeviceManagerV1Global);

object_base! {
    ZwpPrimarySelectionDeviceManagerV1;

    CREATE_SOURCE => create_source,
    GET_DEVICE => get_data_device,
    DESTROY => destroy,
}

impl Object for ZwpPrimarySelectionDeviceManagerV1 {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }
}

simple_add_obj!(ZwpPrimarySelectionDeviceManagerV1);

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionDeviceManagerV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpPrimarySelectionDeviceManagerV1Error, ClientError);
efrom!(ZwpPrimarySelectionDeviceManagerV1Error, MsgParserError);
