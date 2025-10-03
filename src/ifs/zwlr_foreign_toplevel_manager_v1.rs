use {
    crate::{
        client::{CAP_FOREIGN_TOPLEVEL_MANAGER, Client, ClientCaps, ClientError},
        globals::{Global, GlobalName},
        ifs::{
            wl_surface::{x_surface::xwindow::Xwindow, xdg_surface::xdg_toplevel::XdgToplevel},
            zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        tree::{NodeVisitorBase, ToplevelOpt},
        wire::{ZwlrForeignToplevelManagerV1Id, zwlr_foreign_toplevel_manager_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwlrForeignToplevelManagerV1Global {
    name: GlobalName,
}

impl ZwlrForeignToplevelManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwlrForeignToplevelManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZwlrForeignToplevelManagerV1Error> {
        let obj = Rc::new(ZwlrForeignToplevelManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        ZwlrToplevelVisitor { manager: &obj }.visit_display(&client.state.root);
        client.state.toplevel_managers.set((client.id, id), obj);
        Ok(())
    }
}

struct ZwlrToplevelVisitor<'a> {
    manager: &'a ZwlrForeignToplevelManagerV1,
}

impl NodeVisitorBase for ZwlrToplevelVisitor<'_> {
    fn visit_toplevel(&mut self, node: &Rc<XdgToplevel>) {
        node.manager_send_to(self.manager);
    }

    fn visit_xwindow(&mut self, node: &Rc<Xwindow>) {
        node.toplevel_data.manager_send(node.clone(), self.manager);
    }
}

pub struct ZwlrForeignToplevelManagerV1 {
    pub id: ZwlrForeignToplevelManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwlrForeignToplevelManagerV1 {
    pub fn detach(&self) {
        self.client
            .state
            .toplevel_managers
            .remove(&(self.client.id, self.id));
    }
}

impl ZwlrForeignToplevelManagerV1RequestHandler for ZwlrForeignToplevelManagerV1 {
    type Error = ZwlrForeignToplevelManagerV1Error;

    fn stop(&self, _req: Stop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.send_finished();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

impl ZwlrForeignToplevelManagerV1 {
    pub fn send_finished(&self) {
        self.client.event(Finished { self_id: self.id });
    }

    pub fn send_handle(&self, handle: &ZwlrForeignToplevelHandleV1) {
        self.client.event(Toplevel {
            self_id: self.id,
            toplevel: handle.id,
        });
    }

    pub fn publish_toplevel(&self, tl: ToplevelOpt) -> Option<Rc<ZwlrForeignToplevelHandleV1>> {
        let id = match self.client.new_id() {
            Ok(id) => id,
            Err(e) => {
                self.client.error(e);
                return None;
            }
        };
        let handle = Rc::new(ZwlrForeignToplevelHandleV1 {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            toplevel: tl,
        });
        track!(self.client, handle);
        self.client.add_server_obj(&handle);
        self.send_handle(&handle);
        Some(handle)
    }
}

global_base!(
    ZwlrForeignToplevelManagerV1Global,
    ZwlrForeignToplevelManagerV1,
    ZwlrForeignToplevelManagerV1Error
);

impl Global for ZwlrForeignToplevelManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        3
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_FOREIGN_TOPLEVEL_MANAGER
    }
}

simple_add_global!(ZwlrForeignToplevelManagerV1Global);

object_base! {
    self = ZwlrForeignToplevelManagerV1;
    version = self.version;
}

impl Object for ZwlrForeignToplevelManagerV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}

simple_add_obj!(ZwlrForeignToplevelManagerV1);

#[derive(Debug, Error)]
pub enum ZwlrForeignToplevelManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwlrForeignToplevelManagerV1Error, ClientError);
