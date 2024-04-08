use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_viewport::*, WpViewportId},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpViewport {
    pub id: WpViewportId,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpViewport {
    pub fn new(id: WpViewportId, surface: &Rc<WlSurface>, version: Version) -> Self {
        Self {
            id,
            client: surface.client.clone(),
            surface: surface.clone(),
            tracker: Default::default(),
            version,
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), WpViewportError> {
        if self.surface.viewporter.is_some() {
            return Err(WpViewportError::ViewportExists);
        }
        self.surface.viewporter.set(Some(self.clone()));
        Ok(())
    }
}

impl WpViewportRequestHandler for WpViewport {
    type Error = WpViewportError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pending = &mut *self.surface.pending.borrow_mut();
        pending.src_rect = Some(None);
        pending.dst_size = Some(None);
        self.surface.viewporter.take();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_source(&self, req: SetSource, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let rect = if req.x == -1 && req.y == -1 && req.width == -1 && req.height == -1 {
            None
        } else {
            let invalid = req.x < 0 || req.y < 0 || req.width <= 0 || req.height <= 0;
            if invalid {
                return Err(WpViewportError::InvalidSourceRect);
            }
            Some([req.x, req.y, req.width, req.height])
        };
        self.surface.pending.borrow_mut().src_rect = Some(rect);
        Ok(())
    }

    fn set_destination(&self, req: SetDestination, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let size = if req.width == -1 && req.height == -1 {
            None
        } else if req.width <= 0 || req.height <= 0 {
            return Err(WpViewportError::InvalidDestRect);
        } else {
            Some((req.width, req.height))
        };
        self.surface.pending.borrow_mut().dst_size = Some(size);
        Ok(())
    }
}

object_base! {
    self = WpViewport;
    version = self.version;
}

impl Object for WpViewport {}

simple_add_obj!(WpViewport);

#[derive(Debug, Error)]
pub enum WpViewportError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has a viewport")]
    ViewportExists,
    #[error("Rectangle is empty or outside the first quadrant")]
    InvalidSourceRect,
    #[error("Rectangle is empty")]
    InvalidDestRect,
}
efrom!(WpViewportError, ClientError);
