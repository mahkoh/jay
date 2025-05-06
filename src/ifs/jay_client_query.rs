use {
    crate::{
        client::{Client, ClientError, ClientId},
        leaks::Tracker,
        object::{Object, Version},
        utils::copyhashmap::CopyHashMap,
        wire::{
            JayClientQueryId,
            jay_client_query::{
                AddAll, AddId, Comm, Destroy, Done, End, Exe, Execute, IsXwayland,
                JayClientQueryRequestHandler, Pid, SandboxAppId, SandboxEngine, SandboxInstanceId,
                Sandboxed, Start, Uid,
            },
        },
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct JayClientQuery {
    pub id: JayClientQueryId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    ids: CopyHashMap<ClientId, ()>,
    all: Cell<bool>,
}

impl JayClientQuery {
    pub fn new(client: &Rc<Client>, id: JayClientQueryId, version: Version) -> Self {
        Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            ids: Default::default(),
            all: Cell::new(false),
        }
    }
}

impl JayClientQueryRequestHandler for JayClientQuery {
    type Error = JayClientQueryError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn execute(&self, _req: Execute, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let handle_client = |client: &Rc<Client>| {
            self.client.event(Start {
                self_id: self.id,
                id: client.id.raw(),
            });
            if !client.is_xwayland {
                self.client.event(Uid {
                    self_id: self.id,
                    uid: client.pid_info.uid,
                });
                self.client.event(Pid {
                    self_id: self.id,
                    pid: client.pid_info.pid,
                });
                self.client.event(Comm {
                    self_id: self.id,
                    comm: &client.pid_info.comm,
                });
                self.client.event(Exe {
                    self_id: self.id,
                    exe: &client.pid_info.exe,
                });
            }
            if client.acceptor.sandboxed {
                self.client.event(Sandboxed { self_id: self.id });
            }
            if client.is_xwayland {
                self.client.event(IsXwayland { self_id: self.id });
            }
            if let Some(engine) = &client.acceptor.sandbox_engine {
                self.client.event(SandboxEngine {
                    self_id: self.id,
                    engine,
                });
            }
            if let Some(app_id) = &client.acceptor.app_id {
                self.client.event(SandboxAppId {
                    self_id: self.id,
                    app_id,
                });
            }
            if let Some(instance_id) = &client.acceptor.instance_id {
                self.client.event(SandboxInstanceId {
                    self_id: self.id,
                    instance_id,
                });
            }
            self.client.event(End { self_id: self.id });
        };
        if self.all.get() {
            for client in self.client.state.clients.clients.borrow().values() {
                handle_client(&client.data);
            }
        } else {
            for &id in self.ids.lock().keys() {
                let Ok(client) = self.client.state.clients.get(id) else {
                    continue;
                };
                handle_client(&client);
            }
        }
        self.client.event(Done { self_id: self.id });
        Ok(())
    }

    fn add_all(&self, _req: AddAll, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.all.set(true);
        Ok(())
    }

    fn add_id(&self, req: AddId, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.ids.set(ClientId::from_raw(req.id), ());
        Ok(())
    }
}

object_base! {
    self = JayClientQuery;
    version = self.version;
}

impl Object for JayClientQuery {}

simple_add_obj!(JayClientQuery);

#[derive(Debug, Error)]
pub enum JayClientQueryError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayClientQueryError, ClientError);
