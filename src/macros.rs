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
    ($oname:ident; $($code:ident => $f:ident,)*) => {
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
                let res: Result<(), crate::client::MethodError> = match request {
                    $(
                        $code => $oname::$f(&self, parser).map_err(|e| crate::client::MethodError {
                            method: stringify!($f),
                            error: Box::new(e),
                        }),
                    )*
                    _ => unreachable!(),
                };
                if let Err(e) = res {
                    return Err(crate::client::ClientError::ObjectError(crate::client::ObjectError {
                        interface: crate::wire::$oname,
                        error: Box::new(e),
                    }));
                }
                Ok(())
            }

            fn interface(&self) -> crate::object::Interface {
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
        linear_ids!($ids, $id, u32);
    };
    ($ids:ident, $id:ident, $ty:ty) => {
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

        impl $ids {
            pub fn next(&self) -> $id {
                $id(self.next.fetch_add(1))
            }
        }

        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
        pub struct $id($ty);

        impl $id {
            #[allow(dead_code)]
            pub fn raw(&self) -> $ty {
                self.0
            }

            #[allow(dead_code)]
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
        pub struct $name(pub(super) i32);

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

macro_rules! assert_size_eq {
    ($t:ty, $u:ty) => {{
        struct AssertEqSize<T, U>(std::marker::PhantomData<T>, std::marker::PhantomData<U>);
        impl<T, U> AssertEqSize<T, U> {
            const VAL: usize = {
                if std::mem::size_of::<T>() != std::mem::size_of::<U>() {
                    panic!("Types have different size");
                }
                1
            };
        }
        let _ = AssertEqSize::<$t, $u>::VAL;
    }};
}

#[allow(unused_macros)]
macro_rules! assert_size_le {
    ($t:ty, $u:ty) => {{
        struct AssertLeSize<T, U>(std::marker::PhantomData<T>, std::marker::PhantomData<U>);
        impl<T, U> AssertLeSize<T, U> {
            const VAL: usize = {
                if std::mem::size_of::<T>() > std::mem::size_of::<U>() {
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
                if std::mem::align_of::<T>() != std::mem::align_of::<U>() {
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
        #[allow(non_snake_case, dead_code)]
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

macro_rules! tl_node_impl {
    () => {
        fn tl_as_node(&self) -> &dyn Node {
            self
        }

        fn tl_into_node(self: Rc<Self>) -> Rc<dyn Node> {
            self
        }

        fn tl_into_dyn(self: Rc<Self>) -> Rc<dyn ToplevelNode> {
            self
        }
    };
}

macro_rules! stacked_node_impl {
    () => {
        fn stacked_as_node(&self) -> &dyn Node {
            self
        }

        fn stacked_into_node(self: Rc<Self>) -> Rc<dyn Node> {
            self
        }

        fn stacked_into_dyn(self: Rc<Self>) -> Rc<dyn StackedNode> {
            self
        }
    };
}

macro_rules! containing_node_impl {
    () => {
        fn cnode_as_node(&self) -> &dyn Node {
            self
        }

        fn cnode_into_node(self: Rc<Self>) -> Rc<dyn Node> {
            self
        }

        fn cnode_into_dyn(self: Rc<Self>) -> Rc<dyn ContainingNode> {
            self
        }
    };
}
