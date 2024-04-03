use {
    crate::{
        async_engine::SpawnedFuture,
        client::{Client, ClientId, EventFormatter},
        it::{
            test_error::{StdError, TestError},
            test_ifs::{test_callback::TestCallback, test_registry::TestRegistry},
            test_object::TestObject,
            test_utils::test_object_ext::TestObjectExt,
            testrun::TestRun,
        },
        object::{ObjectId, WL_DISPLAY_ID},
        utils::{
            asyncevent::AsyncEvent,
            bitfield::Bitfield,
            buffd::{BufFdIn, BufFdOut, MsgFormatter, MsgParser, OutBuffer, OutBufferSwapchain},
            copyhashmap::CopyHashMap,
            stack::Stack,
            vec_ext::VecExt,
        },
        wire::wl_display,
    },
    std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        future::Future,
        mem,
        rc::Rc,
        task::Poll,
    },
    uapi::OwnedFd,
};

pub struct TestTransport {
    pub run: Rc<TestRun>,
    pub socket: Rc<OwnedFd>,
    pub client_id: Cell<ClientId>,
    pub bufs: Stack<Vec<u32>>,
    pub swapchain: Rc<RefCell<OutBufferSwapchain>>,
    pub flush_request: AsyncEvent,
    pub incoming: Cell<Option<SpawnedFuture<()>>>,
    pub outgoing: Cell<Option<SpawnedFuture<()>>>,
    pub objects: CopyHashMap<ObjectId, Rc<dyn TestObject>>,
    pub obj_ids: RefCell<Bitfield>,
    pub killed: Cell<bool>,
}

impl TestTransport {
    pub fn get_registry(self: &Rc<Self>) -> Rc<TestRegistry> {
        let reg = Rc::new(TestRegistry {
            id: self.id(),
            tran: self.clone(),
            globals: Default::default(),
            singletons: Default::default(),
            jay_compositor: Default::default(),
            compositor: Default::default(),
            subcompositor: Default::default(),
            shm: Default::default(),
            spbm: Default::default(),
            viewporter: Default::default(),
            xdg: Default::default(),
            activation: Default::default(),
            foreign_toplevel_list: Default::default(),
            data_device_manager: Default::default(),
            cursor_shape_manager: Default::default(),
            syncobj_manager: Default::default(),
            content_type_manager: Default::default(),
            data_control_manager: Default::default(),
            seats: Default::default(),
        });
        self.send(wl_display::GetRegistry {
            self_id: WL_DISPLAY_ID,
            registry: reg.id,
        })
        .unwrap();
        let _ = self.add_obj(reg.clone());
        reg
    }

    pub fn get_client(&self) -> Result<Rc<Client>, TestError> {
        self.run
            .state
            .clients
            .get(self.client_id.get())
            .map_err(|e| e.into())
    }

    pub fn add_obj(&self, obj: Rc<dyn TestObject>) -> Result<(), TestError> {
        if self.killed.get() {
            bail!("Transport has already been killed");
        }
        let id = obj.id();
        if self.objects.set(id, obj).is_some() {
            bail!("There already is an object with id {}", id);
        }
        Ok(())
    }

    pub fn kill(&self) {
        self.outgoing.take();
        self.incoming.take();
        for (_, object) in self.objects.lock().drain() {
            object.on_remove(self);
        }
    }

    pub fn sync(self: &Rc<Self>) -> impl Future<Output = ()> {
        let cb = Rc::new(TestCallback {
            id: self.id(),
            tran: self.clone(),
            handler: Cell::new(None),
            done: Cell::new(self.killed.get()),
        });
        self.send(wl_display::Sync {
            self_id: WL_DISPLAY_ID,
            callback: cb.id,
        })
        .unwrap();
        let _ = self.add_obj(cb.clone());
        futures_util::future::poll_fn(move |ctx| {
            if cb.done.get() {
                Poll::Ready(())
            } else {
                let waker = ctx.waker().clone();
                cb.handler.set(Some(Box::new(move || waker.wake())));
                Poll::Pending
            }
        })
    }

    pub fn id<T: From<ObjectId>>(&self) -> T {
        ObjectId::from_raw(self.obj_ids.borrow_mut().acquire()).into()
    }

    pub fn error(&self, msg: &str) {
        let msg = format!("In client {}: {}", self.client_id.get(), msg);
        self.run.errors.push(msg);
    }

    pub fn init(self: &Rc<Self>) {
        self.incoming.set(Some(
            self.run.state.eng.spawn(
                Incoming {
                    tc: self.clone(),
                    buf: BufFdIn::new(&self.socket, &self.run.state.ring),
                }
                .run(),
            ),
        ));
        self.outgoing.set(Some(
            self.run.state.eng.spawn(
                Outgoing {
                    tc: self.clone(),
                    buf: BufFdOut::new(&self.socket, &self.run.state.ring),
                    buffers: Default::default(),
                }
                .run(),
            ),
        ));
    }

    pub fn send<M: EventFormatter>(&self, msg: M) -> Result<(), TestError> {
        if self.killed.get() {
            return Ok(());
        }
        let obj = match self.objects.get(&msg.id()) {
            Some(obj) => obj,
            _ => bail!("Object with id {} has already been deleted", msg.id()),
        };
        if obj.interface().name() != msg.interface().name() {
            bail!(
                "Object with id {} has an incompatible interface: {} != {}",
                msg.id(),
                obj.interface().name(),
                msg.interface().name()
            );
        }
        let mut fds = vec![];
        let mut swapchain = self.swapchain.borrow_mut();
        let mut fmt = MsgFormatter::new(&mut swapchain.cur, &mut fds);
        msg.format(&mut fmt);
        fmt.write_len();
        if swapchain.cur.is_full() {
            swapchain.commit();
        }
        self.flush_request.trigger();
        Ok(())
    }

    pub fn get_server_obj<I: Into<ObjectId>, T: 'static>(&self, id: I) -> Result<Rc<T>, TestError> {
        let client = self.get_client()?;
        client.objects.get_obj(id.into())?.downcast()
    }
}

struct Outgoing {
    tc: Rc<TestTransport>,
    buf: BufFdOut,
    buffers: VecDeque<OutBuffer>,
}

impl Outgoing {
    async fn run(mut self: Self) {
        loop {
            self.tc.flush_request.triggered().await;
            if let Err(e) = self.flush().await {
                let msg = format!(
                    "Could not process an outgoing message for client {}: {}",
                    self.tc.client_id.get(),
                    e
                );
                log::error!("{}", msg);
                self.tc.run.errors.push(msg);
                break;
            }
        }
    }

    async fn flush(&mut self) -> Result<(), TestError> {
        {
            let mut swapchain = self.tc.swapchain.borrow_mut();
            swapchain.commit();
            mem::swap(&mut swapchain.pending, &mut self.buffers);
        }
        while let Some(mut cur) = self.buffers.pop_front() {
            if let Err(e) = self.buf.flush_no_timeout(&mut cur).await {
                return Err(e.with_context("Could not write to wayland socket"));
            }
            self.tc.swapchain.borrow_mut().free.push(cur);
        }
        Ok(())
    }
}

struct Incoming {
    tc: Rc<TestTransport>,
    buf: BufFdIn,
}

impl Incoming {
    async fn run(mut self: Self) {
        loop {
            if let Err(e) = self.handle_msg().await {
                let msg = format!(
                    "Could not process an incoming message for client {}: {}",
                    self.tc.client_id.get(),
                    e
                );
                log::error!("{}", msg);
                self.tc.run.errors.push(msg);
                break;
            }
        }
        self.tc.kill();
    }

    async fn handle_msg(&mut self) -> Result<(), TestError> {
        let mut hdr = [0u32, 0];
        if let Err(e) = self.buf.read_full(&mut hdr[..]).await {
            return Err(e.with_context("Could not read from wayland socket"));
        }
        let obj_id = ObjectId::from_raw(hdr[0]);
        let len = (hdr[1] >> 16) as usize;
        let request = hdr[1] & 0xffff;
        if len < 8 {
            bail!("Message size is < 8");
        }
        if len % 4 != 0 {
            bail!("Message size is not a multiple of 4");
        }
        let len = len / 4 - 2;
        let mut data_buf = self.tc.bufs.pop().unwrap_or_default();
        data_buf.clear();
        data_buf.reserve(len);
        let unused = data_buf.split_at_spare_mut_ext().1;
        if let Err(e) = self.buf.read_full(&mut unused[..len]).await {
            return Err(e.with_context("Could not read from wayland socket"));
        }
        unsafe {
            data_buf.set_len(len);
        }
        let object = match self.tc.objects.get(&obj_id) {
            Some(obj) => obj,
            _ => bail!(
                "Compositor sent a message for object {} which does not exist",
                obj_id
            ),
        };
        let parser = MsgParser::new(&mut self.buf, &data_buf);
        object.handle_request(request, parser)?;
        if data_buf.capacity() > 0 {
            self.tc.bufs.push(data_buf);
        }
        Ok(())
    }
}
