use {
    crate::{
        async_engine::SpawnedFuture,
        client::ClientCaps,
        state::State,
        utils::{copyhashmap::CopyHashMap, errorfmt::ErrorFmt, hash_map_ext::HashMapExt},
    },
    std::{
        cell::Cell,
        fmt::{Display, Formatter},
        rc::Rc,
    },
    uapi::{OwnedFd, c},
};

#[derive(Default)]
pub struct SecurityContextAcceptors {
    ids: AcceptorIds,
    acceptors: CopyHashMap<AcceptorId, Rc<Acceptor>>,
}

linear_ids!(AcceptorIds, AcceptorId, u64);

struct Acceptor {
    id: AcceptorId,
    state: Rc<State>,
    metadata: Rc<AcceptorMetadata>,
    listen_fd: Rc<OwnedFd>,
    close_fd: Rc<OwnedFd>,
    bounding_caps: ClientCaps,
    listen_future: Cell<Option<SpawnedFuture<()>>>,
    close_future: Cell<Option<SpawnedFuture<()>>>,
}

#[derive(Default)]
pub struct AcceptorMetadata {
    pub secure: bool,
    pub sandboxed: bool,
    pub sandbox_engine: Option<String>,
    pub app_id: Option<String>,
    pub instance_id: Option<String>,
}

impl AcceptorMetadata {
    pub fn secure() -> Self {
        Self {
            secure: true,
            ..Default::default()
        }
    }
}

impl SecurityContextAcceptors {
    pub fn clear(&self) {
        for acceptor in self.acceptors.lock().drain_values() {
            acceptor.kill();
        }
    }

    pub fn spawn(
        &self,
        state: &Rc<State>,
        sandbox_engine: Option<String>,
        app_id: Option<String>,
        instance_id: Option<String>,
        listen_fd: &Rc<OwnedFd>,
        close_fd: &Rc<OwnedFd>,
        bounding_caps: ClientCaps,
    ) {
        let acceptor = Rc::new(Acceptor {
            id: self.ids.next(),
            state: state.clone(),
            metadata: Rc::new(AcceptorMetadata {
                secure: false,
                sandboxed: true,
                sandbox_engine,
                app_id,
                instance_id,
            }),
            listen_fd: listen_fd.clone(),
            close_fd: close_fd.clone(),
            bounding_caps,
            listen_future: Cell::new(None),
            close_future: Cell::new(None),
        });
        log::info!("Creating security acceptor {acceptor}");
        acceptor.listen_future.set(Some(
            state
                .eng
                .spawn("security accept", acceptor.clone().accept()),
        ));
        acceptor.close_future.set(Some(
            state
                .eng
                .spawn("security await close", acceptor.clone().close()),
        ));
        self.acceptors.set(acceptor.id, acceptor);
    }
}

impl Acceptor {
    fn kill(&self) {
        log::info!("Destroying security acceptor {self}");
        self.listen_future.take();
        self.close_future.take();
        self.state
            .security_context_acceptors
            .acceptors
            .remove(&self.id);
    }

    async fn accept(self: Rc<Self>) {
        let s = &self.state;
        loop {
            let fd = match s.ring.accept(&self.listen_fd, c::SOCK_CLOEXEC).await {
                Ok(fd) => fd,
                Err(e) => {
                    log::error!("Could not accept a client: {}", ErrorFmt(e));
                    break;
                }
            };
            let id = s.clients.id();
            if let Err(e) = s
                .clients
                .spawn(id, s, fd, self.bounding_caps, true, &self.metadata)
            {
                log::error!("Could not spawn a client: {}", ErrorFmt(e));
                break;
            }
        }
        self.kill();
    }

    async fn close(self: Rc<Self>) {
        let _ = self.state.ring.poll(&self.close_fd, 0).await;
        self.kill();
    }
}

impl Display for Acceptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}/{}/{}",
            self.metadata.sandbox_engine.as_deref().unwrap_or(""),
            self.metadata.app_id.as_deref().unwrap_or(""),
            self.metadata.instance_id.as_deref().unwrap_or(""),
        )
    }
}
