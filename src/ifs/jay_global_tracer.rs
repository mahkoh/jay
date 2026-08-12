use crate::client::Client;
use crate::client::ClientError;
use crate::criteria::CritUpstreamNode;
use crate::ifs::jay_client_trace::JayClientTrace;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::utils::copyhashmap::CopyHashMap;
use crate::wire::JayClientTraceId;
use crate::wire::JayGlobalTracerId;
use crate::wire::jay_global_tracer::*;
use std::rc::Rc;
use thiserror::Error;

#[derive(Default)]
pub struct GlobalTracers {
    ids: GlobalTracerIds,
    map: CopyHashMap<GlobalTracerId, Rc<JayGlobalTracer>>,
}

linear_ids!(GlobalTracerIds, GlobalTracerId, u64);

pub struct JayGlobalTracer {
    id: JayGlobalTracerId,
    tracer_id: GlobalTracerId,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
    m: Rc<dyn CritUpstreamNode<Rc<Client>>>,
}

impl GlobalTracers {
    pub fn clear(&self) {
        self.map.clear();
    }

    pub fn announce(&self, client: &Rc<Client>) {
        for v in self.map.lock().values() {
            v.announce_client(client);
        }
    }
}

impl JayGlobalTracer {
    pub fn install(
        id: JayGlobalTracerId,
        client: &Rc<Client>,
        m: &Rc<dyn CritUpstreamNode<Rc<Client>>>,
        version: Version,
    ) -> Result<(), ClientError> {
        let state = &client.state;
        let slf = Rc::new(JayGlobalTracer {
            id,
            tracer_id: state.global_tracers.ids.next(),
            client: client.clone(),
            tracker: Default::default(),
            version,
            m: m.clone(),
        });
        track!(client, slf);
        client.add_client_obj(&slf)?;
        state.global_tracers.map.set(slf.tracer_id, slf.clone());
        let clients: Vec<_> = state
            .clients
            .clients
            .borrow()
            .values()
            .map(|c| &c.data)
            .filter(|c| slf.m.pull(c))
            .cloned()
            .collect();
        for client in &clients {
            slf.announce_client_unchecked(client);
        }
        Ok(())
    }

    fn announce_client(&self, target: &Rc<Client>) {
        if !self.m.pull(target) {
            return;
        }
        self.announce_client_unchecked(target);
    }

    fn announce_client_unchecked(&self, target: &Rc<Client>) {
        if let Err(e) = self.announce_client_(target) {
            self.client.error(e);
        }
    }

    fn announce_client_(&self, target: &Rc<Client>) -> Result<(), ClientError> {
        let id = self.client.new_id()?;
        self.send_client_trace(id);
        JayClientTrace::install(id, &self.client, Some(target), self.version, true)
    }

    fn send_stopped(&self) {
        self.client.event(Stopped { self_id: self.id });
    }

    fn send_client_trace(&self, id: JayClientTraceId) {
        self.client.event(ClientTrace {
            self_id: self.id,
            id,
        });
    }

    fn detach(&self) {
        self.client.state.global_tracers.map.remove(&self.tracer_id);
    }
}

impl JayGlobalTracerRequestHandler for JayGlobalTracer {
    type Error = JayClientsTracerError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn stop(&self, _req: Stop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.send_stopped();
        Ok(())
    }
}

object_base! {
    self = JayGlobalTracer;
    version = self.version;
}

impl Object for JayGlobalTracer {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}

simple_add_obj!(JayGlobalTracer);

#[derive(Debug, Error)]
pub enum JayClientsTracerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayClientsTracerError, ClientError);
