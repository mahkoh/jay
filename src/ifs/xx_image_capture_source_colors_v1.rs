use crate::client::Client;
use crate::cmm::cmm_description::ColorDescription;
use crate::ifs::color_management::wp_image_description_reference_v1::WpImageDescriptionReferenceV1;
use crate::ifs::wl_surface::SurfaceColorDescriptionListener;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::state::OutputEventListener;
use crate::tree::OutputNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::utils::clonecell::CloneCell;
use crate::utils::event_listener::EventListener;
use crate::wire::XxImageCaptureSourceColorsV1Id;
use crate::wire::xx_image_capture_source_colors_v1::*;
use jay_proc::Object;
use std::cell::OnceCell;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
pub struct XxImageCaptureSourceColorsV1 {
    pub id: XxImageCaptureSourceColorsV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
    pub desc: CloneCell<Rc<ColorDescription>>,
    pub output_listener: OnceCell<EventListener<dyn OutputEventListener>>,
    pub surface_listener: OnceCell<EventListener<dyn SurfaceColorDescriptionListener>>,
}

impl XxImageCaptureSourceColorsV1 {
    fn send_preferred_changed(&self, new: &Rc<ColorDescription>) {
        self.client.event(PreferredChanged {
            self_id: self.id,
            identity: new.id.raw(),
        });
    }

    fn handle_new_description(&self, new: &Rc<ColorDescription>) {
        self.desc.set(new.clone());
        self.send_preferred_changed(new);
    }
}

impl OutputEventListener for XxImageCaptureSourceColorsV1 {
    fn color_description_changed(self: Rc<Self>, on: &Rc<OutputNode>) {
        let new = on.node_state[LiveTL].color_description.get();
        self.handle_new_description(&new);
    }
}

impl SurfaceColorDescriptionListener for XxImageCaptureSourceColorsV1 {
    fn changed(&self, new: &Rc<ColorDescription>) {
        self.handle_new_description(new);
    }
}

impl XxImageCaptureSourceColorsV1RequestHandler for XxImageCaptureSourceColorsV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn get_preferred(&self, req: GetPreferred, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let ref_ = Rc::new(WpImageDescriptionReferenceV1 {
            id: req.id,
            version: Version(1),
            client: self.client.clone(),
            tracker: Default::default(),
            description: self.desc.get(),
        });
        track!(self.client, ref_);
        self.client.add_client_obj(&ref_);
        Ok(())
    }
}
