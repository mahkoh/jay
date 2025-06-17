use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_output::WlOutput,
        leaks::Tracker,
        object::{Object, Version},
        tree::{Direction, OutputNode, ToplevelOpt},
        wire::{ZwlrForeignToplevelHandleV1Id, zwlr_foreign_toplevel_handle_v1::*},
    },
    arrayvec::ArrayVec,
    std::rc::Rc,
    thiserror::Error,
};

const STATE_ACTIVATED: u32 = 2;
const STATE_FULLSCREEN: u32 = 3;

const FULLSCREEN_SINCE: Version = Version(2);

pub struct ZwlrForeignToplevelHandleV1 {
    pub id: ZwlrForeignToplevelHandleV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub toplevel: ToplevelOpt,
    pub version: Version,
}

impl ZwlrForeignToplevelHandleV1 {
    fn detach(&self) {
        if let Some(tl) = self.toplevel.get() {
            tl.tl_data()
                .manager_handles
                .remove(&(self.client.id, self.id));
        }
    }
}

impl ZwlrForeignToplevelHandleV1RequestHandler for ZwlrForeignToplevelHandleV1 {
    type Error = ZwlrForeignToplevelHandleV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_maximized(&self, _req: SetMaximized, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn unset_maximized(&self, _req: UnsetMaximized, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_minimized(&self, _req: SetMinimized, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn unset_minimized(&self, _req: UnsetMinimized, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn activate(&self, req: Activate, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(toplevel) = self.toplevel.get() {
            if !toplevel.node_visible() {
                return Ok(());
            }
            let seat = self.client.lookup(req.seat)?;
            toplevel.tl_restack();
            toplevel.node_do_focus(&seat.global, Direction::Unspecified);
        }
        Ok(())
    }

    fn close(&self, _req: Close, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(toplevel) = self.toplevel.get() {
            toplevel.tl_close();
        }
        Ok(())
    }

    fn set_rectangle(&self, _req: SetRectangle, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_fullscreen(&self, req: SetFullscreen, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(toplevel) = self.toplevel.get() {
            let ws = if req.output.is_some() {
                self.client
                    .lookup(req.output)?
                    .global
                    .node()
                    .map(|node| node.ensure_workspace())
            } else {
                None
            };
            toplevel.tl_set_fullscreen(true, ws);
        }
        Ok(())
    }

    fn unset_fullscreen(&self, _req: UnsetFullscreen, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(toplevel) = self.toplevel.get() {
            toplevel.tl_set_fullscreen(false, None);
        }
        Ok(())
    }
}

impl ZwlrForeignToplevelHandleV1 {
    pub fn leave_output(&self, output: &Rc<OutputNode>) {
        let bindings = output.global.bindings.borrow();
        if let Some(bindings) = bindings.get(&self.client.id) {
            for binding in bindings.values() {
                self.send_output_leave(binding);
            }
        }
    }

    pub fn enter_output(&self, output: &Rc<OutputNode>) {
        let bindings = output.global.bindings.borrow();
        if let Some(bindings) = bindings.get(&self.client.id) {
            for binding in bindings.values() {
                self.send_output_enter(binding);
            }
        }
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

    pub fn send_state(&self, activated: bool, fullscreen: bool) {
        let mut state: ArrayVec<u32, 2> = ArrayVec::new();
        if activated {
            state.push(STATE_ACTIVATED);
        }
        if self.version >= FULLSCREEN_SINCE {
            if fullscreen {
                state.push(STATE_FULLSCREEN);
            }
        }
        self.client.event(State {
            self_id: self.id,
            state: &state,
        });
    }

    #[expect(dead_code)]
    pub fn send_parent(&self, parent: &Rc<ZwlrForeignToplevelHandleV1>) {
        self.client.event(Parent {
            self_id: self.id,
            parent: parent.id,
        });
    }

    pub fn send_output_enter(&self, output: &Rc<WlOutput>) {
        self.client.event(OutputEnter {
            self_id: self.id,
            output: output.id,
        });
    }

    pub fn send_output_leave(&self, output: &Rc<WlOutput>) {
        self.client.event(OutputLeave {
            self_id: self.id,
            output: output.id,
        });
    }
}

object_base! {
    self = ZwlrForeignToplevelHandleV1;
    version = self.version;
}

impl Object for ZwlrForeignToplevelHandleV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwlrForeignToplevelHandleV1);

#[derive(Debug, Error)]
pub enum ZwlrForeignToplevelHandleV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwlrForeignToplevelHandleV1Error, ClientError);
