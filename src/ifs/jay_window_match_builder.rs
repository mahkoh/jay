use crate::client::Client;
use crate::client::ClientError;
use crate::ifs::jay_generic_match_builder::JayGenericMatchBuilderError;
use crate::ifs::jay_generic_match_builder::MatchBuilder;
use crate::ifs::jay_generic_match_builder::MatchBuilderDyn;
use crate::ifs::jay_window_match::JayWindowMatch;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::tree::ToplevelData;
use crate::wire::JayWindowMatchBuilderId;
use crate::wire::jay_window_match_builder::*;
use jay_config::window::ContentType;
use jay_config::window::WindowType;
use std::rc::Rc;
use thiserror::Error;

pub struct JayWindowMatchBuilder {
    pub id: JayWindowMatchBuilderId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub builder: Rc<MatchBuilder<ToplevelData>>,
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

impl JayWindowMatchBuilderRequestHandler for JayWindowMatchBuilder {
    type Error = JayWindowMatchBuilderError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get(&self, req: Get, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let obj = Rc::new(JayWindowMatch {
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

    fn types(&self, req: Types, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.builder.push(self.builder.mgr.kind(WindowType(req.v)));
        Ok(())
    }

    fn client(
        &self,
        req: crate::wire::jay_window_match_builder::Client,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let cm = self.client.lookup(req.v)?;
        self.builder
            .push(self.builder.mgr.client(&self.client.state, &cm.m));
        Ok(())
    }

    fn title(&self, req: Title<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_str!(self, req, title)
    }

    fn app_id(&self, req: AppId<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_str!(self, req, app_id)
    }

    fn floating(&self, _req: Floating, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_bool!(self, floating)
    }

    fn visible(&self, _req: Visible, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_bool!(self, visible)
    }

    fn urgent(&self, _req: Urgent, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_bool!(self, urgent)
    }

    fn focused(&self, _req: Focused, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.builder.nest();
        for seat in self.client.state.globals.seats.lock().values() {
            self.builder.push(self.builder.mgr.seat_focus(seat));
        }
        self.builder.list(false);
        Ok(())
    }

    fn fullscreen(&self, _req: Fullscreen, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_bool!(self, fullscreen)
    }

    fn just_mapped(&self, _req: JustMapped, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_bool!(self, just_mapped)
    }

    fn tag(&self, req: Tag<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_str!(self, req, tag)
    }

    fn x_class(&self, req: XClass<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_str!(self, req, class)
    }

    fn x_instance(&self, req: XInstance<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_str!(self, req, instance)
    }

    fn x_role(&self, req: XRole<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_str!(self, req, role)
    }

    fn workspace(&self, req: Workspace<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        push_str!(self, req, workspace)
    }

    fn content_types(&self, req: ContentTypes, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.builder
            .push(self.builder.mgr.content_type(ContentType(req.v)));
        Ok(())
    }
}

object_base! {
    self = JayWindowMatchBuilder;
    version = self.version;
}

impl Object for JayWindowMatchBuilder {}

simple_add_obj!(JayWindowMatchBuilder);

#[derive(Debug, Error)]
pub enum JayWindowMatchBuilderError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    Generic(#[from] JayGenericMatchBuilderError),
}
efrom!(JayWindowMatchBuilderError, ClientError);
