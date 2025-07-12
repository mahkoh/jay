use {
    crate::{
        client::ClientError,
        ifs::head_management::{
            Head, HeadState,
            jay_head_manager_session_v1::{JayHeadManagerSessionV1, JayHeadManagerSessionV1Error},
            jay_head_manager_v1::JayHeadManagerV1,
            jay_head_v1::JayHeadV1,
        },
        object::{ObjectId, Version},
        state::ConnectorData,
        utils::clonecell::CloneCell,
    },
    std::rc::Rc,
};

macro_rules! error_ {
    ($error_name:ident, $($tt:tt)*) => {
        #[derive(Debug, thiserror::Error)]
        pub enum $error_name {
            #[error(transparent)]
            ClientError(Box<crate::client::ClientError>),
            #[error(transparent)]
            Common(#[from] super::super::HeadCommonError),
            $($tt)*
        }
        efrom!($error_name, ClientError, crate::client::ClientError);
    };
}

macro_rules! impl_head_ext {
    (
        $snake:ident,
        $camel:ident,
        $macro_name:ident,
        $mgr_module:ident,
        $mgr_name:ident,
        $mgr_id_name:ident,
        $head_module:ident,
        $head_name:ident,
        $head_id_name:ident,
        $version:expr,
        $(filter = $filter:ident,)?
        $(after_announce = $after_announce:ident,)?
        $(after_transaction = $after_transaction:ident,)?
    ) => {
        pub(in super::super) struct $mgr_name {
            pub(in super::super) id: crate::wire::$mgr_id_name,
            pub(in super::super) client: std::rc::Rc<crate::client::Client>,
            pub(in super::super) tracker: crate::leaks::Tracker<Self>,
            pub(in super::super) version: crate::object::Version,
            pub(in super::super) common: std::rc::Rc<super::super::HeadMgrCommon>,
        }

        impl $mgr_name {
            pub(in super::super) const VERSION: crate::object::Version = crate::object::Version($version);
            pub(in super::super) const NAME: &str = concat!("jay_head_manager_ext_", stringify!($snake));
        }

        object_base! {
            self = $mgr_name;
            version = self.version;
        }

        impl crate::object::Object for $mgr_name {}

        simple_add_obj!($mgr_name);

        pub(in super::super) struct $head_name {
            pub(in super::super) id: crate::wire::$head_id_name,
            pub(in super::super) client: std::rc::Rc<crate::client::Client>,
            pub(in super::super) tracker: crate::leaks::Tracker<Self>,
            pub(in super::super) version: crate::object::Version,
            pub(in super::super) common: std::rc::Rc<super::super::HeadCommon>,
        }

        object_base! {
            self = $head_name;
            version = self.version;
        }

        impl crate::object::Object for $head_name {}

        simple_add_obj!($head_name);

        impl $mgr_name {
            pub(in super::super) fn announce(
                &self,
                #[allow(clippy::allow_attributes, unused_variables)]
                connector: &crate::state::ConnectorData,
                common: &std::rc::Rc<super::super::HeadCommon>,
            ) -> Result<Option<std::rc::Rc<$head_name>>, crate::client::ClientError> {
                $(
                    if !self.$filter(connector, common) {
                        return Ok(None);
                    }
                )?
                let obj = std::rc::Rc::new($head_name {
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

            fn send_head(&self, obj: &$head_name) {
                self.client.event(crate::wire::$mgr_module::Head {
                    self_id: self.id,
                    head: obj.id,
                });
            }
        }

        impl $head_name {
            pub(in super::super) fn after_announce_wrapper(
                &self,
                #[allow(clippy::allow_attributes, unused_variables)]
                shared: &super::super::HeadState,
            ) {
                $(
                    self.$after_announce(shared);
                )?
            }

            pub(in super::super) fn after_transaction_wrapper(
                &self,
                #[allow(clippy::allow_attributes, unused_variables)]
                shared: &super::super::HeadState,
                #[allow(clippy::allow_attributes, unused_variables)]
                tran: &super::super::HeadState,
            ) {
                $(
                    self.$after_transaction(shared, tran);
                )?
            }
        }
    }
}

#[rustfmt::skip]
macro_rules! declare_extension_type_macro {
    (
        ($dollar:tt),
        $snake:ident,
        $camel:ident,
        $macro_name:ident,
        $macro_name_int:ident,
        $mgr_module:ident,
        $mgr_name:ident,
        $mgr_id_name:ident,
        $mgr_request_handler:ident,
        $head_module:ident,
        $head_name:ident,
        $head_id_name:ident,
        $head_request_handler:ident,
        $error_name:ident,
    ) => {
        macro_rules! $macro_name {
            (
                version = $version:expr,
                $dollar(filter = $filter:ident,)?
                $dollar(after_announce = $after_announce:ident,)?
                $dollar(after_transaction = $after_transaction:ident,)?
            ) => {
                $macro_name_int! {
                    ($),
                    version = $version,
                    $dollar(filter = $filter,)?
                    $dollar(after_announce = $after_announce,)?
                    $dollar(after_transaction = $after_transaction,)?
                }
            }
        }

        macro_rules! $macro_name_int {
            (
                ($dollar2:tt),
                version = $version:expr,
                $dollar(filter = $filter:ident,)?
                $dollar(after_announce = $after_announce:ident,)?
                $dollar(after_transaction = $after_transaction:ident,)?
            ) => {
                impl_head_ext!(
                    $snake,
                    $camel,
                    $macro_name,
                    $mgr_module,
                    $mgr_name,
                    $mgr_id_name,
                    $head_module,
                    $head_name,
                    $head_id_name,
                    $version,
                    $dollar(filter = $filter,)?
                    $dollar(after_announce = $after_announce,)?
                    $dollar(after_transaction = $after_transaction,)?
                );

                macro_rules! head_common_req {
                    () => {
                        head_common_req_!($snake);
                    }
                }

                macro_rules! mgr_common_req {
                    () => {
                        mgr_common_req_!($snake);
                    }
                }

                type ErrorName = $error_name;
                type MgrName = $mgr_name;
                type HeadName = $head_name;

                macro_rules! error {
                    ($dollar2($tt:tt)*) => {
                        error_!($error_name, $dollar2($tt)*);
                    }
                }
            }
        }
    };
}

macro_rules! mgr_common_req_ {
    ($snake:ident) => {
        with_builtin_macros::with_eager_expansions! {
            fn destroy(
                &self,
                _req: crate::wire::#{concat_idents!(jay_head_manager_ext_, $snake)}::Destroy,
                _slf: &Rc<Self>,
            ) -> Result<(), Self::Error> {
                self.common.assert_stopped()?;
                self.client.remove_obj(self)?;
                Ok(())
            }
        }
    };
}

macro_rules! head_common_req_ {
    ($snake:ident) => {
        with_builtin_macros::with_eager_expansions! {
            fn destroy(
                &self,
                _req: crate::wire::#{concat_idents!(jay_head_ext_, $snake)}::Destroy,
                _slf: &Rc<Self>,
            ) -> Result<(), Self::Error> {
                self.common.assert_removed()?;
                self.client.remove_obj(self)?;
                Ok(())
            }
        }
    };
}

macro_rules! declare_extensions {
    ($($snake:ident: $camel:ident,)*) => {
        #[derive(linearize::Linearize)]
        pub(super) enum HeadExtension {
            $($camel,)*
        }

        pub(super) fn send_available_extensions(mgr: &JayHeadManagerV1) {
            use linearize::Linearize;
            with_builtin_macros::with_eager_expansions! {
                $(
                    type $camel = super::jay_head_ext::#{concat_idents!(jay_head_ext_, $snake)}::#{concat_idents!(JayHeadManagerExt, $camel)};
                    mgr.send_extension(
                        HeadExtension::$camel.linearize() as _,
                        $camel::NAME,
                        $camel::VERSION,
                    );
                )*
            }
        }

        pub(super) fn announce_head(
            session: &Rc<JayHeadManagerSessionV1>,
            head: &Rc<JayHeadV1>,
            connector: &ConnectorData,
        ) -> Result<Rc<Head>, ClientError> {
            session.send_head_start(head, connector.head_managers.name);
            let head = super::Head {
                session: session.clone(),
                common: head.common.clone(),
                head: head.clone(),
                ext: HeadExts {
                    $(
                        $snake: match session.ext.$snake.get() {
                            Some(f) => f.announce(connector, &head.common)?,
                            _ => None,
                        },
                    )*
                },
            };
            session.send_head_complete();
            let shared = &*connector.head_managers.state.borrow();
            $(
                if let Some(ext) = &head.ext.$snake {
                    ext.after_announce_wrapper(shared);
                }
            )*
            Ok(Rc::new(head))
        }

        pub(super) fn bind_extension(session: &JayHeadManagerSessionV1, ext: HeadExtension, name: u32, version: u32, id: ObjectId) -> Result<(), JayHeadManagerSessionV1Error> {
            match ext {
                $(
                    HeadExtension::$camel => {
                        if session.ext.$snake.is_some() {
                            return Err(JayHeadManagerSessionV1Error::AlreadyBound(name));
                        }
                        let version = Version(version);
                        with_builtin_macros::with_eager_expansions! {
                            type T = super::jay_head_ext::#{concat_idents!(jay_head_ext_, $snake)}::#{concat_idents!(JayHeadManagerExt, $camel)};
                            if version > T::VERSION {
                                return Err(JayHeadManagerSessionV1Error::UnsupportedVersion(name, version));
                            }
                            let obj = Rc::new(T {
                                id: id.into(),
                                client: session.client.clone(),
                                tracker: Default::default(),
                                version,
                                common: session.common.clone(),
                            });
                        }
                        track!(session.client, obj);
                        session.client.add_client_obj(&obj)?;
                        session.ext.$snake.set(Some(obj));
                    }
                )*
            }
            Ok(())
        }

        with_builtin_macros::with_eager_expansions! {
            #[derive(Default)]
            pub(super) struct MgrExts {
                $(
                    pub(super) $snake: CloneCell<Option<Rc<super::jay_head_ext::#{concat_idents!(jay_head_ext_, $snake)}::#{concat_idents!(JayHeadManagerExt, $camel)}>>>,
                )*
            }

            pub(super) struct HeadExts {
                $(
                    pub(super) $snake: Option<Rc<super::jay_head_ext::#{concat_idents!(jay_head_ext_, $snake)}::#{concat_idents!(JayHeadExt, $camel)}>>,
                )*
            }
        }

        impl HeadExts {
            pub(super) fn after_transaction(&self, shared: &HeadState, tran: &HeadState) {
                $(
                    if let Some(ext) = &self.$snake {
                        ext.after_transaction_wrapper(shared, tran);
                    }
                )*
            }
        }

        with_builtin_macros::with_eager_expansions! {
            $(
                declare_extension_type_macro!(
                    ($),
                    $snake,
                    $camel,
                    #{concat_idents!(impl_, $snake)},
                    #{concat_idents!(impl_, $snake, _int)},
                    #{concat_idents!(jay_head_manager_ext_, $snake)},
                    #{concat_idents!(JayHeadManagerExt, $camel)},
                    #{concat_idents!(JayHeadManagerExt, $camel, Id)},
                    #{concat_idents!(JayHeadManagerExt, $camel, RequestHandler)},
                    #{concat_idents!(jay_head_ext_, $snake)},
                    #{concat_idents!(JayHeadExt, $camel)},
                    #{concat_idents!(JayHeadExt, $camel, Id)},
                    #{concat_idents!(JayHeadExt, $camel, RequestHandler)},
                    #{concat_idents!(JayHeadExt, $camel, Error)},
                );
            )*
        }
    };
}

declare_extensions! {
    core_info_v1: CoreInfoV1,
    compositor_space_info_v1: CompositorSpaceInfoV1,
    compositor_space_positioner_v1: CompositorSpacePositionerV1,
    compositor_space_transformer_v1: CompositorSpaceTransformerV1,
    compositor_space_scaler_v1: CompositorSpaceScalerV1,
    compositor_space_enabler_v1: CompositorSpaceEnablerV1,
    connector_info_v1: ConnectorInfoV1,
    mode_info_v1: ModeInfoV1,
    mode_setter_v1: ModeSetterV1,
    physical_display_info_v1: PhysicalDisplayInfoV1,
}
