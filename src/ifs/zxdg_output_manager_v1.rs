use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::zxdg_output_v1::ZxdgOutputV1,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zxdg_output_manager_v1::*, ZxdgOutputManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZxdgOutputManagerV1Global {
    name: GlobalName,
}

pub struct ZxdgOutputManagerV1 {
    pub id: ZxdgOutputManagerV1Id,
    pub client: Rc<Client>,
    pub version: u32,
    pub tracker: Tracker<Self>,
}

impl ZxdgOutputManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZxdgOutputManagerV1Id,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), ZxdgOutputManagerV1Error> {
        let obj = Rc::new(ZxdgOutputManagerV1 {
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

impl ZxdgOutputManagerV1 {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZxdgOutputManagerV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_xdg_output(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), ZxdgOutputManagerV1Error> {
        let req: GetXdgOutput = self.client.parse(&**self, parser)?;
        let output = self.client.lookup(req.output)?;
        let xdg_output = Rc::new(ZxdgOutputV1 {
            id: req.id,
            version: self.version,
            client: self.client.clone(),
            output: output.clone(),
            tracker: Default::default(),
        });
        track!(self.client, xdg_output);
        self.client.add_client_obj(&xdg_output)?;
        xdg_output.send_updates();
        output.xdg_outputs.set(req.id, xdg_output);
        Ok(())
    }
}

global_base!(
    ZxdgOutputManagerV1Global,
    ZxdgOutputManagerV1,
    ZxdgOutputManagerV1Error
);

impl Global for ZxdgOutputManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        3
    }
}

simple_add_global!(ZxdgOutputManagerV1Global);

object_base! {
    ZxdgOutputManagerV1;

    DESTROY => destroy,
    GET_XDG_OUTPUT => get_xdg_output,
}

simple_add_obj!(ZxdgOutputManagerV1);

impl Object for ZxdgOutputManagerV1 {
    fn num_requests(&self) -> u32 {
        GET_XDG_OUTPUT + 1
    }
}

#[derive(Debug, Error)]
pub enum ZxdgOutputManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(ZxdgOutputManagerV1Error, ClientError);
efrom!(ZxdgOutputManagerV1Error, MsgParserError);
