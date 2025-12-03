use {
    crate::{
        client::{Client, ClientError},
        cmm::cmm_description::ColorDescription,
        ifs::color_management::wp_image_description_reference_v1::WpImageDescriptionReferenceV1,
        leaks::Tracker,
        object::{Object, Version},
        utils::{
            clonecell::CloneCell,
            event_listener::{EventListener, EventSource},
        },
        wire::{ExtImageCaptureSourceColorsV1Id, ext_image_capture_source_colors_v1::*},
    },
    std::{
        cell::{Cell, LazyCell},
        ops::Deref,
        rc::Rc,
    },
    thiserror::Error,
};

#[derive(Default)]
pub struct PreferredColorDescriptionListeners {
    init: Cell<bool>,
    source: LazyCell<EventSource<dyn PreferredColorDescriptionListener>>,
}

impl PreferredColorDescriptionListeners {
    pub fn changed(&self, desc: impl FnOnce() -> Rc<ColorDescription>) {
        if !self.init.get() {
            return;
        }
        let source = self.source.deref();
        if source.has_listeners() {
            let desc = desc();
            for listener in source.iter() {
                listener.changed(&desc);
            }
        }
    }

    pub fn attach(&self, listener: &EventListener<dyn PreferredColorDescriptionListener>) {
        self.init.set(true);
        listener.attach(&self.source);
    }
}

pub trait PreferredColorDescriptionListener {
    fn changed(&self, new: &Rc<ColorDescription>);
}

pub struct ExtImageCaptureSourceColorsV1 {
    pub id: ExtImageCaptureSourceColorsV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
    pub desc: CloneCell<Rc<ColorDescription>>,
    pub listener: EventListener<dyn PreferredColorDescriptionListener>,
}

impl ExtImageCaptureSourceColorsV1 {
    fn send_preferred_changed(&self, new: &Rc<ColorDescription>) {
        self.client.event(PreferredChanged {
            self_id: self.id,
            identity: new.id.raw(),
        });
    }
}

impl PreferredColorDescriptionListener for ExtImageCaptureSourceColorsV1 {
    fn changed(&self, new: &Rc<ColorDescription>) {
        self.desc.set(new.clone());
        self.send_preferred_changed(new);
    }
}

impl ExtImageCaptureSourceColorsV1RequestHandler for ExtImageCaptureSourceColorsV1 {
    type Error = ExtImageCaptureSourceColorsError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_preferred(&self, req: GetPreferred, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let ref_ = Rc::new(WpImageDescriptionReferenceV1 {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            description: self.desc.get(),
        });
        track!(self.client, ref_);
        self.client.add_client_obj(&ref_)?;
        Ok(())
    }
}

object_base! {
    self = ExtImageCaptureSourceColorsV1;
    version = self.version;
}

impl Object for ExtImageCaptureSourceColorsV1 {}

simple_add_obj!(ExtImageCaptureSourceColorsV1);

#[derive(Debug, Error)]
pub enum ExtImageCaptureSourceColorsError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtImageCaptureSourceColorsError, ClientError);
