use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::{
            ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
            wl_surface::{x_surface::xwindow::Xwindow, xdg_surface::xdg_toplevel::XdgToplevel},
        },
        leaks::Tracker,
        object::{Object, Version},
        tree::{NodeVisitorBase, ToplevelNode},
        utils::buffd::{MsgParser, MsgParserError},
        wire::{
            ext_foreign_toplevel_list_v1::*, ExtForeignToplevelHandleV1Id,
            ExtForeignToplevelListV1Id,
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtForeignToplevelListV1Global {
    pub name: GlobalName,
}

impl ExtForeignToplevelListV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ExtForeignToplevelListV1Id,
        client: &Rc<Client>,
        _version: Version,
    ) -> Result<(), ExtForeignToplevelListV1Error> {
        let obj = Rc::new(ExtForeignToplevelListV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        ToplevelVisitor { list: &obj }.visit_display(&client.state.root);
        client.state.toplevel_lists.set((client.id, id), obj);
        Ok(())
    }
}

struct ToplevelVisitor<'a> {
    list: &'a ExtForeignToplevelListV1,
}

impl NodeVisitorBase for ToplevelVisitor<'_> {
    fn visit_toplevel(&mut self, node: &Rc<XdgToplevel>) {
        node.send_to(self.list);
    }

    fn visit_xwindow(&mut self, node: &Rc<Xwindow>) {
        node.toplevel_data.send(node.clone(), self.list);
    }
}

pub struct ExtForeignToplevelListV1 {
    pub id: ExtForeignToplevelListV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
}

impl ExtForeignToplevelListV1 {
    fn detach(&self) {
        self.client
            .state
            .toplevel_lists
            .remove(&(self.client.id, self.id));
    }

    fn stop(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtForeignToplevelListV1Error> {
        let _req: Stop = self.client.parse(self, msg)?;
        self.detach();
        self.send_finished();
        Ok(())
    }

    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), ExtForeignToplevelListV1Error> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn send_finished(&self) {
        self.client.event(Finished { self_id: self.id })
    }

    fn send_handle(&self, handle: &ExtForeignToplevelHandleV1) {
        self.client.event(Toplevel {
            self_id: self.id,
            toplevel: handle.id,
        });
    }

    pub fn publish_toplevel(
        &self,
        tl: &Rc<dyn ToplevelNode>,
    ) -> Option<Rc<ExtForeignToplevelHandleV1>> {
        let id: ExtForeignToplevelHandleV1Id = match self.client.new_id() {
            Ok(i) => i,
            Err(e) => {
                self.client.error(e);
                return None;
            }
        };
        let handle = Rc::new(ExtForeignToplevelHandleV1 {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
            toplevel: tl.clone(),
        });
        track!(self.client, handle);
        self.client.add_server_obj(&handle);
        self.send_handle(&handle);
        Some(handle)
    }
}

global_base!(
    ExtForeignToplevelListV1Global,
    ExtForeignToplevelListV1,
    ExtForeignToplevelListV1Error
);

impl Global for ExtForeignToplevelListV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn secure(&self) -> bool {
        true
    }
}

simple_add_global!(ExtForeignToplevelListV1Global);

object_base! {
    self = ExtForeignToplevelListV1;

    STOP => stop,
    DESTROY => destroy,
}

impl Object for ExtForeignToplevelListV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ExtForeignToplevelListV1);

#[derive(Debug, Error)]
pub enum ExtForeignToplevelListV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtForeignToplevelListV1Error, MsgParserError);
efrom!(ExtForeignToplevelListV1Error, ClientError);
