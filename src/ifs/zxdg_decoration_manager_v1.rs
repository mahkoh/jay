use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
        leaks::Tracker,
        object::{Object, Version},
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zxdg_decoration_manager_v1::*, ZxdgDecorationManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZxdgDecorationManagerV1Global {
    name: GlobalName,
}

impl ZxdgDecorationManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZxdgDecorationManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZxdgDecorationManagerV1Error> {
        let obj = Rc::new(ZxdgDecorationManagerV1 {
            id,
            client: client.clone(),
            _version: version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(
    ZxdgDecorationManagerV1Global,
    ZxdgDecorationManagerV1,
    ZxdgDecorationManagerV1Error
);

impl Global for ZxdgDecorationManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ZxdgDecorationManagerV1Global);

pub struct ZxdgDecorationManagerV1 {
    id: ZxdgDecorationManagerV1Id,
    client: Rc<Client>,
    _version: Version,
    tracker: Tracker<Self>,
}

impl ZxdgDecorationManagerV1 {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZxdgDecorationManagerV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_toplevel_decoration(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZxdgDecorationManagerV1Error> {
        let req: GetToplevelDecoration = self.client.parse(self, parser)?;
        let tl = self.client.lookup(req.toplevel)?;
        let obj = Rc::new(ZxdgToplevelDecorationV1::new(req.id, &self.client, &tl));
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.do_send_configure();
        Ok(())
    }
}

object_base! {
    self = ZxdgDecorationManagerV1;

    DESTROY => destroy,
    GET_TOPLEVEL_DECORATION => get_toplevel_decoration,
}

impl Object for ZxdgDecorationManagerV1 {}

simple_add_obj!(ZxdgDecorationManagerV1);

#[derive(Debug, Error)]
pub enum ZxdgDecorationManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(ZxdgDecorationManagerV1Error, ClientError);
efrom!(ZxdgDecorationManagerV1Error, MsgParserError);
