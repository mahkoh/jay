macro_rules! efrom {
    ($ename:ty, $vname:ident) => {
        efrom!($ename, $vname, $vname);
    };
    ($ename:ty, $vname:ident, $sname:ty) => {
        impl From<$sname> for $ename {
            fn from(e: $sname) -> Self {
                Self::$vname(Box::new(e))
            }
        }
    };
}

macro_rules! usr_object_base {
    ($self:ident = $oname:ident = $iname:ident; version = $version:expr;) => {
        impl crate::wl_usr::usr_object::UsrObjectBase for $oname {
            fn id(&$self) -> crate::object::ObjectId {
                $self.id.into()
            }

            fn version(&$self) -> crate::object::Version {
                $version
            }

            fn handle_event(
                $self: std::rc::Rc<Self>,
                con: &crate::wl_usr::UsrCon,
                event: u32,
                parser: crate::utils::buffd::MsgParser<'_, '_>,
            ) -> Result<(), crate::wl_usr::UsrConError> {
                $self.handle_event_impl(con, event, parser)
            }

            fn interface(&$self) -> crate::object::Interface {
                crate::wire::$iname
            }
        }
    };
}

macro_rules! object_base {
    ($self:ident = $oname:ident; version = $version:expr;) => {
        impl crate::object::ObjectBase for $oname {
            fn id(&$self) -> crate::object::ObjectId {
                $self.id.into()
            }

            fn version(&$self) -> crate::object::Version {
                $version
            }

            fn handle_request(
                $self: std::rc::Rc<Self>,
                client: &crate::client::Client,
                request: u32,
                parser: crate::utils::buffd::MsgParser<'_, '_>,
            ) -> Result<(), crate::client::ClientError> {
                $self.handle_request_impl(client, request, parser)
            }

            fn interface(&$self) -> crate::object::Interface {
                crate::wire::$oname
            }
        }
    };
}

macro_rules! global_base {
    ($oname:ty, $ifname:ident, $ename:ty) => {
        impl crate::globals::GlobalBase for $oname {
            fn name(&self) -> crate::globals::GlobalName {
                self.name
            }

            fn bind<'a>(
                self: std::rc::Rc<Self>,
                client: &'a std::rc::Rc<crate::client::Client>,
                id: crate::object::ObjectId,
                version: crate::object::Version,
            ) -> Result<(), crate::globals::GlobalsError> {
                if let Err(e) = self.bind_(id.into(), client, version) {
                    return Err(crate::globals::GlobalsError::GlobalError(e.into()));
                }
                Ok(())
            }

            fn interface(&self) -> crate::object::Interface {
                crate::wire::$ifname
            }
        }

        impl From<$ename> for crate::globals::GlobalError {
            fn from(e: $ename) -> Self {
                Self {
                    interface: crate::wire::$ifname,
                    error: Box::new(e),
                }
            }
        }
    };
}

macro_rules! id {
    ($name:ident) => {
        #[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
        pub struct $name(u32);

        #[expect(dead_code)]
        impl $name {
            pub const NONE: Self = $name(0);

            pub const fn from_raw(raw: u32) -> Self {
                Self(raw)
            }

            pub fn raw(self) -> u32 {
                self.0
            }

            pub fn is_some(self) -> bool {
                self.0 != 0
            }

            pub fn is_none(self) -> bool {
                self.0 == 0
            }
        }

        impl From<crate::object::ObjectId> for $name {
            fn from(f: crate::object::ObjectId) -> Self {
                Self(f.raw())
            }
        }

        impl From<$name> for crate::object::ObjectId {
            fn from(f: $name) -> Self {
                crate::object::ObjectId::from_raw(f.0)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

macro_rules! shared_ids {
    ($id:ident) => {
        shared_ids!($id, u32);
    };
    ($id:ident, $ty:ty) => {
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
        pub struct $id($ty);

        impl $id {
            #[expect(dead_code)]
            pub fn raw(&self) -> $ty {
                self.0
            }

            #[expect(dead_code)]
            pub fn from_raw(id: $ty) -> Self {
                Self(id)
            }
        }

        impl From<$ty> for $id {
            fn from(id: $ty) -> Self {
                Self(id)
            }
        }

        impl std::fmt::Display for $id {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

macro_rules! linear_ids {
    ($(#[$attr1:meta])* $ids:ident, $id:ident $(,)?) => {
        linear_ids!($(#[$attr1])* $ids, $id, u32);
    };
    ($(#[$attr1:meta])* $ids:ident, $id:ident, $ty:ty $(,)?) => {
        #[derive(Debug)]
        pub struct $ids {
            next: crate::utils::numcell::NumCell<$ty>,
        }

        impl Default for $ids {
            fn default() -> Self {
                Self {
                    next: crate::utils::numcell::NumCell::new(1),
                }
            }
        }

        $(#[$attr1])*
        impl $ids {
            pub fn next(&self) -> $id {
                $id(self.next.fetch_add(1))
            }
        }

        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
        pub struct $id($ty);

        impl $id {
            #[allow(clippy::allow_attributes, dead_code)]
            pub fn raw(&self) -> $ty {
                self.0
            }

            #[allow(clippy::allow_attributes, dead_code)]
            pub fn from_raw(id: $ty) -> Self {
                Self(id)
            }
        }

        impl std::fmt::Display for $id {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

macro_rules! cenum {
    ($name:ident, $uc:ident; $($name2:ident = $val:expr,)*) => {
        #[derive(Copy, Clone, Debug, Eq, PartialEq)]
        pub struct $name(pub i32);

        impl $name {
            pub fn raw(self) -> i32 {
                self.0
            }
        }

        pub const $uc: &[i32] = &[$($val,)*];

        $(
            pub const $name2: $name = $name($val);
        )*
    }
}

#[expect(unused_macros)]
macro_rules! bitor {
    ($name:ident) => {
        impl std::ops::BitOr for $name {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }

        impl $name {
            pub fn contains(self, rhs: Self) -> bool {
                self.0 & rhs.0 == rhs.0
            }

            pub fn is_some(self) -> bool {
                self.0 != 0
            }
        }
    };
}

macro_rules! tree_id {
    ($id:ident) => {
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
        pub struct $id(u32);

        impl $id {
            #[allow(clippy::allow_attributes, dead_code)]
            pub fn raw(&self) -> u32 {
                self.0
            }

            #[allow(clippy::allow_attributes, dead_code)]
            pub fn none() -> Self {
                Self(0)
            }
        }

        impl std::fmt::Display for $id {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }

        impl From<crate::tree::NodeId> for $id {
            fn from(v: crate::tree::NodeId) -> $id {
                $id(v.0)
            }
        }

        impl From<$id> for crate::tree::NodeId {
            fn from(v: $id) -> crate::tree::NodeId {
                crate::tree::NodeId(v.0)
            }
        }

        impl PartialEq<crate::tree::NodeId> for $id {
            fn eq(&self, other: &crate::tree::NodeId) -> bool {
                self.0 == other.0
            }
        }

        impl PartialEq<$id> for crate::tree::NodeId {
            fn eq(&self, other: &$id) -> bool {
                self.0 == other.0
            }
        }
    };
}

macro_rules! dedicated_add_obj {
    ($oname:ident, $idname:ident, $field:ident) => {
        impl crate::client::WaylandObject for $oname {
            fn add(self: Rc<Self>, client: &crate::client::Client) {
                client.objects.$field.set(self.id.into(), self);
            }
            fn remove(&self, client: &crate::client::Client) {
                client.objects.$field.remove(&self.id.into());
            }
        }

        impl crate::client::WaylandObjectLookup for $idname {
            type Object = $oname;
            const INTERFACE: crate::object::Interface = crate::wire::$oname;

            fn lookup(client: &crate::client::Client, id: Self) -> Option<Rc<$oname>> {
                client.objects.$field.get(&id)
            }
        }
    };
}

macro_rules! simple_add_obj {
    ($ty:ty) => {
        impl crate::client::WaylandObject for $ty {}
    };
}

macro_rules! simple_add_global {
    ($ty:ty) => {
        impl crate::globals::WaylandGlobal for $ty {}
    };
}

macro_rules! dedicated_add_global {
    ($oname:ident, $field:ident) => {
        impl crate::globals::WaylandGlobal for $oname {
            fn add(self: Rc<Self>, globals: &crate::globals::Globals) {
                globals.$field.set(self.name, self);
            }
            fn remove(&self, globals: &crate::globals::Globals) {
                globals.$field.remove(&self.name);
            }
        }
    };
}

macro_rules! assert_size_eq {
    ($t:ty, $u:ty) => {{
        struct AssertEqSize<T, U>(std::marker::PhantomData<T>, std::marker::PhantomData<U>);
        impl<T, U> AssertEqSize<T, U> {
            const VAL: usize = {
                if size_of::<T>() != size_of::<U>() {
                    panic!("Types have different size");
                }
                1
            };
        }
        let _ = AssertEqSize::<$t, $u>::VAL;
    }};
}

#[expect(unused_macros)]
macro_rules! assert_size_le {
    ($t:ty, $u:ty) => {{
        struct AssertLeSize<T, U>(std::marker::PhantomData<T>, std::marker::PhantomData<U>);
        impl<T, U> AssertLeSize<T, U> {
            const VAL: usize = {
                if size_of::<T>() > size_of::<U>() {
                    panic!("Left type has size larger than right type");
                }
                1
            };
        }
        let _ = AssertLeSize::<$t, $u>::VAL;
    }};
}

macro_rules! assert_align_eq {
    ($t:ty, $u:ty) => {{
        struct AssertEqAlign<T, U>(std::marker::PhantomData<T>, std::marker::PhantomData<U>);
        impl<T, U> AssertEqAlign<T, U> {
            const VAL: usize = {
                if align_of::<T>() != align_of::<U>() {
                    panic!("Types have different alignment");
                }
                1
            };
        }
        let _ = AssertEqAlign::<$t, $u>::VAL;
    }};
}

macro_rules! atoms {
    {
        $name:ident;
        $($field_name:ident,)*
    } => {
        #[expect(non_snake_case, dead_code)]
        #[derive(Debug, Clone, Copy)]
        struct $name {
            $(
                $field_name: u32,
            )*
        }

        impl $name {
            fn get(
                conn: &std::rc::Rc<crate::xcon::Xcon>,
            ) -> impl std::future::Future<Output = Result<Self, crate::xcon::XconError>> {
                #![allow(non_snake_case)]
                use bstr::ByteSlice;
                $(
                    let $field_name = conn.call(&InternAtom {
                        only_if_exists: 0,
                        name: stringify!($field_name).as_bytes().as_bstr(),
                    });
                )*
                async move {
                    Ok(Self {
                        $(
                            $field_name: $field_name.await?.get().atom,
                        )*
                    })
                }
            }
        }
    }
}

macro_rules! fatal {
    ($($arg:tt)+) => {{
        log::error!($($arg)+);
        std::process::exit(1);
    }}
}

macro_rules! bitflags {
    ($name:ident: $rep:ty; $($var:ident = $val:expr,)*) => {
        #[derive(Copy, Clone, Eq, PartialEq, Default)]
        pub struct $name(pub $rep);

        $(
            #[allow(clippy::allow_attributes, dead_code)]
            pub const $var: $name = $name($val);
        )*

        #[allow(clippy::allow_attributes, dead_code)]
        impl $name {
            pub fn none() -> Self {
                Self(0)
            }

            pub fn is_some(self) -> bool {
                self.0 != 0
            }

            pub fn is_none(self) -> bool {
                self.0 == 0
            }

            pub fn all() -> Self {
                Self(0 $(| $val)*)
            }

            pub fn is_valid(self) -> bool {
                Self::all().contains(self)
            }

            pub fn contains(self, other: Self) -> bool {
                self.0 & other.0 == other.0
            }

            pub fn not_contains(self, other: Self) -> bool {
                self.0 & other.0 != other.0
            }

            pub fn intersects(self, other: Self) -> bool {
                self.0 & other.0 != 0
            }
        }

        impl std::ops::BitOr for $name {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }

        impl std::ops::BitAnd for $name {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self::Output {
                Self(self.0 & rhs.0)
            }
        }

        impl std::ops::BitOrAssign for $name {
            fn bitor_assign(&mut self, rhs: Self) {
                self.0 |= rhs.0;
            }
        }

        impl std::ops::BitAndAssign for $name {
            fn bitand_assign(&mut self, rhs: Self) {
                self.0 &= rhs.0;
            }
        }

        impl std::ops::BitXorAssign for $name {
            fn bitxor_assign(&mut self, rhs: Self) {
                self.0 ^= rhs.0;
            }
        }

        impl std::ops::Not for $name {
            type Output = Self;

            fn not(self) -> Self::Output {
                Self(!self.0)
            }
        }

        impl std::fmt::Debug for $name {
            #[allow(clippy::allow_attributes, clippy::bad_bit_mask)]
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut any = false;
                let mut v = self.0;
                $(
                    if $val != 0 && v & $val == $val {
                        if any {
                            write!(f, "|")?;
                        }
                        any = true;
                        write!(f, "{}", stringify!($var))?;
                        v &= !$val;
                    }
                )*
                if !any || v != 0 {
                    if any {
                        write!(f, "|")?;
                    }
                    write!(f, "0x{:x}", v)?;
                }
                Ok(())
            }
        }
    }
}

macro_rules! pw_opcodes {
    ($name:ident; $($var:ident = $val:expr,)*) => {
        #[derive(Copy, Clone, Debug)]
        pub enum $name {
            $(
                $var,
            )*
        }

        #[allow(clippy::allow_attributes, dead_code)]
        impl $name {
            pub fn from_id(id: u8) -> Option<Self> {
                let v = match id {
                    $($val => Self::$var,)*
                    _ => return None,
                };
                Some(v)
            }

            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$var => stringify!($var),)*
                }
            }
        }

        impl crate::pipewire::pw_object::PwOpcode for $name {
            fn id(&self) -> u8 {
                match self {
                    $(Self::$var => $val,)*
                }
            }
        }
    }
}

macro_rules! pw_object_base {
    ($name:ident, $if:expr, $events:ident; $($event:ident => $method:ident,)*) => {
        impl crate::pipewire::pw_object::PwObjectBase for $name {
            fn data(&self) -> &crate::pipewire::pw_object::PwObjectData {
                &self.data
            }

            fn interface(&self) -> &str {
                $if
            }

            fn handle_msg(self: std::rc::Rc<Self>, opcode: u8, parser: crate::pipewire::pw_parser::PwParser<'_>) -> Result<(), crate::pipewire::pw_object::PwObjectError> {
                match $events::from_id(opcode) {
                    None => Err(crate::pipewire::pw_object::PwObjectError {
                        interface: $if,
                        source: crate::pipewire::pw_object::PwObjectErrorType::UnknownEvent(opcode),
                    }),
                    Some(m) => {
                        let (res, method) = match m {
                            $(
                                $events::$event => (self.$method(parser), stringify!($event)),
                            )*
                        };
                        match res {
                            Ok(_) => Ok(()),
                            Err(source) => Err(crate::pipewire::pw_object::PwObjectError {
                                interface: $if,
                                source: crate::pipewire::pw_object::PwObjectErrorType::EventError {
                                    method,
                                    source: Box::new(source),
                                },
                            })
                        }
                    },
                }
            }

            fn event_name(&self, opcode: u8) -> Option<&'static str> {
                $events::from_id(opcode).map(|o| o.name())
            }
        }
    }
}

macro_rules! ei_id {
    ($name:ident) => {
        #[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
        pub struct $name(u64);

        #[expect(dead_code)]
        impl $name {
            pub const NONE: Self = $name(0);

            pub const fn from_raw(raw: u64) -> Self {
                Self(raw)
            }

            pub fn raw(self) -> u64 {
                self.0
            }

            pub fn is_some(self) -> bool {
                self.0 != 0
            }

            pub fn is_none(self) -> bool {
                self.0 == 0
            }
        }

        impl From<crate::ei::ei_object::EiObjectId> for $name {
            fn from(f: crate::ei::ei_object::EiObjectId) -> Self {
                Self(f.raw())
            }
        }

        impl From<$name> for crate::ei::ei_object::EiObjectId {
            fn from(f: $name) -> Self {
                crate::ei::ei_object::EiObjectId::from_raw(f.0)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }

        impl std::fmt::LowerHex for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::LowerHex::fmt(&self.0, f)
            }
        }
    };
}

macro_rules! ei_object_base {
    ($self:ident = $oname:ident; version = $version:expr;) => {
        impl crate::ei::ei_object::EiObjectBase for $oname {
            fn id(&$self) -> crate::ei::ei_object::EiObjectId {
                $self.id.into()
            }

            fn version(&$self) -> crate::ei::ei_object::EiVersion {
                $version
            }

            fn client(&$self) -> &crate::ei::ei_client::EiClient {
                &$self.client
            }

            fn handle_request(
                $self: std::rc::Rc<Self>,
                client: &crate::ei::ei_client::EiClient,
                request: u32,
                parser: crate::utils::buffd::EiMsgParser<'_, '_>,
            ) -> Result<(), crate::ei::ei_client::EiClientError> {
                $self.handle_request_impl(client, request, parser)
            }

            fn interface(&$self) -> crate::ei::ei_object::EiInterface {
                crate::wire_ei::$oname
            }
        }
    };
}

macro_rules! logical_to_client_wire_scale {
    ($client:expr, $($field:expr),+ $(,)?) => {
        #[expect(clippy::allow_attributes)]
        {
            #[allow(clippy::assign_op_pattern)]
            if let Some(scale) = $client.wire_scale.get() {
                $(
                    $field = $field * scale;
                )+
            }
        }
    };
}

macro_rules! client_wire_scale_to_logical {
    ($client:expr, $($field:expr),+ $(,)?) => {
        #[expect(clippy::allow_attributes)]
        {
            #[allow(clippy::assign_op_pattern)]
            if let Some(scale) = $client.wire_scale.get() {
                $(
                    $field = $field / scale;
                )+
            }
        }
    };
}

macro_rules! not_matches {
    ($($tt:tt)*) => {
        !matches!($($tt)*)
    };
}

macro_rules! jay_allow_realtime_config_so {
    () => {
        "JAY_ALLOW_REALTIME_CONFIG_SO"
    };
}
