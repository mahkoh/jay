use crate::dbus::types::{ObjectPath, Signature, Variant};
use crate::dbus::{
    DbusMessage, DbusSocket, DbusType, Formatter, Message, MethodCall, HDR_DESTINATION,
    HDR_INTERFACE, HDR_MEMBER, HDR_PATH, HDR_SIGNATURE, HDR_UNIX_FDS,
};

const MESSAGE_CALL: u8 = 1;
const MESSAGE_RETURN: u8 = 2;
const ERROR: u8 = 3;
const SIGNAL: u8 = 4;

impl DbusSocket {
    pub fn new() -> Self {
        todo!();
    }

    pub fn call_noreply<'a, T: MethodCall<'a>>(&self, destination: &str, path: &str, msg: T) {
        let (msg, _) = self.format_call(path, Some(destination), &msg);
        self.outgoing.push(msg);
    }

    fn format_call<'a, T: Message<'a>>(
        &self,
        path: &str,
        destination: Option<&str>,
        msg: &T,
    ) -> (DbusMessage, u32) {
        let num_fds = msg.num_fds();
        let mut fds = Vec::with_capacity(num_fds as _);
        let serial = self.next_serial.fetch_add(1);
        let mut buf = self.bufs.pop().unwrap_or_default();
        buf.clear();
        let mut fmt = Formatter::new(&mut fds, &mut buf);
        self.format_header(
            &mut fmt,
            MESSAGE_CALL,
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
        serial: u32,
        path: &str,
        interface: &str,
        member: &str,
        destination: Option<&str>,
        signature: &str,
        fds: u32,
    ) {
        #[cfg(target_endian = "little")]
        b'l'.marshal(fmt);
        #[cfg(not(target_endian = "little"))]
        b'b'.marshal(fmt);
        ty.marshal(fmt);
        0u8.marshal(fmt);
        1u8.marshal(fmt);
        0u32.marshal(fmt);
        serial.marshal(fmt);
        let mut headers = self.headers.borrow_mut();
        let mut headers = headers.take_as::<(u8, Variant)>();
        headers.push((HDR_PATH, Variant::ObjectPath(ObjectPath(path.into()))));
        headers.push((HDR_INTERFACE, Variant::String(interface.into())));
        headers.push((HDR_MEMBER, Variant::String(member.into())));
        if let Some(dst) = destination {
            headers.push((HDR_DESTINATION, Variant::String(dst.into())));
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
