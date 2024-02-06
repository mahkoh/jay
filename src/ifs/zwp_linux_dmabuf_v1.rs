use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwp_linux_dmabuf_v1::*, ZwpLinuxDmabufV1Id},
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
        version: u32,
    ) -> Result<(), ZwpLinuxDmabufV1Error> {
        let obj = Rc::new(ZwpLinuxDmabufV1 {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        if let Some(ctx) = client.state.render_ctx.get() {
            let formats = ctx.formats();
            for format in formats.values() {
                if format.implicit_external_only && !ctx.supports_external_texture() {
                    continue;
                }
                obj.send_format(format.format.drm);
                if version >= MODIFIERS_SINCE_VERSION {
                    for modifier in format.modifiers.values() {
                        if modifier.external_only && !ctx.supports_external_texture() {
                            continue;
                        }
                        obj.send_modifier(format.format.drm, modifier.modifier);
                    }
                }
            }
        }
        Ok(())
    }
}

const MODIFIERS_SINCE_VERSION: u32 = 3;

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
        3
    }
}

simple_add_global!(ZwpLinuxDmabufV1Global);

pub struct ZwpLinuxDmabufV1 {
    id: ZwpLinuxDmabufV1Id,
    pub client: Rc<Client>,
    pub version: u32,
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
}

object_base! {
    self = ZwpLinuxDmabufV1;

    DESTROY => destroy,
    CREATE_PARAMS => create_params,
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
