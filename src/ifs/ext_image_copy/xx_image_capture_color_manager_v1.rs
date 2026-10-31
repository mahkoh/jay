use crate::client::CAP_SCREENCOPY_MANAGER;
use crate::client::Client;
use crate::client::ClientCaps;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::ext_image_capture_source_v1::ImageCaptureSource;
use crate::ifs::ext_image_copy::ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1Error;
use crate::ifs::xx_image_capture_source_colors_v1::XxImageCaptureSourceColorsV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::state::State;
use crate::tree::TreeTimeline::LiveTL;
use crate::utils::clonecell::CloneCell;
use crate::utils::event_listener::EventListener;
use crate::wire::XxImageCaptureColorManagerV1Id;
use crate::wire::xx_image_capture_color_manager_v1::*;
use jay_proc::Global;
use jay_proc::Object;
use std::rc::Rc;
use std::rc::Weak;
use thiserror::Error;

#[derive(Global)]
pub struct XxImageCaptureColorManagerV1Global {
    pub name: GlobalName,
}

impl XxImageCaptureColorManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: XxImageCaptureColorManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), XxImageCaptureColorManagerV1Error> {
        let obj = Rc::new(XxImageCaptureColorManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

#[derive(Object)]
pub struct XxImageCaptureColorManagerV1 {
    pub(super) id: XxImageCaptureColorManagerV1Id,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) version: Version,
}

impl XxImageCaptureColorManagerV1RequestHandler for XxImageCaptureColorManagerV1 {
    type Error = XxImageCaptureColorManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn get_capture_source_colors(
        &self,
        req: GetCaptureSourceColors,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let obj = Rc::new(XxImageCaptureSourceColorsV1 {
            id: req.colors,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            desc: CloneCell::new(self.client.state.color_manager.srgb_gamma22().clone()),
            output_listener: Default::default(),
            surface_listener: Default::default(),
        });
        track!(self.client, &obj);
        self.client.add_client_obj(&obj);
        macro_rules! init {
            ($field:ident) => {
                obj.$field
                    .get_or_init(|| EventListener::new(Rc::downgrade(&obj) as Weak<_>))
            };
        }
        match &self.client.lookup(req.source)?.ty {
            ImageCaptureSource::Output(o) => {
                if let Some(o) = o.node() {
                    obj.desc.set(o.node_state[LiveTL].color_description.get());
                    init!(output_listener).attach(&o.global.connector.listeners);
                }
            }
            ImageCaptureSource::Toplevel(tl) => {
                if let Some(tl) = tl.get()
                    && let Some(s) = tl.tl_surface()
                {
                    obj.desc.set(s.color_description());
                    init!(surface_listener).attach(&s.color_description_changed);
                }
            }
        }
        Ok(())
    }

    fn set_frame_image_description(
        &self,
        req: SetFrameImageDescription,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let cd = self.client.lookup(req.image_description)?;
        let Some(desc) = &cd.description else {
            return Err(XxImageCaptureColorManagerV1Error::NotReady);
        };
        let frame = self.client.lookup(req.frame)?;
        frame.set_color_description(desc)?;
        Ok(())
    }
}

impl Global for XxImageCaptureColorManagerV1Global {
    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_SCREENCOPY_MANAGER
    }

    fn exposed(&self, state: &State) -> bool {
        state.color_management_available()
    }
}

#[derive(Debug, Error)]
pub enum XxImageCaptureColorManagerV1Error {
    #[error(transparent)]
    Lookup(#[from] LookupError),
    #[error("The image description is not ready")]
    NotReady,
    #[error(transparent)]
    FrameError(#[from] ExtImageCopyCaptureFrameV1Error),
}
