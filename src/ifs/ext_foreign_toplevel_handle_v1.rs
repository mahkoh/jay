use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::Object,
        tree::ToplevelNode,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{ext_foreign_toplevel_handle_v1::*, ExtForeignToplevelHandleV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtForeignToplevelHandleV1 {
    pub id: ExtForeignToplevelHandleV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub toplevel: Rc<dyn ToplevelNode>,
}

impl ExtForeignToplevelHandleV1 {
    fn detach(&self) {
        self.toplevel
            .tl_data()
            .handles
            .remove(&(self.client.id, self.id));
    }

    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtSessionLockV1Error> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    pub fn send_closed(&self) {
        self.client.event(Closed { self_id: self.id });
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_title(&self, title: &str) {
        self.client.event(Title {
            self_id: self.id,
            title,
        });
    }

    pub fn send_app_id(&self, app_id: &str) {
        self.client.event(AppId {
            self_id: self.id,
            app_id,
        });
    }

    pub fn send_identifier(&self, identifier: &str) {
        self.client.event(Identifier {
            self_id: self.id,
            identifier,
        });
    }
}

object_base! {
    self = ExtForeignToplevelHandleV1;

    DESTROY => destroy,
}

impl Object for ExtForeignToplevelHandleV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ExtForeignToplevelHandleV1);

#[derive(Debug, Error)]
pub enum ExtSessionLockV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtSessionLockV1Error, MsgParserError);
efrom!(ExtSessionLockV1Error, ClientError);
