use {
    crate::{
        client::{CAP_SCREENCOPY_MANAGER, Client, ClientCaps, ClientError},
        globals::{Global, GlobalName},
        ifs::{
            ext_image_capture_source_colors_v1::ExtImageCaptureSourceColorsV1,
            ext_image_capture_source_v1::ImageCaptureSource,
            ext_image_copy::ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1Error,
        },
        leaks::Tracker,
        object::{Object, Version},
        state::State,
        utils::{clonecell::CloneCell, event_listener::EventListener},
        wire::{ExtImageCaptureColorManagerV1Id, ext_image_capture_color_manager_v1::*},
    },
    std::rc::{Rc, Weak},
    thiserror::Error,
};

pub struct ExtImageCaptureColorManagerV1Global {
    pub name: GlobalName,
}

impl ExtImageCaptureColorManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ExtImageCaptureColorManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ExtImageCaptureColorManagerV1Error> {
        let obj = Rc::new(ExtImageCaptureColorManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

pub struct ExtImageCaptureColorManagerV1 {
    pub(super) id: ExtImageCaptureColorManagerV1Id,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) version: Version,
}

impl ExtImageCaptureColorManagerV1RequestHandler for ExtImageCaptureColorManagerV1 {
    type Error = ExtImageCaptureColorManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_capture_source_colors(
        &self,
        req: GetCaptureSourceColors,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let source = self.client.lookup(req.source)?;
        let output;
        let surface;
        let mut event_source = None;
        let mut desc = self.client.state.color_manager.srgb_gamma22().clone();
        match &source.ty {
            ImageCaptureSource::Output(o) => {
                if let Some(o) = o.get() {
                    desc = o.color_description.get();
                    output = o;
                    event_source = Some(&output.color_description_listeners2);
                }
            }
            ImageCaptureSource::Toplevel(tl) => {
                if let Some(tl) = tl.get()
                    && let Some(s) = tl.tl_scanout_surface()
                {
                    desc = s.color_description();
                    surface = s;
                    event_source = Some(&surface.color_description_listeners);
                }
            }
        }
        let obj = Rc::new_cyclic(|slf: &Weak<ExtImageCaptureSourceColorsV1>| {
            ExtImageCaptureSourceColorsV1 {
                id: req.colors,
                client: self.client.clone(),
                version: self.version,
                tracker: Default::default(),
                desc: CloneCell::new(desc),
                listener: EventListener::new(slf.clone()),
            }
        });
        track!(self.client, &obj);
        self.client.add_client_obj(&obj)?;
        if let Some(source) = event_source {
            source.attach(&obj.listener);
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
            return Err(ExtImageCaptureColorManagerV1Error::NotReady);
        };
        let frame = self.client.lookup(req.frame)?;
        frame.set_color_description(desc)?;
        Ok(())
    }
}

global_base!(
    ExtImageCaptureColorManagerV1Global,
    ExtImageCaptureColorManagerV1,
    ExtImageCaptureColorManagerV1Error
);

impl Global for ExtImageCaptureColorManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

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

simple_add_global!(ExtImageCaptureColorManagerV1Global);

object_base! {
    self = ExtImageCaptureColorManagerV1;
    version = self.version;
}

impl Object for ExtImageCaptureColorManagerV1 {}

simple_add_obj!(ExtImageCaptureColorManagerV1);

#[derive(Debug, Error)]
pub enum ExtImageCaptureColorManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The image description is not ready")]
    NotReady,
    #[error(transparent)]
    FrameError(#[from] ExtImageCopyCaptureFrameV1Error),
}
efrom!(ExtImageCaptureColorManagerV1Error, ClientError);
