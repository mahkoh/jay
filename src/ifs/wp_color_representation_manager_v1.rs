use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::wp_color_representation_surface_v1::{
            AM_PREMULTIPLIED_ELECTRICAL, AM_PREMULTIPLIED_OPTICAL, AM_STRAIGHT,
            WpColorRepresentationSurfaceV1, WpColorRepresentationSurfaceV1Error,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            WpColorRepresentationManagerV1Id,
            wp_color_representation_manager_v1::{
                Destroy, Done, GetSurface, SupportedAlphaMode,
                WpColorRepresentationManagerV1RequestHandler,
            },
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpColorRepresentationManagerV1Global {
    pub name: GlobalName,
}

impl WpColorRepresentationManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WpColorRepresentationManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpColorRepresentationManagerV1Error> {
        let mut supports_alpha_modes = false;
        if let Some(ctx) = client.state.render_ctx.get()
            && ctx.supports_alpha_modes()
        {
            supports_alpha_modes = true;
        }
        let obj = Rc::new(WpColorRepresentationManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            supports_alpha_modes,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        obj.send_capabilities();
        Ok(())
    }
}

pub struct WpColorRepresentationManagerV1 {
    pub id: WpColorRepresentationManagerV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
    pub supports_alpha_modes: bool,
}

impl WpColorRepresentationManagerV1 {
    fn send_capabilities(&self) {
        self.send_supported_alpha_mode(AM_PREMULTIPLIED_ELECTRICAL);
        if self.supports_alpha_modes {
            self.send_supported_alpha_mode(AM_PREMULTIPLIED_OPTICAL);
            self.send_supported_alpha_mode(AM_STRAIGHT);
        }
        self.send_done();
    }

    fn send_supported_alpha_mode(&self, alpha_mode: u32) {
        self.client.event(SupportedAlphaMode {
            self_id: self.id,
            alpha_mode,
        });
    }

    fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }
}

impl WpColorRepresentationManagerV1RequestHandler for WpColorRepresentationManagerV1 {
    type Error = WpColorRepresentationManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_surface(&self, req: GetSurface, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let obj = Rc::new(WpColorRepresentationSurfaceV1 {
            id: req.id,
            client: self.client.clone(),
            version: self.version,
            tracker: Default::default(),
            surface: surface.clone(),
            supports_alpha_modes: self.supports_alpha_modes,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        obj.install()?;
        Ok(())
    }
}

global_base!(
    WpColorRepresentationManagerV1Global,
    WpColorRepresentationManagerV1,
    WpColorRepresentationManagerV1Error
);

impl Global for WpColorRepresentationManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(WpColorRepresentationManagerV1Global);

object_base! {
    self = WpColorRepresentationManagerV1;
    version = self.version;
}

impl Object for WpColorRepresentationManagerV1 {}

simple_add_obj!(WpColorRepresentationManagerV1);

#[derive(Debug, Error)]
pub enum WpColorRepresentationManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    Surface(#[from] WpColorRepresentationSurfaceV1Error),
}
efrom!(WpColorRepresentationManagerV1Error, ClientError);
