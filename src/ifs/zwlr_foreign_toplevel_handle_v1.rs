use {
    super::wl_output::WlOutput,
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        tree::{OutputNode, ToplevelOpt},
        wire::{ZwlrForeignToplevelHandleV1Id, zwlr_foreign_toplevel_handle_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

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

    fn activate(&self, _req: Activate, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(toplevel) = self.toplevel.get() {
            let seat = self.client.state.seat_queue.first();
            if let Some(seat) = seat {
                seat.focus_node(toplevel);
            }
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

    fn set_fullscreen(&self, _req: SetFullscreen, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(toplevel) = self.toplevel.get() {
            toplevel.tl_set_fullscreen(true);
        }
        Ok(())
    }

    fn unset_fullscreen(&self, _req: UnsetFullscreen, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(toplevel) = self.toplevel.get() {
            toplevel.tl_set_fullscreen(false);
        }
        Ok(())
    }
}

impl ZwlrForeignToplevelHandleV1 {
    pub fn handle_output_changed(&self, prev: Option<Rc<OutputNode>>, new: Rc<OutputNode>) {
        let prev_bindings = prev.clone().map(|o| o.global.bindings.borrow().clone());
        let new_bindings = new.global.bindings.borrow();
        let new_outputs: Option<Vec<&Rc<WlOutput>>> = new_bindings
            .get(&self.client.id)
            .map(|b| b.values().collect());
        if let Some(new_outputs) = new_outputs {
            if let Some(prev_bindings) = prev_bindings {
                let prev_outputs: Option<Vec<&Rc<WlOutput>>> = prev_bindings
                    .get(&self.client.id)
                    .map(|b| b.values().collect());
                if let Some(prev_outputs) = prev_outputs {
                    for output in prev_outputs.clone() {
                        if !new_outputs.iter().any(|o| o.id == output.id) {
                            self.send_output_leave(output.clone());
                        }
                    }

                    for output in new_outputs {
                        if !prev_outputs.iter().any(|o| o.id == output.id) {
                            self.send_output_enter(output.clone());
                        }
                    }
                }
            } else {
                for output in new_outputs {
                    self.send_output_enter(output.clone());
                }
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

    pub fn send_state(&self, maximized: bool, minimized: bool, activated: bool, fullscreen: bool) {
        let mut state: Vec<u32> = vec![];
        if maximized {
            state.push(0);
        }
        if minimized {
            state.push(1);
        }
        if activated {
            state.push(2);
        }
        if fullscreen {
            state.push(3);
        }
        self.client.event(State {
            self_id: self.id,
            state: &state,
        });
    }

    pub fn send_parent(&self, parent: Rc<ZwlrForeignToplevelHandleV1>) {
        self.client.event(Parent {
            self_id: self.id,
            parent: parent.id,
        });
    }

    pub fn send_output_enter(&self, output: Rc<WlOutput>) {
        self.client.event(OutputEnter {
            self_id: self.id,
            output: output.id,
        });
    }

    pub fn send_output_leave(&self, output: Rc<WlOutput>) {
        self.client.event(OutputLeave {
            self_id: self.id,
            output: output.id,
        });
    }

    pub fn send_set_fullscreen(&self) {
        self.client.event(SetFullscreen { self_id: self.id });
    }

    pub fn send_unset_fullscreen(&self) {
        self.client.event(UnsetFullscreen { self_id: self.id });
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

dedicated_add_obj!(
    ZwlrForeignToplevelHandleV1,
    ZwlrForeignToplevelHandleV1Id,
    zwlr_foreign_toplevel_handles
);

#[derive(Debug, Error)]
pub enum ZwlrForeignToplevelHandleV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwlrForeignToplevelHandleV1Error, ClientError);
