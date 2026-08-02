use crate::client::Client;
use crate::client::ClientError;
use crate::ifs::jay_client_match::JayClientMatch;
use crate::ifs::jay_generic_match_builder::JayGenericMatchBuilderError;
use crate::ifs::jay_generic_match_builder::MatchBuilder;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::wire::JayClientMatchBuilderId;
use crate::wire::jay_client_match_builder::*;
use std::rc::Rc;
use thiserror::Error;

pub struct JayClientMatchBuilder {
    pub id: JayClientMatchBuilderId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub builder: Rc<MatchBuilder<Rc<Client>>>,
}

macro_rules! push_str {
    ($slf:expr, $req:expr, $fun:ident) => {{
        $slf.builder
            .push_str($req.v, $req.regex, |v| $slf.builder.mgr.$fun(v))?;
        Ok(())
    }};
}

macro_rules! push_bool {
    ($slf:expr, $fun:ident) => {{
        $slf.builder.push($slf.builder.mgr.$fun());
        Ok(())
    }};
}

impl JayClientMatchBuilderRequestHandler for JayClientMatchBuilder {
    type Error = JayClientMatchBuilderError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get(&self, req: Get, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(JayClientMatch {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            m: self.builder.build()?,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }

    fn sandbox_engine(&self, req: SandboxEngine<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_str!(self, req, sandbox_engine)
    }

    fn sandbox_app_id(&self, req: SandboxAppId<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_str!(self, req, sandbox_app_id)
    }

    fn sandbox_instance_id(
        &self,
        req: SandboxInstanceId<'_>,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        push_str!(self, req, sandbox_instance_id)
    }

    fn sandboxed(&self, _req: Sandboxed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_bool!(self, sandboxed)
    }

    fn uid(&self, req: Uid, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.builder.push(self.builder.mgr.uid(req.v));
        Ok(())
    }

    fn pid(&self, req: Pid, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.builder.push(self.builder.mgr.pid(req.v));
        Ok(())
    }

    fn is_xwayland(&self, _req: IsXwayland, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_bool!(self, is_xwayland)
    }

    fn comm(&self, req: Comm<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_str!(self, req, comm)
    }

    fn exe(&self, req: Exe<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_str!(self, req, exe)
    }

    fn tag(&self, req: Tag<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_str!(self, req, tag)
    }
}

object_base! {
    self = JayClientMatchBuilder;
    version = self.version;
}

impl Object for JayClientMatchBuilder {}

simple_add_obj!(JayClientMatchBuilder);

#[derive(Debug, Error)]
pub enum JayClientMatchBuilderError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    Generic(#[from] JayGenericMatchBuilderError),
}
efrom!(JayClientMatchBuilderError, ClientError);
