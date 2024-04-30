use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ipc::wl_data_source::WlDataSource, wl_seat::WlSeatGlobal,
            wl_surface::xdg_surface::xdg_toplevel::XdgToplevel,
        },
        leaks::Tracker,
        object::{Object, Version},
        utils::clonecell::CloneCell,
        wire::{xdg_toplevel_drag_v1::*, XdgToplevelDragV1Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct XdgToplevelDragV1 {
    pub id: XdgToplevelDragV1Id,
    pub client: Rc<Client>,
    pub source: Rc<WlDataSource>,
    pub tracker: Tracker<Self>,
    pub toplevel: CloneCell<Option<Rc<XdgToplevel>>>,
    pub x_off: Cell<i32>,
    pub y_off: Cell<i32>,
    pub version: Version,
}

impl XdgToplevelDragV1 {
    pub fn new(id: XdgToplevelDragV1Id, source: &Rc<WlDataSource>, version: Version) -> Self {
        Self {
            id,
            client: source.data.client.clone(),
            source: source.clone(),
            tracker: Default::default(),
            toplevel: Default::default(),
            x_off: Cell::new(0),
            y_off: Cell::new(0),
            version,
        }
    }

    pub fn is_ongoing(&self) -> bool {
        self.source.data.was_used() && !self.source.data.was_dropped_or_cancelled()
    }

    fn detach(&self) {
        self.source.toplevel_drag.take();
        if let Some(tl) = self.toplevel.take() {
            tl.drag.take();
        }
    }
}

impl XdgToplevelDragV1RequestHandler for XdgToplevelDragV1 {
    type Error = XdgToplevelDragV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.is_ongoing() {
            return Err(XdgToplevelDragV1Error::ActiveDrag);
        }
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn attach(&self, req: Attach, slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.source.data.was_dropped_or_cancelled() {
            return Ok(());
        }
        let toplevel = self.client.lookup(req.toplevel)?;
        if toplevel.drag.set(Some(slf.clone())).is_some() {
            return Err(XdgToplevelDragV1Error::AlreadyDragged);
        }
        if let Some(prev) = self.toplevel.set(Some(toplevel)) {
            if prev.xdg.surface.buffer.is_some() {
                return Err(XdgToplevelDragV1Error::ToplevelAttached);
            }
            if prev.id != req.toplevel {
                prev.drag.set(None);
            }
        }
        self.x_off.set(req.x_offset);
        self.y_off.set(req.y_offset);
        self.start_drag();
        Ok(())
    }
}

impl XdgToplevelDragV1 {
    pub fn start_drag(&self) {
        if !self.is_ongoing() {
            return;
        }
        let Some(tl) = self.toplevel.get() else {
            return;
        };
        tl.prepare_toplevel_drag();
        self.client.state.tree_changed();
    }

    pub fn finish_drag(&self, seat: &Rc<WlSeatGlobal>) {
        if self.source.data.was_used() {
            if let Some(tl) = self.toplevel.get() {
                let output = seat.get_output();
                let (x, y) = seat.pointer_cursor().position();
                tl.drag.take();
                tl.after_toplevel_drag(
                    &output,
                    x.round_down() - self.x_off.get(),
                    y.round_down() - self.y_off.get(),
                );
            }
        }
        self.detach();
    }
}

object_base! {
    self = XdgToplevelDragV1;
    version = self.version;
}

impl Object for XdgToplevelDragV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(XdgToplevelDragV1);

#[derive(Debug, Error)]
pub enum XdgToplevelDragV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The toplevel already has a drag attached")]
    AlreadyDragged,
    #[error("There already is a mapped toplevel attached")]
    ToplevelAttached,
    #[error("The drag is ongoing")]
    ActiveDrag,
}
efrom!(XdgToplevelDragV1Error, ClientError);
