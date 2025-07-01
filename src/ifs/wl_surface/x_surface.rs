use {
    crate::{
        ifs::wl_surface::{
            SurfaceExt, WlSurface, WlSurfaceError,
            x_surface::{xwayland_surface_v1::XwaylandSurfaceV1, xwindow::Xwindow},
        },
        leaks::Tracker,
        tree::{Node, ToplevelNode, ToplevelNodeBase},
        utils::clonecell::CloneCell,
        xwayland::XWaylandEvent,
    },
    std::rc::Rc,
};

pub mod xwayland_surface_v1;
pub mod xwindow;

pub struct XSurface {
    pub surface: Rc<WlSurface>,
    pub xwindow: CloneCell<Option<Rc<Xwindow>>>,
    pub xwayland_surface: CloneCell<Option<Rc<XwaylandSurfaceV1>>>,
    pub tracker: Tracker<Self>,
}

impl SurfaceExt for XSurface {
    fn after_apply_commit(self: Rc<Self>) {
        if let Some(xwindow) = self.xwindow.get() {
            xwindow.map_status_changed();
        }
    }

    fn on_surface_destroy(&self) -> Result<(), WlSurfaceError> {
        if self.xwayland_surface.is_some() {
            return Err(WlSurfaceError::ReloObjectStillExists);
        }
        self.surface.unset_ext();
        if let Some(xwindow) = self.xwindow.take() {
            xwindow.tl_destroy();
            xwindow.data.window.set(None);
            xwindow.data.surface_id.set(None);
            xwindow
                .data
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::SurfaceDestroyed(
                    self.surface.id,
                    self.surface.xwayland_serial.get(),
                ));
        }
        Ok(())
    }

    fn extents_changed(&self) {
        if let Some(xwindow) = self.xwindow.get() {
            xwindow.toplevel_data.pos.set(self.surface.extents.get());
            xwindow.tl_extents_changed();
        }
    }

    fn focus_node(&self) -> Option<Rc<dyn Node>> {
        if let Some(xwindow) = self.xwindow.get()
            && xwindow.tl_accepts_keyboard_focus()
        {
            return Some(xwindow.x.surface.clone());
        }
        None
    }

    fn into_xsurface(self: Rc<Self>) -> Option<Rc<XSurface>> {
        Some(self)
    }
}
