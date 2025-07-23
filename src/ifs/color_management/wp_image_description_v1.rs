use {
    crate::{
        client::{Client, ClientError},
        cmm::cmm_description::ColorDescription,
        ifs::color_management::{
            wp_color_management_surface_feedback_v1::UNIQUE_CM_IDS_SINCE,
            wp_image_description_info_v1::WpImageDescriptionInfoV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{WpImageDescriptionV1Id, wp_image_description_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpImageDescriptionV1 {
    pub id: WpImageDescriptionV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
    pub description: Option<Rc<ColorDescription>>,
}

impl WpImageDescriptionV1 {
    pub fn send_failed(&self, cause: u32, msg: &str) {
        self.client.event(Failed {
            self_id: self.id,
            cause,
            msg,
        });
    }

    pub fn send_ready(&self) {
        let id = self.description.as_ref().unwrap().id.raw();
        if self.version >= UNIQUE_CM_IDS_SINCE {
            self.client.event(Ready2 {
                self_id: self.id,
                identity: id,
            });
        } else {
            self.client.event(Ready {
                self_id: self.id,
                identity: id as u32,
            });
        }
    }
}

impl WpImageDescriptionV1RequestHandler for WpImageDescriptionV1 {
    type Error = WpImageDescriptionV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_information(&self, req: GetInformation, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(desc) = &self.description else {
            return Err(WpImageDescriptionV1Error::NotReady);
        };
        let obj = Rc::new(WpImageDescriptionInfoV1 {
            id: req.information,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
        });
        self.client.add_client_obj(&obj)?;
        track!(self.client, obj);
        obj.send_description(desc);
        self.client.remove_obj(&*obj)?;
        Ok(())
    }
}

object_base! {
    self = WpImageDescriptionV1;
    version = self.version;
}

impl Object for WpImageDescriptionV1 {}

dedicated_add_obj!(
    WpImageDescriptionV1,
    WpImageDescriptionV1Id,
    wp_image_description
);

#[derive(Debug, Error)]
pub enum WpImageDescriptionV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The description is not ready")]
    NotReady,
}
efrom!(WpImageDescriptionV1Error, ClientError);
