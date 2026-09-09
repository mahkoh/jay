use crate::client::Client;
use crate::client::ClientError;
use crate::ifs::color_management::CAUSE_NO_OUTPUT;
use crate::ifs::color_management::wp_image_description_v1::WpImageDescriptionV1;
use crate::ifs::wl_output::OutputGlobalOpt;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::state::OutputEventListener;
use crate::tree::OutputNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::utils::event_listener::EventListener;
use crate::wire::WpColorManagementOutputV1Id;
use crate::wire::wp_color_management_output_v1::*;
use std::rc::Rc;
use thiserror::Error;

pub struct WpColorManagementOutputV1 {
    pub id: WpColorManagementOutputV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
    pub output: Rc<OutputGlobalOpt>,
    pub listener: EventListener<dyn OutputEventListener>,
}

impl OutputEventListener for WpColorManagementOutputV1 {
    fn color_description_changed(self: Rc<Self>, _on: &Rc<OutputNode>) {
        self.send_image_description_changed();
    }
}

impl WpColorManagementOutputV1 {
    fn send_image_description_changed(&self) {
        self.client
            .event(ImageDescriptionChanged { self_id: self.id });
    }
}

impl WpColorManagementOutputV1RequestHandler for WpColorManagementOutputV1 {
    type Error = WpColorManagementOutputV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.listener.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_image_description(
        &self,
        req: GetImageDescription,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let obj = Rc::new(WpImageDescriptionV1 {
            id: req.image_description,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            description: self
                .output
                .node()
                .map(|o| o.node_state[LiveTL].color_description.get()),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        if obj.description.is_some() {
            obj.send_ready();
        } else {
            obj.send_failed(CAUSE_NO_OUTPUT, "the output no longer exists");
        }
        Ok(())
    }
}

object_base! {
    self = WpColorManagementOutputV1;
    version = self.version;
}

impl Object for WpColorManagementOutputV1 {}

simple_add_obj!(WpColorManagementOutputV1);

#[derive(Debug, Error)]
pub enum WpColorManagementOutputV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpColorManagementOutputV1Error, ClientError);
