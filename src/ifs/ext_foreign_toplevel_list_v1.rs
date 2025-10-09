use {
    crate::{
        client::{CAP_FOREIGN_TOPLEVEL_LIST, Client, ClientCaps, ClientError},
        globals::{Global, GlobalName},
        ifs::{
            ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
            wl_surface::{x_surface::xwindow::Xwindow, xdg_surface::xdg_toplevel::XdgToplevel},
        },
        leaks::Tracker,
        object::{Object, Version},
        tree::{NodeVisitorBase, ToplevelOpt},
        wire::{
            ExtForeignToplevelHandleV1Id, ExtForeignToplevelListV1Id,
            ext_foreign_toplevel_list_v1::*,
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
        version: Version,
    ) -> Result<(), ExtForeignToplevelListV1Error> {
        let obj = Rc::new(ExtForeignToplevelListV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
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
    pub version: Version,
}

impl ExtForeignToplevelListV1 {
    fn detach(&self) {
        self.client
            .state
            .toplevel_lists
            .remove(&(self.client.id, self.id));
    }
}

impl ExtForeignToplevelListV1RequestHandler for ExtForeignToplevelListV1 {
    type Error = ExtForeignToplevelListV1Error;

    fn stop(&self, _req: Stop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.send_finished();
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

impl ExtForeignToplevelListV1 {
    fn send_finished(&self) {
        self.client.event(Finished { self_id: self.id })
    }

    fn send_handle(&self, handle: &ExtForeignToplevelHandleV1) {
        self.client.event(Toplevel {
            self_id: self.id,
            toplevel: handle.id,
        });
    }

    pub fn publish_toplevel(&self, tl: ToplevelOpt) -> Option<Rc<ExtForeignToplevelHandleV1>> {
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
            toplevel: tl,
            version: self.version,
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

    fn required_caps(&self) -> ClientCaps {
        CAP_FOREIGN_TOPLEVEL_LIST
    }
}

simple_add_global!(ExtForeignToplevelListV1Global);

object_base! {
    self = ExtForeignToplevelListV1;
    version = self.version;
}

impl Object for ExtForeignToplevelListV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}

simple_add_obj!(ExtForeignToplevelListV1);

#[derive(Debug, Error)]
pub enum ExtForeignToplevelListV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtForeignToplevelListV1Error, ClientError);
