macro_rules! efrom {
    ($ename:ty, $vname:ident, $sname:ty) => {
        impl From<$sname> for $ename {
            fn from(e: $sname) -> Self {
                Self::$vname(Box::new(e))
            }
        }
    };
}

macro_rules! handle_request {
    ($oname:ty) => {
        impl crate::object::ObjectHandleRequest for $oname {
            fn handle_request<'a>(
                self: std::rc::Rc<Self>,
                request: u32,
                parser: crate::utils::buffd::MsgParser<'a, 'a>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), crate::client::ClientError>> + 'a>,
            > {
                Box::pin(async move {
                    self.handle_request_(request, parser).await?;
                    Ok(())
                })
            }
        }
    };
}

macro_rules! bind {
    ($oname:ty) => {
        impl crate::globals::GlobalBind for $oname {
            fn bind<'a>(
                self: std::rc::Rc<Self>,
                client: &'a std::rc::Rc<crate::client::Client>,
                id: crate::object::ObjectId,
                version: u32,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), crate::globals::GlobalError>> + 'a>,
            > {
                Box::pin(async move {
                    self.bind_(id.into(), client, version).await?;
                    Ok(())
                })
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

            pub fn from_raw(raw: u32) -> Self {
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
            pub fn raw(&self) -> u32 {
                self.0
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
    };
}
