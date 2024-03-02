use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
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
}

impl WpViewport {
    pub fn new(id: WpViewportId, surface: &Rc<WlSurface>) -> Self {
        Self {
            id,
            client: surface.client.clone(),
            surface: surface.clone(),
            tracker: Default::default(),
        }
    }

    pub fn install(self: &Rc<Self>) -> Result<(), WpViewportError> {
        if self.surface.viewporter.is_some() {
            return Err(WpViewportError::ViewportExists);
        }
        self.surface.viewporter.set(Some(self.clone()));
        Ok(())
    }

    fn destroy(&self, msg: MsgParser<'_, '_>) -> Result<(), WpViewportError> {
        let _req: Destroy = self.client.parse(self, msg)?;
        self.surface.pending.src_rect.set(Some(None));
        self.surface.pending.dst_size.set(Some(None));
        self.surface.viewporter.take();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_source(&self, msg: MsgParser<'_, '_>) -> Result<(), WpViewportError> {
        let req: SetSource = self.client.parse(self, msg)?;
        let rect = if req.x == -1 && req.y == -1 && req.width == -1 && req.height == -1 {
            None
        } else {
            let invalid = req.x < 0 || req.y < 0 || req.width <= 0 || req.height <= 0;
            if invalid {
                return Err(WpViewportError::InvalidSourceRect);
            }
            Some([req.x, req.y, req.width, req.height])
        };
        self.surface.pending.src_rect.set(Some(rect));
        Ok(())
    }

    fn set_destination(&self, msg: MsgParser<'_, '_>) -> Result<(), WpViewportError> {
        let req: SetDestination = self.client.parse(self, msg)?;
        let size = if req.width == -1 && req.height == -1 {
            None
        } else if req.width <= 0 || req.height <= 0 {
            return Err(WpViewportError::InvalidDestRect);
        } else {
            Some((req.width, req.height))
        };
        self.surface.pending.dst_size.set(Some(size));
        Ok(())
    }
}

object_base! {
    self = WpViewport;

    DESTROY => destroy,
    SET_SOURCE => set_source,
    SET_DESTINATION => set_destination,
}

impl Object for WpViewport {}

simple_add_obj!(WpViewport);

#[derive(Debug, Error)]
pub enum WpViewportError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The surface already has a viewport")]
    ViewportExists,
    #[error("Rectangle is empty or outside the first quadrant")]
    InvalidSourceRect,
    #[error("Rectangle is empty")]
    InvalidDestRect,
}
efrom!(WpViewportError, MsgParserError);
efrom!(WpViewportError, ClientError);
