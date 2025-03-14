use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            color_management::{CAUSE_NO_OUTPUT, wp_image_description_v1::WpImageDescriptionV1},
            wl_output::OutputGlobalOpt,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{WpColorManagementOutputV1Id, wp_color_management_output_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpColorManagementOutputV1 {
    pub id: WpColorManagementOutputV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
    pub output: Rc<OutputGlobalOpt>,
}

impl WpColorManagementOutputV1 {
    pub fn send_image_description_changed(&self) {
        self.client
            .event(ImageDescriptionChanged { self_id: self.id });
    }

    fn detach(&self) {
        if let Some(output) = self.output.get() {
            output
                .color_description_listeners
                .remove(&(self.client.id, self.id));
        }
    }
}

impl WpColorManagementOutputV1RequestHandler for WpColorManagementOutputV1 {
    type Error = WpColorManagementOutputV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
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
            description: self.output.get().map(|o| o.color_description.get()),
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

impl Object for WpColorManagementOutputV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(WpColorManagementOutputV1);

#[derive(Debug, Error)]
pub enum WpColorManagementOutputV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpColorManagementOutputV1Error, ClientError);
