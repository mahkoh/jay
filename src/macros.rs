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
                    self.bind_(id, client, version).await?;
                    Ok(())
                })
            }
        }
    };
}
