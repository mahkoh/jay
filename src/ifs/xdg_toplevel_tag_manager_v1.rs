use crate::client::Client;
use crate::client::LookupError;
use crate::criteria::tlm::TL_CHANGED_TAG;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::tree::ToplevelNodeBase;
use crate::wire::XdgToplevelTagManagerV1Id;
use crate::wire::xdg_toplevel_tag_manager_v1::*;
use jay_proc::Global;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Global)]
pub struct XdgToplevelTagManagerV1Global {
    name: GlobalName,
}

impl XdgToplevelTagManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: XdgToplevelTagManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(XdgToplevelTagManagerV1 {
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

impl Global for XdgToplevelTagManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}

#[derive(Object)]
pub struct XdgToplevelTagManagerV1 {
    id: XdgToplevelTagManagerV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl XdgToplevelTagManagerV1RequestHandler for XdgToplevelTagManagerV1 {
    type Error = LookupError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn set_toplevel_tag(
        &self,
        req: SetToplevelTag<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let tl = self.client.lookup(req.toplevel)?;
        let tag = &mut *tl.data.tag.borrow_mut();
        if tag == req.tag {
            return Ok(());
        }
        tag.clear();
        tag.push_str(req.tag);
        tl.tl_data().property_changed(TL_CHANGED_TAG);
        Ok(())
    }

    fn set_toplevel_description(
        &self,
        _req: SetToplevelDescription<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
