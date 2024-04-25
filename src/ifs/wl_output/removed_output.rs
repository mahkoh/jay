use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName, RemovableWaylandGlobal},
        ifs::wl_output::{WlOutput, WlOutputGlobal, OUTPUT_VERSION},
        object::Version,
        wire::WlOutputId,
    },
    std::rc::Rc,
    thiserror::Error,
};

struct RemovedOutputGlobal {
    name: GlobalName,
}

impl RemovedOutputGlobal {
    fn bind_(
        self: Rc<Self>,
        id: WlOutputId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), RemovedOutputError> {
        let obj = Rc::new(WlOutput {
            global: Default::default(),
            id,
            xdg_outputs: Default::default(),
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(RemovedOutputGlobal, WlOutput, RemovedOutputError);

impl Global for RemovedOutputGlobal {
    fn singleton(&self) -> bool {
        false
    }

    fn version(&self) -> u32 {
        OUTPUT_VERSION
    }
}

simple_add_global!(RemovedOutputGlobal);

impl RemovableWaylandGlobal for WlOutputGlobal {
    fn create_replacement(&self) -> Rc<dyn Global> {
        Rc::new(RemovedOutputGlobal { name: self.name })
    }
}

#[derive(Debug, Error)]
enum RemovedOutputError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(RemovedOutputError, ClientError);
