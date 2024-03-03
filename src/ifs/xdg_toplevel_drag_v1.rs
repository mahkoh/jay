use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ipc::wl_data_source::WlDataSource, wl_seat::WlSeatGlobal,
            wl_surface::xdg_surface::xdg_toplevel::XdgToplevel,
        },
        leaks::Tracker,
        object::Object,
        utils::{
            buffd::{MsgParser, MsgParserError},
            clonecell::CloneCell,
        },
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
}

impl XdgToplevelDragV1 {
    pub fn new(id: XdgToplevelDragV1Id, source: &Rc<WlDataSource>) -> Self {
        Self {
            id,
            client: source.data.client.clone(),
            source: source.clone(),
            tracker: Default::default(),
            toplevel: Default::default(),
            x_off: Cell::new(0),
            y_off: Cell::new(0),
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

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelDragV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        if self.is_ongoing() {
            return Err(XdgToplevelDragV1Error::ActiveDrag);
        }
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn attach(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), XdgToplevelDragV1Error> {
        let req: Attach = self.client.parse(&**self, parser)?;
        if self.source.data.was_dropped_or_cancelled() {
            return Ok(());
        }
        let toplevel = self.client.lookup(req.toplevel)?;
        if toplevel.drag.set(Some(self.clone())).is_some() {
            return Err(XdgToplevelDragV1Error::AlreadyDragged);
        }
        if let Some(prev) = self.toplevel.set(Some(toplevel)) {
            if prev.xdg.surface.buffer.get().is_some() {
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

    pub fn start_drag(self: &Rc<Self>) {
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
                let (x, y) = seat.position();
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

    DESTROY => destroy,
    ATTACH => attach,
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
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("The toplevel already has a drag attached")]
    AlreadyDragged,
    #[error("There already is a mapped toplevel attached")]
    ToplevelAttached,
    #[error("The drag is ongoing")]
    ActiveDrag,
}
efrom!(XdgToplevelDragV1Error, ClientError);
efrom!(XdgToplevelDragV1Error, MsgParserError);
