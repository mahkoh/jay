use crate::client::Client;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::globals::RemovableWaylandGlobal;
use crate::ifs::wl_output::OUTPUT_VERSION;
use crate::ifs::wl_output::WlOutput;
use crate::ifs::wl_output::WlOutputGlobal;
use crate::object::Version;
use crate::wire::WlOutputId;
use jay_proc::Global;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Global)]
#[interface(WlOutput)]
struct RemovedOutputGlobal {
    name: GlobalName,
}

impl RemovedOutputGlobal {
    fn bind_(
        self: Rc<Self>,
        id: WlOutputId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(WlOutput {
            global: Default::default(),
            id,
            xdg_outputs: Default::default(),
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

impl Global for RemovedOutputGlobal {
    fn version(&self) -> u32 {
        OUTPUT_VERSION
    }
}

impl RemovableWaylandGlobal for WlOutputGlobal {
    fn create_replacement(self: Rc<Self>) -> Rc<dyn Global> {
        Rc::new(RemovedOutputGlobal { name: self.name })
    }
}
