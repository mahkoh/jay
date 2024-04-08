use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::{
            wl_surface::WlSurface, zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
            zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwp_linux_dmabuf_v1::*, ZwpLinuxDmabufFeedbackV1Id, ZwpLinuxDmabufV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpLinuxDmabufV1Global {
    name: GlobalName,
}

impl ZwpLinuxDmabufV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpLinuxDmabufV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZwpLinuxDmabufV1Error> {
        let obj = Rc::new(ZwpLinuxDmabufV1 {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        if version < FEEDBACK_SINCE_VERSION {
            if let Some(ctx) = client.state.render_ctx.get() {
                let formats = ctx.formats();
                for format in formats.values() {
                    obj.send_format(format.format.drm);
                    if version >= MODIFIERS_SINCE_VERSION {
                        for &modifier in &format.read_modifiers {
                            obj.send_modifier(format.format.drm, modifier);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

const MODIFIERS_SINCE_VERSION: Version = Version(3);
const FEEDBACK_SINCE_VERSION: Version = Version(4);

global_base!(
    ZwpLinuxDmabufV1Global,
    ZwpLinuxDmabufV1,
    ZwpLinuxDmabufV1Error
);

impl Global for ZwpLinuxDmabufV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        5
    }
}

simple_add_global!(ZwpLinuxDmabufV1Global);

pub struct ZwpLinuxDmabufV1 {
    id: ZwpLinuxDmabufV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub tracker: Tracker<Self>,
}

impl ZwpLinuxDmabufV1 {
    fn send_format(&self, format: u32) {
        self.client.event(Format {
            self_id: self.id,
            format,
        })
    }

    fn send_modifier(&self, format: u32, modifier: u64) {
        self.client.event(Modifier {
            self_id: self.id,
            format,
            modifier_hi: (modifier >> 32) as _,
            modifier_lo: modifier as _,
        })
    }

    fn destroy(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), ZwpLinuxDmabufV1Error> {
        let _req: Destroy = self.client.parse(&**self, parser)?;
        self.client.remove_obj(&**self)?;
        Ok(())
    }

    fn create_params(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwpLinuxDmabufV1Error> {
        let req: CreateParams = self.client.parse(&**self, parser)?;
        let params = Rc::new(ZwpLinuxBufferParamsV1::new(req.params_id, self));
        track!(self.client, params);
        self.client.add_client_obj(&params)?;
        Ok(())
    }

    fn get_feedback(
        self: &Rc<Self>,
        id: ZwpLinuxDmabufFeedbackV1Id,
        surface: Option<&Rc<WlSurface>>,
    ) -> Result<Rc<ZwpLinuxDmabufFeedbackV1>, ZwpLinuxDmabufV1Error> {
        let fb = Rc::new(ZwpLinuxDmabufFeedbackV1::new(id, &self.client, surface));
        track!(self.client, fb);
        self.client.add_client_obj(&fb)?;
        self.client
            .state
            .drm_feedback_consumers
            .set((self.client.id, id), fb.clone());
        if let Some(feedback) = self.client.state.drm_feedback.get() {
            fb.send_feedback(&feedback);
        }
        Ok(fb)
    }

    fn get_default_feedback(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwpLinuxDmabufV1Error> {
        let req: GetDefaultFeedback = self.client.parse(&**self, parser)?;
        self.get_feedback(req.id, None)?;
        Ok(())
    }

    fn get_surface_feedback(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwpLinuxDmabufV1Error> {
        let req: GetSurfaceFeedback = self.client.parse(&**self, parser)?;
        let surface = self.client.lookup(req.surface)?;
        let fb = self.get_feedback(req.id, Some(&surface))?;
        surface.drm_feedback.set(req.id, fb);
        Ok(())
    }
}

object_base! {
    self = ZwpLinuxDmabufV1;

    DESTROY => destroy,
    CREATE_PARAMS => create_params,
    GET_DEFAULT_FEEDBACK => get_default_feedback if self.version.0 >= 4,
    GET_SURFACE_FEEDBACK => get_surface_feedback if self.version.0 >= 4,
}

impl Object for ZwpLinuxDmabufV1 {}

simple_add_obj!(ZwpLinuxDmabufV1);

#[derive(Debug, Error)]
pub enum ZwpLinuxDmabufV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(ZwpLinuxDmabufV1Error, ClientError);
efrom!(ZwpLinuxDmabufV1Error, MsgParserError);
