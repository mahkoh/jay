use crate::dbus::property::Get;
use crate::dbus::types::{ObjectPath, Signature, Variant};
use crate::dbus::{
    AsyncProperty, AsyncReply, AsyncReplySlot, DbusError, DbusMessage, DbusSocket, DbusType,
    Formatter, Headers, Message, MethodCall, Parser, Property, Reply, ReplyHandler,
    HDR_DESTINATION, HDR_INTERFACE, HDR_MEMBER, HDR_PATH, HDR_SIGNATURE, HDR_UNIX_FDS,
    MSG_METHOD_CALL, NO_REPLY_EXPECTED,
};
use std::cell::Cell;
use std::marker::PhantomData;
use std::mem;
use std::ops::DerefMut;
use std::rc::Rc;
use uapi::c;

impl DbusSocket {
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

    fn send_call<'a, T: Message<'a>>(
        &self,
        path: &str,
        destination: &str,
        flags: u8,
        msg: &T,
    ) -> u32 {
        let (msg, serial) = self.format_call(path, destination, flags, msg);
        self.outgoing.push(msg);
        serial
    }

    fn format_call<'a, T: Message<'a>>(
        &self,
        path: &str,
        destination: &str,
        flags: u8,
        msg: &T,
    ) -> (DbusMessage, u32) {
        let num_fds = msg.num_fds();
        let mut fds = Vec::with_capacity(num_fds as _);
        let serial = self.serial();
        let mut buf = self.bufs.pop().unwrap_or_default();
        buf.clear();
        let mut fmt = Formatter::new(&mut fds, &mut buf);
        self.format_header(
            &mut fmt,
            MSG_METHOD_CALL,
            flags,
            serial,
            path,
            T::INTERFACE,
            T::MEMBER,
            destination,
            T::SIGNATURE,
            num_fds,
        );
        let body_start = fmt.len();
        msg.marshal(&mut fmt);
        let body_len = (buf.len() - body_start) as u32;
        buf[4..8].copy_from_slice(uapi::as_bytes(&body_len));
        (DbusMessage { fds, buf }, serial)
    }

    fn format_header(
        &self,
        fmt: &mut Formatter,
        ty: u8,
        flags: u8,
        serial: u32,
        path: &str,
        interface: &str,
        member: &str,
        destination: &str,
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
        headers.push((HDR_PATH, Variant::ObjectPath(ObjectPath(path.into()))));
        headers.push((HDR_INTERFACE, Variant::String(interface.into())));
        headers.push((HDR_MEMBER, Variant::String(member.into())));
        headers.push((HDR_DESTINATION, Variant::String(destination.into())));
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
        socket.bufs.push(buf);
        Ok(())
    }
}

struct AsyncReplyHandler<T: Message<'static>>(Rc<AsyncReplySlot<T>>);

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
        })?;
        let reply = Reply {
            socket: socket.clone(),
            buf,
            t: msg,
        };
        self.0.data.set(Some(Ok(reply)));
        if let Some(waker) = self.0.waker.take() {
            waker.wake();
        }
        Ok(())
    }
}
