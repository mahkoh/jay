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

macro_rules! object_base {
    ($oname:ident, $ename:ty; $($code:ident => $f:ident,)*) => {
        impl crate::object::ObjectBase for $oname {
            fn id(&self) -> crate::object::ObjectId {
                self.id.into()
            }

            #[allow(unused_variables, unreachable_code)]
            fn handle_request(
                self: std::rc::Rc<Self>,
                request: u32,
                parser: crate::utils::buffd::MsgParser<'_, '_>,
            ) -> Result<(), crate::client::ClientError> {
                fn handle_request(
                    slf: std::rc::Rc<$oname>,
                    request: u32,
                    parser: crate::utils::buffd::MsgParser<'_, '_>,
                ) -> Result<(), $ename> {
                    match request {
                        $(
                            $code => $oname::$f(&slf, parser)?,
                        )*
                        _ => unreachable!(),
                    }
                    Ok(())
                }
                if let Err(e) = handle_request(self, request, parser) {
                    return Err(crate::client::ClientError::ObjectError(e.into()));
                }
                Ok(())
            }

            fn interface(&self) -> crate::object::Interface {
                crate::wire::$oname
            }
        }

        impl From<$ename> for crate::client::ObjectError {
            fn from(v: $ename) -> Self {
                Self {
                    interface: crate::wire::$oname,
                    error: Box::new(v),
                }
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
                version: u32,
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

        #[allow(dead_code)]
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

macro_rules! linear_ids {
    ($ids:ident, $id:ident) => {
        pub struct $ids {
            next: crate::utils::numcell::NumCell<u32>,
        }

        impl Default for $ids {
            fn default() -> Self {
                Self {
                    next: crate::utils::numcell::NumCell::new(1),
                }
            }
        }

        impl $ids {
            pub fn next(&self) -> $id {
                $id(self.next.fetch_add(1))
            }
        }

        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
        pub struct $id(u32);

        impl $id {
            #[allow(dead_code)]
            pub fn raw(&self) -> u32 {
                self.0
            }

            #[allow(dead_code)]
            pub fn from_raw(id: u32) -> Self {
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
        pub struct $name(pub(super) u32);

        impl $name {
            pub fn raw(self) -> u32 {
                self.0
            }
        }

        pub const $uc: &[u32] = &[$($val,)*];

        $(
            pub const $name2: $name = $name($val);
        )*
    }
}

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
            #[allow(dead_code)]
            pub fn raw(&self) -> u32 {
                self.0
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
