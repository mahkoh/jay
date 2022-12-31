use {
    crate::{
        dbus::{
            property::Get,
            types::{ObjectPath, Signature, Variant},
            AsyncProperty, AsyncReply, AsyncReplySlot, DbusError, DbusObject, DbusObjectData,
            DbusSocket, DbusType, ErrorMessage, Formatter, Headers, InterfaceSignalHandlers,
            Message, MethodCall, Parser, Property, Reply, ReplyHandler, Signal, SignalHandler,
            SignalHandlerApi, SignalHandlerData, BUS_DEST, BUS_PATH, HDR_DESTINATION,
            HDR_ERROR_NAME, HDR_INTERFACE, HDR_MEMBER, HDR_PATH, HDR_REPLY_SERIAL, HDR_SIGNATURE,
            HDR_UNIX_FDS, MSG_ERROR, MSG_METHOD_CALL, MSG_METHOD_RETURN, MSG_SIGNAL,
            NO_REPLY_EXPECTED,
        },
        utils::{bufio::BufIoMessage, errorfmt::ErrorFmt},
        wire_dbus::org,
    },
    std::{
        borrow::Cow, cell::Cell, collections::hash_map::Entry, fmt::Write, marker::PhantomData,
        mem, ops::DerefMut, rc::Rc,
    },
    uapi::c,
};

impl DbusSocket {
    pub fn clear(&self) {
        self.auth.take();
        self.incoming.take();
        self.outgoing_.take();
        self.reply_handlers.clear();
        self.signal_handlers.borrow_mut().clear();
        self.objects.clear();
    }

    pub(super) fn kill(self: &Rc<Self>) {
        self.dead.set(true);
        self.auth.take();
        self.incoming.take();
        self.outgoing_.take();
        let _ = uapi::shutdown(self.fd.raw(), c::SHUT_RDWR);
        let replies = mem::take(self.reply_handlers.lock().deref_mut());
        for (_, handler) in replies {
            handler.handle_error(self, DbusError::Killed);
        }
    }

    pub fn call_noreply<'a, T: MethodCall<'a>>(&self, destination: &str, path: &str, msg: T) {
        if !self.dead.get() {
            self.send_call(path, destination, NO_REPLY_EXPECTED, &msg);
        }
    }

    fn serial(&self) -> u32 {
        self.next_serial.fetch_add(1)
    }

    pub fn call<'a, T, F>(&self, destination: &str, path: &str, msg: T, f: F)
    where
        T: MethodCall<'a>,
        F: for<'b> FnOnce(Result<&<T::Reply as Message<'static>>::Generic<'b>, DbusError>)
            + 'static,
    {
        if self.dead.get() {
            self.run_toplevel
                .schedule(move || f(Err(DbusError::Killed)));
            return;
        }
        let serial = self.send_call(path, destination, 0, &msg);
        self.reply_handlers
            .set(serial, Box::new(SyncReplyHandler(f, PhantomData)));
    }

    pub fn call_async<'a, T>(
        self: &Rc<Self>,
        destination: &str,
        path: &str,
        msg: T,
    ) -> AsyncReply<T::Reply>
    where
        T: MethodCall<'a>,
    {
        if self.dead.get() {
            return AsyncReply {
                socket: self.clone(),
                serial: self.serial(),
                slot: Rc::new(AsyncReplySlot {
                    data: Cell::new(Some(Err(DbusError::Killed))),
                    waker: Cell::new(None),
                }),
            };
        }
        let serial = self.send_call(path, destination, 0, &msg);
        let slot = Rc::new(AsyncReplySlot {
            data: Cell::new(None),
            waker: Cell::new(None),
        });
        self.reply_handlers
            .set(serial, Box::new(AsyncReplyHandler(slot.clone())));
        AsyncReply {
            socket: self.clone(),
            serial,
            slot,
        }
    }

    #[allow(dead_code)]
    pub fn get<T, F>(&self, destination: &str, path: &str, f: F)
    where
        T: Property,
        F: for<'b> FnOnce(Result<&<T::Type as DbusType<'static>>::Generic<'b>, DbusError>)
            + 'static,
    {
        let msg: Get<T::Type> = Get {
            interface_name: T::INTERFACE.into(),
            property_name: T::PROPERTY.into(),
            _phantom: PhantomData,
        };
        self.call(destination, path, msg, move |res| {
            f(res.map(|v| &v.value));
        });
    }

    pub fn get_async<T: Property>(
        self: &Rc<Self>,
        destination: &str,
        path: &str,
    ) -> AsyncProperty<T> {
        let msg: Get<T::Type> = Get {
            interface_name: T::INTERFACE.into(),
            property_name: T::PROPERTY.into(),
            _phantom: PhantomData,
        };
        AsyncProperty {
            reply: self.call_async(destination, path, msg),
        }
    }

    pub fn add_object(
        self: &Rc<Self>,
        object: impl Into<Cow<'static, str>>,
    ) -> Result<DbusObject, DbusError> {
        let object = object.into();
        let data = Rc::new(DbusObjectData {
            path: object.clone(),
            methods: Default::default(),
            properties: Default::default(),
        });
        match self.objects.lock().entry(object) {
            Entry::Occupied(_) => Err(DbusError::AlreadyHandled),
            Entry::Vacant(v) => {
                v.insert(data.clone());
                Ok(DbusObject {
                    socket: self.clone(),
                    data,
                })
            }
        }
    }

    pub fn handle_signal<T, F>(
        self: &Rc<Self>,
        sender: Option<&str>,
        path: Option<&str>,
        handler: F,
    ) -> Result<SignalHandler, DbusError>
    where
        T: Signal<'static>,
        F: for<'a> Fn(T::Generic<'a>) + 'static,
    {
        let mut rule = format!(
            "type='signal',interface='{}',member='{}'",
            T::INTERFACE,
            T::MEMBER
        );
        if let Some(sender) = sender {
            let _ = write!(rule, ",sender='{}'", sender);
        }
        if let Some(path) = path {
            let _ = write!(rule, ",path='{}'", path);
        }
        let shd: SignalHandlerData<T, _> = SignalHandlerData {
            path: path.map(|s| s.to_owned()),
            rule,
            handler,
            _phantom: Default::default(),
        };
        self.handle_signal_dyn(Rc::new(shd))
    }

    fn handle_signal_dyn(
        self: &Rc<Self>,
        handler: Rc<dyn SignalHandlerApi>,
    ) -> Result<SignalHandler, DbusError> {
        let mut sh = self.signal_handlers.borrow_mut();
        let entry = sh
            .entry((handler.interface(), handler.member()))
            .or_insert_with(|| InterfaceSignalHandlers {
                unconditional: Default::default(),
                conditional: Default::default(),
            });
        match handler.path() {
            Some(p) => match entry.conditional.entry(p.to_owned()) {
                Entry::Occupied(_) => return Err(DbusError::AlreadyHandled),
                Entry::Vacant(v) => {
                    v.insert(handler.clone());
                }
            },
            _ if entry.unconditional.is_some() => return Err(DbusError::AlreadyHandled),
            _ => entry.unconditional = Some(handler.clone()),
        }
        self.call(
            BUS_DEST,
            BUS_PATH,
            org::freedesktop::dbus::AddMatch {
                rule: handler.rule().into(),
            },
            {
                let slf = self.clone();
                move |res| {
                    if let Err(e) = res {
                        log::error!(
                            "{}: Could not register a signal handler: {}",
                            slf.bus_name,
                            ErrorFmt(e)
                        );
                    }
                }
            },
        );
        Ok(SignalHandler {
            socket: self.clone(),
            data: handler,
        })
    }

    pub(super) fn remove_signal_handler(self: &Rc<Self>, handler: &dyn SignalHandlerApi) {
        let mut sh = self.signal_handlers.borrow_mut();
        let mut entry = match sh.entry((handler.interface(), handler.member())) {
            Entry::Occupied(o) => o,
            Entry::Vacant(_) => return,
        };
        match handler.path() {
            Some(p) => {
                entry.get_mut().conditional.remove(p);
            }
            _ => entry.get_mut().unconditional = None,
        }
        if entry.get().unconditional.is_none() && entry.get().conditional.is_empty() {
            entry.remove();
        }
        self.call(
            BUS_DEST,
            BUS_PATH,
            org::freedesktop::dbus::RemoveMatch {
                rule: handler.rule().into(),
            },
            {
                let slf = self.clone();
                move |res| {
                    if let Err(e) = res {
                        log::error!(
                            "{}: Could not unregister a signal handler: {}",
                            slf.bus_name,
                            ErrorFmt(e)
                        );
                    }
                }
            },
        );
    }

    pub fn emit_signal<'a, T: Signal<'a>>(&self, path: &str, msg: &T) -> u32 {
        let (msg, serial) = self.format_signal(path, msg);
        self.bufio.send(msg);
        serial
    }

    pub fn send_error(&self, destination: &str, reply_serial: u32, msg: &str) -> u32 {
        let (msg, serial) = self.format_error(destination, reply_serial, msg);
        self.bufio.send(msg);
        serial
    }

    pub fn send_reply<'a, T: Message<'a>>(
        &self,
        destination: &str,
        reply_serial: u32,
        msg: &T,
    ) -> u32 {
        let (msg, serial) = self.format_reply(destination, reply_serial, msg);
        self.bufio.send(msg);
        serial
    }

    fn send_call<'a, T: Message<'a>>(
        &self,
        path: &str,
        destination: &str,
        flags: u8,
        msg: &T,
    ) -> u32 {
        let (msg, serial) = self.format_call(path, destination, flags, msg);
        self.bufio.send(msg);
        serial
    }

    fn format_signal<'a, T: Signal<'a>>(&self, path: &str, msg: &T) -> (BufIoMessage, u32) {
        self.format_generic(MSG_SIGNAL, Some(path), None, None, 0, msg, None, true, true)
    }

    fn format_error(&self, destination: &str, reply_serial: u32, msg: &str) -> (BufIoMessage, u32) {
        let em = ErrorMessage { msg: msg.into() };
        self.format_generic(
            MSG_ERROR,
            None,
            Some(reply_serial),
            Some(destination),
            0,
            &em,
            Some("jay.Error"),
            false,
            false,
        )
    }

    fn format_reply<'a, T: Message<'a>>(
        &self,
        destination: &str,
        reply_serial: u32,
        msg: &T,
    ) -> (BufIoMessage, u32) {
        self.format_generic(
            MSG_METHOD_RETURN,
            None,
            Some(reply_serial),
            Some(destination),
            0,
            msg,
            None,
            true,
            true,
        )
    }

    fn format_call<'a, T: Message<'a>>(
        &self,
        path: &str,
        destination: &str,
        flags: u8,
        msg: &T,
    ) -> (BufIoMessage, u32) {
        self.format_generic(
            MSG_METHOD_CALL,
            Some(path),
            None,
            Some(destination),
            flags,
            msg,
            None,
            true,
            true,
        )
    }

    fn format_generic<'a, T: Message<'a>>(
        &self,
        ty: u8,
        path: Option<&str>,
        reply_serial: Option<u32>,
        destination: Option<&str>,
        flags: u8,
        msg: &T,
        error_name: Option<&str>,
        include_interface: bool,
        include_member: bool,
    ) -> (BufIoMessage, u32) {
        let num_fds = msg.num_fds();
        let mut fds = Vec::with_capacity(num_fds as _);
        let serial = self.serial();
        let mut buf = self.bufio.buf();
        let mut fmt = Formatter::new(&mut fds, &mut buf);
        let interface = match include_interface {
            true => Some(T::INTERFACE),
            _ => None,
        };
        let member = match include_member {
            true => Some(T::MEMBER),
            _ => None,
        };
        self.format_header(
            &mut fmt,
            ty,
            flags,
            serial,
            reply_serial,
            path,
            error_name,
            interface,
            member,
            destination,
            T::SIGNATURE,
            num_fds,
        );
        let body_start = fmt.len();
        msg.marshal(&mut fmt);
        let body_len = (buf.len() - body_start) as u32;
        buf[4..8].copy_from_slice(uapi::as_bytes(&body_len));
        (
            BufIoMessage {
                fds,
                buf: buf.unwrap(),
            },
            serial,
        )
    }

    fn format_header(
        &self,
        fmt: &mut Formatter,
        ty: u8,
        flags: u8,
        serial: u32,
        reply_serial: Option<u32>,
        path: Option<&str>,
        error_name: Option<&str>,
        interface: Option<&str>,
        member: Option<&str>,
        destination: Option<&str>,
        signature: &str,
        fds: u32,
    ) {
        #[cfg(target_endian = "little")]
        b'l'.marshal(fmt);
        #[cfg(not(target_endian = "little"))]
        b'b'.marshal(fmt);
        ty.marshal(fmt);
        flags.marshal(fmt);
        1u8.marshal(fmt);
        0u32.marshal(fmt);
        serial.marshal(fmt);
        let mut headers = self.headers.borrow_mut();
        let mut headers = headers.take_as::<(u8, Variant)>();
        if let Some(path) = path {
            headers.push((HDR_PATH, Variant::ObjectPath(ObjectPath(path.into()))));
        }
        if let Some(interface) = interface {
            headers.push((HDR_INTERFACE, Variant::String(interface.into())));
        }
        if let Some(member) = member {
            headers.push((HDR_MEMBER, Variant::String(member.into())));
        }
        if let Some(error_name) = error_name {
            headers.push((HDR_ERROR_NAME, Variant::String(error_name.into())));
        }
        if let Some(destination) = destination {
            headers.push((HDR_DESTINATION, Variant::String(destination.into())));
        }
        if let Some(rs) = reply_serial {
            headers.push((HDR_REPLY_SERIAL, Variant::U32(rs)));
        }
        if signature.len() > 0 {
            headers.push((
                HDR_SIGNATURE,
                Variant::Signature(Signature(signature.into())),
            ));
        }
        if fds > 0 {
            headers.push((HDR_UNIX_FDS, Variant::U32(fds)));
        }
        fmt.write_array(&headers);
        fmt.pad_to(8);
    }
}

struct SyncReplyHandler<T, F>(F, PhantomData<T>);

unsafe impl<T, F> ReplyHandler for SyncReplyHandler<T, F>
where
    T: Message<'static>,
    F: for<'b> FnOnce(Result<&T::Generic<'b>, DbusError>),
{
    fn signature(&self) -> &str {
        T::SIGNATURE
    }

    fn handle_error(self: Box<Self>, _socket: &Rc<DbusSocket>, error: DbusError) {
        (self.0)(Err(error))
    }

    fn handle<'a>(
        self: Box<Self>,
        socket: &Rc<DbusSocket>,
        _headers: &Headers,
        parser: &mut Parser<'a>,
        buf: Vec<u8>,
    ) -> Result<(), DbusError> {
        let msg = <T::Generic<'a> as Message>::unmarshal(parser)?;
        (self.0)(Ok(&msg));
        socket.in_bufs.push(buf);
        Ok(())
    }
}

struct AsyncReplyHandler<T: Message<'static>>(Rc<AsyncReplySlot<T>>);

impl<T: Message<'static>> AsyncReplyHandler<T> {
    fn complete(self, res: Result<Reply<T>, DbusError>) {
        self.0.data.set(Some(res));
        if let Some(waker) = self.0.waker.take() {
            waker.wake();
        }
    }
}

unsafe impl<T> ReplyHandler for AsyncReplyHandler<T>
where
    T: Message<'static>,
{
    fn signature(&self) -> &str {
        T::SIGNATURE
    }

    fn handle_error(self: Box<Self>, _socket: &Rc<DbusSocket>, error: DbusError) {
        self.0.data.set(Some(Err(error)));
        if let Some(waker) = self.0.waker.take() {
            waker.wake();
        }
    }

    fn handle<'a>(
        self: Box<Self>,
        socket: &Rc<DbusSocket>,
        _headers: &Headers,
        parser: &mut Parser<'a>,
        buf: Vec<u8>,
    ) -> Result<(), DbusError> {
        let msg = <T::Generic<'static> as Message<'static>>::unmarshal(unsafe {
            mem::transmute::<&mut Parser<'a>, &mut Parser<'static>>(parser)
        });
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                let e = Rc::new(e);
                self.complete(Err(DbusError::DbusError(e.clone())));
                return Err(DbusError::DbusError(e.clone()));
            }
        };
        let reply = Reply {
            socket: socket.clone(),
            buf,
            t: msg,
        };
        self.complete(Ok(reply));
        Ok(())
    }
}
