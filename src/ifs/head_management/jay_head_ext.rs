macro_rules! head_ext {
    (
        mgr_path = $ext_path:ident,
        mgr = $ext_ty:ident,
        mgr_id = $ext_id:path,
        $(filter = $filter:ident,)?
        $(after_announce = $after_announce:ident,)?
        hed = $obj_ty:ident,
        hed_id = $obj_id:path,
        version = $version:expr,
    ) => {
        pub struct $ext_ty {
            pub id: $ext_id,
            pub client: std::rc::Rc<crate::client::Client>,
            pub tracker: crate::leaks::Tracker<Self>,
            pub version: crate::object::Version,
            pub common: std::rc::Rc<super::super::HeadMgrCommon>,
        }

        impl $ext_ty {
            pub const VERSION: crate::object::Version = crate::object::Version($version);
            pub const NAME: &str = stringify!($ext_path);
        }

        object_base! {
            self = $ext_ty;
            version = self.version;
        }

        impl crate::object::Object for $ext_ty {}

        simple_add_obj!($ext_ty);

        pub struct $obj_ty {
            pub id: $obj_id,
            pub client: std::rc::Rc<crate::client::Client>,
            pub tracker: crate::leaks::Tracker<Self>,
            pub version: crate::object::Version,
            pub common: std::rc::Rc<super::super::HeadCommon>,
        }

        object_base! {
            self = $obj_ty;
            version = self.version;
        }

        impl crate::object::Object for $obj_ty {}

        simple_add_obj!($obj_ty);

        impl $ext_ty {
            pub fn announce(
                &self,
                #[allow(unused_variables)]
                connector: &crate::state::ConnectorData,
                common: &std::rc::Rc<super::super::HeadCommon>,
            ) -> Result<Option<std::rc::Rc<$obj_ty>>, crate::client::ClientError> {
                $(
                    if !self.$filter(connector, common) {
                        return Ok(None);
                    }
                )?
                let obj = std::rc::Rc::new($obj_ty {
                    id: self.client.new_id()?,
                    client: self.client.clone(),
                    tracker: Default::default(),
                    version: self.version,
                    common: common.clone(),
                });
                track!(self.client, obj);
                self.client.add_server_obj(&obj);
                self.send_head(&obj);
                Ok(Some(obj))
            }

            fn send_head(&self, obj: &$obj_ty) {
                self.client.event(crate::wire::$ext_path::Head {
                    self_id: self.id,
                    head: obj.id,
                });
            }
        }

        impl $obj_ty {
            pub fn after_announce_wrapper(
                &self,
                #[allow(unused_variables)]
                connector: &crate::state::ConnectorData,
            ) {
                $(
                    self.$after_announce(connector);
                )?
            }
        }
    };
}

macro_rules! ext {
    (
        snake = $snake:ident,
        camel = $camel:ident,
        version = $version:expr,
        $(filter = $filter:ident,)?
        $(after_announce = $after_announce:ident,)?
    ) => {
        with_builtin_macros::with_eager_expansions! {
            head_ext! {
                mgr_path = #{ concat_idents!(jay_head_manager_ext_, $snake) },
                mgr = #{ concat_idents!(JayHeadManagerExt, $camel) },
                mgr_id = crate::wire::#{ concat_idents!(JayHeadManagerExt, $camel, Id) },
                $(filter = $filter,)?
                $(after_announce = $after_announce,)?
                hed = #{ concat_idents!(JayHeadExt, $camel) },
                hed_id = crate::wire::#{ concat_idents!(JayHeadExt, $camel, Id) },
                version = $version,
            }
        }
    };
}

macro_rules! ext_common_req {
    ($snake:ident) => {
        with_builtin_macros::with_eager_expansions! {
            fn destroy(
                &self,
                _req: crate::wire::#{ concat_idents!(jay_head_manager_ext_, $snake) }::Destroy,
                _slf: &Rc<Self>,
            ) -> Result<(), Self::Error> {
                self.common.assert_stopped()?;
                self.client.remove_obj(self)?;
                Ok(())
            }
        }
    };
}

macro_rules! head_common_req {
    ($snake:ident) => {
        with_builtin_macros::with_eager_expansions! {
            fn destroy(
                &self,
                _req: crate::wire::#{ concat_idents!(jay_head_ext_, $snake) }::Destroy,
                _slf: &Rc<Self>,
            ) -> Result<(), Self::Error> {
                self.common.assert_removed()?;
                self.client.remove_obj(self)?;
                Ok(())
            }
        }
    };
}

macro_rules! error {
    ($camel:ident$(, $($tt:tt)*)?) => {
        with_builtin_macros::with_eager_expansions! {
            #[derive(Debug, thiserror::Error)]
            pub enum #{concat_idents!(JayHeadExt, $camel, Error)} {
                #[error(transparent)]
                ClientError(Box<ClientError>),
                #[error(transparent)]
                Common(#[from] HeadCommonError),
                $($($tt)*)?
            }
            efrom!(#{concat_idents!(JayHeadExt, $camel, Error)}, ClientError);
        }
    };
}

macro_rules! tran {
    ($slf:expr, $req:expr, $tran:ident) => {
        let tran = $slf.client.lookup($req.transaction)?;
        let mut $tran = tran.tran.get_or_create($slf.common.name);
    };
}

pub mod jay_head_ext_compositor_space_info_v1;
pub mod jay_head_ext_compositor_space_positioner_v1;
pub mod jay_head_ext_compositor_space_transformer_v1;
pub mod jay_head_ext_connector_info_v1;
pub mod jay_head_ext_connector_settings_v1;
pub mod jay_head_ext_core_info_v1;
pub mod jay_head_ext_physical_display_info_v1;
