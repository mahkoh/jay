use crate::async_engine::AsyncEngine;
use crate::client::Client;
use crate::client::ClientError;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::utils::bool_ext::BoolExt;
use crate::utils::client_trace::ClientTraceMessage;
use crate::utils::client_trace::ClientTraceWrite;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::cow_ext::OptionCowExt;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::hash_map_ext::HashMapExt;
use crate::utils::numcell::NumCell;
use crate::utils::str_fmt::StrFmtUs;
use crate::wire::JayClientTraceId;
use crate::wire::ObjectId;
use crate::wire::jay_client_trace::*;
use bincode::Options;
use jay_algorithms::oserror::OsError;
use jay_algorithms::oserror::OsErrorExt2;
use jay_config::_private::bincode_ops;
use jay_proc::StrFmt;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::Cell;
use std::io;
use std::io::Write;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c;

#[derive(Debug, Serialize, Deserialize, StrFmt)]
pub struct ClientTraceInfo<'a> {
    pub id: u64,
    pub sandboxed: bool,
    pub sandbox_engine: Option<Cow<'a, str>>,
    pub sandbox_app_id: Option<Cow<'a, str>>,
    pub sandbox_instance_id: Option<Cow<'a, str>>,
    pub uid: c::uid_t,
    pub pid: c::pid_t,
    pub is_xwayland: bool,
    pub comm: Cow<'a, str>,
    pub exe: Cow<'a, str>,
    pub tag: Option<Cow<'a, str>>,
    pub connect_time_us: StrFmtUs,
    pub now_us: StrFmtUs,
}

#[derive(Debug, Error)]
enum ClientTraceInfoError {
    #[error("Could not serialize client info")]
    Serialize(#[source] bincode::Error),
    #[error("Could not create memfd")]
    Memfd(#[source] OsError),
    #[error("Could not write memfd")]
    Write(#[source] io::Error),
}

pub struct ClientTracers {
    ids: ClientTracerIds,
    eng: Rc<AsyncEngine>,
    pub num: NumCell<usize>,
    map: CopyHashMap<ClientTracerId, Rc<JayClientTrace>>,
}

linear_ids!(ClientTracerIds, ClientTracerId, u64);

struct TraceTarget {
    id: ClientTracerId,
    client: Rc<Client>,
}

pub struct JayClientTrace {
    id: JayClientTraceId,
    client: Rc<Client>,
    target: Cell<Option<TraceTarget>>,
    tracker: Tracker<Self>,
    version: Version,
    sink: Option<ClientTraceWrite>,
}

impl ClientTracers {
    pub fn new(eng: &Rc<AsyncEngine>) -> Self {
        Self {
            ids: Default::default(),
            eng: eng.clone(),
            num: Default::default(),
            map: Default::default(),
        }
    }

    #[cold]
    pub fn handle_message(&self, id: ObjectId, msg: &dyn ClientTraceMessage) {
        let now_us = self.eng.now_rt().usec();
        for trace in self.map.lock().values() {
            if let Some(sink) = &trace.sink {
                sink.write_msg(id, now_us, msg);
            }
        }
    }

    #[cold]
    pub fn handle_delete_id(&self, id: ObjectId) {
        for trace in self.map.lock().values() {
            if let Some(sink) = &trace.sink {
                sink.write_delete_id(id);
            }
        }
    }

    pub fn handle_disconnected(&self) {
        for debugger in self.map.clear().drain_values() {
            debugger.send_disconnected();
            debugger.target.take();
        }
        self.num.set(0);
    }
}

impl ClientTraceInfo<'_> {
    fn create(target: &Rc<Client>) -> Result<Rc<OwnedFd>, ClientTraceInfoError> {
        let info = ClientTraceInfo {
            id: target.id.raw(),
            sandboxed: target.acceptor.sandboxed,
            sandbox_engine: target.acceptor.sandbox_engine.as_deref().borrowed(),
            sandbox_app_id: target.acceptor.app_id.as_deref().borrowed(),
            sandbox_instance_id: target.acceptor.instance_id.as_deref().borrowed(),
            uid: target.pid_info.uid,
            pid: target.pid_info.pid,
            is_xwayland: target.is_xwayland,
            comm: target.pid_info.comm.as_str().into(),
            exe: target.pid_info.exe.as_str().into(),
            tag: target.acceptor.tag.as_deref().borrowed(),
            connect_time_us: StrFmtUs(target.connect_time_us),
            now_us: StrFmtUs(target.state.now_usec_rt()),
        };
        let info = bincode_ops()
            .serialize(&info)
            .map_err(ClientTraceInfoError::Serialize)?;
        let mut memfd = uapi::memfd_create("client-info", c::MFD_CLOEXEC)
            .map_os_err(ClientTraceInfoError::Memfd)?;
        memfd
            .write_all(&info)
            .map_err(ClientTraceInfoError::Write)?;
        Ok(Rc::new(memfd))
    }
}

impl JayClientTrace {
    pub fn install(
        id: JayClientTraceId,
        client: &Rc<Client>,
        target: Option<&Rc<Client>>,
        version: Version,
        server: bool,
    ) -> Result<(), ClientError> {
        let state = &client.state;
        let mut error = String::new();
        let sink = target.is_some().and_then(|| {
            ClientTraceWrite::new(&state.ring)
                .inspect_err(|e| {
                    error = format!("Could not create ClientTraceWrite: {}", ErrorFmt(e));
                    log::error!("{error}");
                })
                .ok()
        });
        let info = target.and_then(|target| {
            ClientTraceInfo::create(target)
                .inspect_err(|e| {
                    error = format!("Could not create client info: {}", ErrorFmt(e));
                    log::error!("{error}");
                })
                .ok()
        });
        let slf = Rc::new(Self {
            id,
            client: client.clone(),
            target: Default::default(),
            tracker: Default::default(),
            version,
            sink,
        });
        track!(client, slf);
        if server {
            client.add_server_obj(&slf);
        } else {
            client.add_client_obj(&slf)?;
        }
        if let Some(target) = target
            && let Some(sink) = &slf.sink
            && let Some(info) = &info
        {
            let target = TraceTarget {
                id: target.tracers.ids.next(),
                client: target.clone(),
            };
            slf.send_info(info);
            slf.send_storage(unsafe { sink.memfd() });
            target.client.tracers.map.set(target.id, slf.clone());
            target.client.tracers.num.fetch_add(1);
            slf.target.set(Some(target));
        } else if target.is_none() {
            slf.send_failed("Client does not exist");
        } else {
            slf.send_failed(&error);
        }
        Ok(())
    }

    fn send_failed(&self, msg: &str) {
        self.client.event(Failed {
            self_id: self.id,
            msg,
        });
    }

    fn send_disconnected(&self) {
        self.client.event(Disconnected { self_id: self.id });
    }

    fn send_info(&self, fd: &Rc<OwnedFd>) {
        self.client.event(Info {
            self_id: self.id,
            fd: fd.clone(),
        });
    }

    fn send_storage(&self, fd: &Rc<OwnedFd>) {
        self.client.event(Storage {
            self_id: self.id,
            fd: fd.clone(),
        });
    }

    fn detach(&self) {
        if let Some(target) = self.target.take() {
            target.client.tracers.map.remove(&target.id);
            target.client.tracers.num.fetch_sub(1);
        }
    }
}

impl JayClientTraceRequestHandler for JayClientTrace {
    type Error = JayClientDebuggerError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayClientTrace;
    version = self.version;
}

impl Object for JayClientTrace {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}

simple_add_obj!(JayClientTrace);

#[derive(Debug, Error)]
pub enum JayClientDebuggerError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayClientDebuggerError, ClientError);
