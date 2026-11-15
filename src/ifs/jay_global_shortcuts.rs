use crate::client::Client;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::JayGlobalShortcutsId;
use crate::wire::jay_global_shortcuts::*;
use jay_proc::Object;
use std::rc::Rc;

#[derive(Object)]
pub struct JayGlobalShortcuts {
    pub id: JayGlobalShortcutsId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl JayGlobalShortcutsRequestHandler for JayGlobalShortcuts {
    type Error = !;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn get(&self, _req: Get, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let (shortcuts, configured_triggers) = self.client.state.global_shortcuts.snapshot();
        for sc in shortcuts {
            self.client.event(Shortcut {
                self_id: self.id,
                app_id: &sc.app_id,
                shortcut_id: &sc.shortcut_id,
                description: &sc.description,
                preferred_trigger: &sc.preferred_trigger,
                configured_trigger: &sc.configured_trigger,
            });
        }
        for trigger in configured_triggers {
            self.client.event(ConfiguredTrigger {
                self_id: self.id,
                app_id: &trigger.app_id,
                shortcut_id: &trigger.shortcut_id,
                trigger: &trigger.trigger,
            });
        }
        Ok(())
    }
}
