use crate::client::Client;
use crate::client::ClientError;
use crate::criteria::CritLiteralOrRegex;
use crate::criteria::CritMgrExt;
use crate::criteria::CritTarget;
use crate::criteria::CritUpstreamNode;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::wire::JayGenericMatchBuilderId;
use crate::wire::jay_generic_match_builder::*;
use regex::Regex;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;

pub struct MatchBuilder<T>
where
    T: CritTarget,
{
    pub mgr: Rc<T::Mgr>,
    stack: RefCell<Vec<Vec<Rc<dyn CritUpstreamNode<T>>>>>,
}

pub trait MatchBuilderDyn: 'static {
    fn not(&self) -> Result<(), JayGenericMatchBuilderError>;
    fn nest(&self);
    fn list(&self, all: bool);
    fn exactly(&self, n: usize);
}

impl<T> MatchBuilder<T>
where
    T: CritTarget,
{
    pub fn new(mgr: &Rc<T::Mgr>) -> Self {
        Self {
            mgr: mgr.clone(),
            stack: Default::default(),
        }
    }

    pub fn build(&self) -> Result<Rc<dyn CritUpstreamNode<T>>, JayGenericMatchBuilderError> {
        let stack = &mut *self.stack.borrow_mut();
        let [crits] = &**stack else {
            return Err(JayGenericMatchBuilderError::NotUnique);
        };
        let [crit] = &**crits else {
            return Err(JayGenericMatchBuilderError::NotUnique);
        };
        Ok(crit.clone())
    }

    pub fn push(&self, crit: Rc<dyn CritUpstreamNode<T>>) {
        let stack = &mut *self.stack.borrow_mut();
        let mut last = stack.pop().unwrap_or_default();
        last.push(crit);
        stack.push(last);
    }

    pub fn push_str(
        &self,
        s: &str,
        regex: bool,
        build: impl FnOnce(CritLiteralOrRegex) -> Rc<dyn CritUpstreamNode<T>>,
    ) -> Result<(), JayGenericMatchBuilderError> {
        let v = match regex {
            false => CritLiteralOrRegex::Literal(s.to_string()),
            true => CritLiteralOrRegex::Regex(
                Regex::new(s).map_err(JayGenericMatchBuilderError::InvalidRegex)?,
            ),
        };
        self.push(build(v));
        Ok(())
    }
}

impl<T> MatchBuilderDyn for MatchBuilder<T>
where
    T: CritTarget,
{
    fn not(&self) -> Result<(), JayGenericMatchBuilderError> {
        let stack = &mut *self.stack.borrow_mut();
        if let Some(last) = stack.last_mut()
            && let Some(last) = last.last_mut()
        {
            *last = last.not(&self.mgr);
            Ok(())
        } else {
            Err(JayGenericMatchBuilderError::NoPreviousMatch)
        }
    }

    fn nest(&self) {
        let stack = &mut *self.stack.borrow_mut();
        stack.push(vec![]);
    }

    fn list(&self, all: bool) {
        let stack = &mut *self.stack.borrow_mut();
        let vec = stack.pop().unwrap_or_default();
        let mut last = stack.pop().unwrap_or_default();
        last.push(self.mgr.list(&vec, all));
        stack.push(last);
    }

    fn exactly(&self, n: usize) {
        let stack = &mut *self.stack.borrow_mut();
        let vec = stack.pop().unwrap_or_default();
        let mut last = stack.pop().unwrap_or_default();
        last.push(self.mgr.exactly(&vec, n));
        stack.push(last);
    }
}

pub struct JayGenericMatchBuilder {
    pub id: JayGenericMatchBuilderId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub builder: Rc<dyn MatchBuilderDyn>,
}

impl JayGenericMatchBuilder {
    pub fn create(
        id: JayGenericMatchBuilderId,
        client: &Rc<Client>,
        version: Version,
        builder: &Rc<impl MatchBuilderDyn>,
    ) -> Result<(), ClientError> {
        let slf = Rc::new(Self {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            builder: builder.clone(),
        });
        track!(client, slf);
        client.add_client_obj(&slf)?;
        Ok(())
    }
}

impl JayGenericMatchBuilderRequestHandler for JayGenericMatchBuilder {
    type Error = JayGenericMatchBuilderError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn not(&self, _req: Not, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.builder.not()
    }

    fn nest(&self, _req: Nest, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.builder.nest();
        Ok(())
    }

    fn list(&self, req: List, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.builder.list(req.all);
        Ok(())
    }

    fn exactly(&self, req: Exactly, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.builder.exactly(req.n);
        Ok(())
    }
}

object_base! {
    self = JayGenericMatchBuilder;
    version = self.version;
}

impl Object for JayGenericMatchBuilder {}

simple_add_obj!(JayGenericMatchBuilder);

#[derive(Debug, Error)]
pub enum JayGenericMatchBuilderError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("The stack is empty")]
    NoPreviousMatch,
    #[error("The builder does not contain exactly one element")]
    NotUnique,
    #[error("The regex is invalid")]
    InvalidRegex(#[source] regex::Error),
}
efrom!(JayGenericMatchBuilderError, ClientError);
