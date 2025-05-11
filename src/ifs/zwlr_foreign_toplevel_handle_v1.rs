use {
    super::wl_output::WlOutput,
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        tree::{OutputNode, ToplevelOpt},
        wire::{ZwlrForeignToplevelHandleV1Id, zwlr_foreign_toplevel_handle_v1::*},
    },
    arrayvec::ArrayVec,
    std::rc::Rc,
    thiserror::Error,
};

const STATE_ACTIVATED: u32 = 2;
const STATE_FULLSCREEN: u32 = 3;

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

    fn set_fullscreen(&self, req: SetFullscreen, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(toplevel) = self.toplevel.get() {
            let output = self.client.objects.outputs.get(&req.output);
            toplevel.tl_set_fullscreen(true, output);
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
    pub fn handle_output_changed(&self, prev: Option<Rc<OutputNode>>, new: Rc<OutputNode>) {
        if let Some(prev) = prev {
            if prev.id == new.id {
                return;
            }
            let prev_bindings = prev.global.bindings.borrow();
            if let Some(bindings) = prev_bindings.get(&self.client.id) {
                for binding in bindings {
                    self.send_output_leave(binding.1.to_owned());
                }
            }
        }
        let new_bindings = new.global.bindings.borrow();
        if let Some(bindings) = new_bindings.get(&self.client.id) {
            for binding in bindings {
                self.send_output_leave(binding.1.to_owned());
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
        if fullscreen {
            state.push(STATE_FULLSCREEN);
        }
        self.client.event(State {
            self_id: self.id,
            state: &state.as_slice(),
        });
    }

    #[expect(dead_code)]
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

#[derive(Debug, Error)]
pub enum ZwlrForeignToplevelHandleV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwlrForeignToplevelHandleV1Error, ClientError);
