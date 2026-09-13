use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_seat::text_input::zwp_text_input_v3::ZwpTextInputV3;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::ZwpTextInputManagerV3Id;
use crate::wire::zwp_text_input_manager_v3::*;
use jay_proc::Global;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Global)]
pub struct ZwpTextInputManagerV3Global {
    name: GlobalName,
}

#[derive(Object)]
pub struct ZwpTextInputManagerV3 {
    id: ZwpTextInputManagerV3Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl ZwpTextInputManagerV3Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpTextInputManagerV3Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(ZwpTextInputManagerV3 {
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

impl Global for ZwpTextInputManagerV3Global {
    fn version(&self) -> u32 {
        1
    }
}

impl ZwpTextInputManagerV3RequestHandler for ZwpTextInputManagerV3 {
    type Error = LookupError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn get_text_input(&self, req: GetTextInput, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?;
        let ti = Rc::new(ZwpTextInputV3::new(
            req.id,
            &self.client,
            &seat.global,
            self.version,
        ));
        track!(self.client, ti);
        self.client.add_client_obj(&ti);
        seat.global
            .text_inputs
            .borrow_mut()
            .entry(self.client.id)
            .or_default()
            .set(req.id, ti.clone());
        if let Some(surface) = seat.global.keyboard_node.get().node_into_surface()
            && surface.client.id == self.client.id
        {
            ti.send_enter(&surface);
            ti.send_done();
        }
        Ok(())
    }
}
