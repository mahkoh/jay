use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::WlSurface,
        leaks::Tracker,
        object::{Object, Version},
        wire::{wp_content_type_v1::*, WpContentTypeV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

const NONE: u32 = 0;
const PHOTO: u32 = 1;
const VIDEO: u32 = 2;
const GAME: u32 = 3;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ContentType {
    Photo,
    Video,
    Game,
}

pub struct WpContentTypeV1 {
    pub id: WpContentTypeV1Id,
    pub client: Rc<Client>,
    pub surface: Rc<WlSurface>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WpContentTypeV1RequestHandler for WpContentTypeV1 {
    type Error = WpContentTypeV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.surface.has_content_type_manager.set(false);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_content_type(&self, req: SetContentType, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.content_type == NONE {
            self.surface.set_content_type(None);
            return Ok(());
        }
        let ct = match req.content_type {
            PHOTO => ContentType::Photo,
            VIDEO => ContentType::Video,
            GAME => ContentType::Game,
            _ => return Err(WpContentTypeV1Error::UnknownContentType(req.content_type)),
        };
        self.surface.set_content_type(Some(ct));
        Ok(())
    }
}

object_base! {
    self = WpContentTypeV1;
    version = self.version;
}

impl Object for WpContentTypeV1 {}

simple_add_obj!(WpContentTypeV1);

#[derive(Debug, Error)]
pub enum WpContentTypeV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Content type {0} is unknown")]
    UnknownContentType(u32),
}
efrom!(WpContentTypeV1Error, ClientError);
