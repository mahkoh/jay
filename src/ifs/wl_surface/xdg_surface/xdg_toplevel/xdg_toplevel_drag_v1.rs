use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ipc::wl_data_source::WlDataSource, wl_seat::WlSeatGlobal,
            wl_surface::xdg_surface::xdg_toplevel::XdgToplevel,
        },
        leaks::Tracker,
        object::{Object, Version},
        rect::Rect,
        renderer::Renderer,
        tree::{Node, ToplevelNode},
        utils::clonecell::CloneCell,
        wire::{xdg_toplevel_drag_v1::*, XdgToplevelDragV1Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

fn clamp_offset(offset: i32) -> i32 {
    const OFFSET_CLAMP: i32 = 100_000;
    offset.clamp(-OFFSET_CLAMP, OFFSET_CLAMP)
}

const SET_OFFSET_SINCE: Version = Version(2);

linear_ids!(ToplevelDragIds, ToplevelDragId, u64);

pub struct XdgToplevelDragV1 {
    pub id: XdgToplevelDragV1Id,
    pub toplevel_id: Cell<ToplevelDragId>,
    pub client: Rc<Client>,
    pub source: Rc<WlDataSource>,
    pub tracker: Tracker<Self>,
    pub toplevel: CloneCell<Option<Rc<XdgToplevel>>>,
    pub x_off: Cell<i32>,
    pub y_off: Cell<i32>,
    pub version: Version,
    pub enabled: Cell<bool>,
}

impl XdgToplevelDragV1 {
    pub fn new(id: XdgToplevelDragV1Id, source: &Rc<WlDataSource>, version: Version) -> Self {
        Self {
            id,
            toplevel_id: Cell::new(source.data.client.state.toplevel_drag_ids.next()),
            client: source.data.client.clone(),
            source: source.clone(),
            tracker: Default::default(),
            toplevel: Default::default(),
            x_off: Cell::new(0),
            y_off: Cell::new(0),
            version,
            enabled: Cell::new(true),
        }
    }

    pub fn is_ongoing(&self) -> bool {
        self.enabled.get()
            && self.source.data.was_used()
            && !self.source.data.was_dropped_or_cancelled()
    }

    fn detach(&self) {
        self.source.toplevel_drag.take();
        if let Some(tl) = self.toplevel.take() {
            tl.drag.take();
        }
    }

    fn move2(&self, x: i32, y: i32, damage_initial: bool) {
        if let Some(tl) = self.toplevel.get() {
            if damage_initial && tl.node_visible() {
                tl.xdg.damage();
            }
            let extents = tl.xdg.absolute_desired_extents.get();
            let extents = extents.at_point(x - self.x_off.get(), y - self.y_off.get());
            tl.clone().tl_change_extents(&extents);
            if tl.node_visible() {
                tl.xdg.damage();
            }
        }
    }

    pub fn move_(&self, x: i32, y: i32) {
        self.move2(x, y, true);
    }

    pub fn render(&self, renderer: &mut Renderer<'_>, cursor_rect: &Rect, x: i32, y: i32) {
        if let Some(tl) = self.toplevel.get() {
            if tl.xdg.surface.buffer.get().is_some() {
                let (x, y) = cursor_rect.translate(x - self.x_off.get(), y - self.y_off.get());
                renderer.render_xdg_surface(&tl.xdg, x, y, None)
            }
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
        if let Some(prev) = self.toplevel.set(Some(toplevel.clone())) {
            if prev.xdg.surface.buffer.is_some() {
                return Err(XdgToplevelDragV1Error::ToplevelAttached);
            }
            if prev.id != req.toplevel {
                prev.drag.set(None);
            }
        }
        let x_off = clamp_offset(req.x_offset);
        let y_off = clamp_offset(req.y_offset);
        if self.version >= SET_OFFSET_SINCE {
            let id = self.client.state.toplevel_drag_ids.next();
            self.toplevel_id.set(id);
            self.enabled.set(false);
            let pending = &mut *toplevel.xdg.pending();
            pending.toplevel_drag_id = Some(id);
            pending.toplevel_drag_offset = Some((x_off, y_off));
        } else {
            self.x_off.set(x_off);
            self.y_off.set(y_off);
            self.start_drag();
        }
        Ok(())
    }

    fn set_offset(&self, req: SetOffset, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(tl) = self.toplevel.get() {
            let x_off = clamp_offset(req.x_offset);
            let y_off = clamp_offset(req.y_offset);
            tl.xdg.pending().toplevel_drag_offset = Some((x_off, y_off));
        }
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
        if let Some(seat) = self.source.data.seat.get() {
            let (x, y) = seat.pointer_cursor().position_int();
            self.move2(x, y, false)
        }
    }

    pub fn finish_drag(&self, seat: &Rc<WlSeatGlobal>) {
        if self.enabled.get() && self.source.data.was_used() {
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
