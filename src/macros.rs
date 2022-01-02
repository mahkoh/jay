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
        impl crate::objects::ObjectHandleRequest for $oname {
            fn handle_request<'a>(
                &'a self,
                request: u32,
                parser: crate::utils::buffd::WlParser<'a, 'a>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), crate::objects::ObjectError>> + 'a>,
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
                client: &'a std::rc::Rc<crate::wl_client::WlClientData>,
                id: crate::objects::ObjectId,
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
