use crate::client::Client;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::JayKeymapBuilderId;
use crate::wire::jay_keymap_builder::*;
use jay_proc::Object;
use std::cell::Cell;
use std::convert::Infallible;
use std::rc::Rc;
use uapi::OwnedFd;

#[derive(Object)]
pub struct JayKeymapBuilder {
    pub id: JayKeymapBuilderId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub kind: Cell<Option<MapKind>>,
    pub shortcuts_group: Cell<Option<u32>>,
}

pub enum MapKind {
    Map {
        fd: Rc<OwnedFd>,
        len: u32,
    },
    Names {
        rules: Option<String>,
        model: Option<String>,
        layout: Option<String>,
        variant: Option<String>,
        options: Option<String>,
    },
}

impl JayKeymapBuilderRequestHandler for JayKeymapBuilder {
    type Error = Infallible;

    fn set_map(&self, req: SetMap, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.kind.set(Some(MapKind::Map {
            fd: req.keymap,
            len: req.keymap_len,
        }));
        Ok(())
    }

    fn set_names(&self, req: SetNames<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.kind.set(Some(MapKind::Names {
            rules: req.rules.map(Into::into),
            model: req.model.map(Into::into),
            layout: req.layout.map(Into::into),
            variant: req.variant.map(Into::into),
            options: req.options.map(Into::into),
        }));
        Ok(())
    }

    fn set_shortcuts_group(
        &self,
        req: SetShortcutsGroup,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.shortcuts_group.set(Some(req.group));
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}
