use crate::client::{Client, ClientError, DynEventFormatter};
use crate::drm::INVALID_MODIFIER;
use crate::globals::{Global, GlobalName};
use crate::ifs::zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
use thiserror::Error;
use crate::wire::zwp_linux_dmabuf_v1::*;
use crate::utils::buffd::MsgParserError;
use crate::wire::ZwpLinuxDmabufV1Id;


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
            _version: version,
        });
        client.add_client_obj(&obj)?;
        if let Some(ctx) = client.state.render_ctx.get() {
            let formats = ctx.formats();
            for format in formats.values() {
                client.event(obj.format(format.drm));
                if version >= MODIFIERS_SINCE_VERSION {
                    client.event(obj.modifier(format.drm, INVALID_MODIFIER));
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
    _version: u32,
}

impl ZwpLinuxDmabufV1 {
    fn format(self: &Rc<Self>, format: u32) -> DynEventFormatter {
        Box::new(Format {
            self_id: self.id,
            format,
        })
    }

    fn modifier(self: &Rc<Self>, format: u32, modifier: u64) -> DynEventFormatter {
        Box::new(Modifier {
            self_id: self.id,
            format,
            modifier_hi: (modifier >> 32) as _,
            modifier_lo: modifier as _,
        })
    }

    fn destroy(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(&**self, parser)?;
        self.client.remove_obj(&**self)?;
        Ok(())
    }

    fn create_params(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), CreateParamsError> {
        let req: CreateParams = self.client.parse(&**self, parser)?;
        let params = Rc::new(ZwpLinuxBufferParamsV1::new(req.params_id, self));
        self.client.add_client_obj(&params)?;
        Ok(())
    }
}

object_base! {
    ZwpLinuxDmabufV1, ZwpLinuxDmabufV1Error;

    DESTROY => destroy,
    CREATE_PARAMS => create_params,
}

impl Object for ZwpLinuxDmabufV1 {
    fn num_requests(&self) -> u32 {
        CREATE_PARAMS + 1
    }
}

simple_add_obj!(ZwpLinuxDmabufV1);

#[derive(Debug, Error)]
pub enum ZwpLinuxDmabufV1Error {
    #[error("Could not process a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process a `create_params` request")]
    CreateParamsError(#[from] CreateParamsError),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpLinuxDmabufV1Error, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ClientError);
efrom!(DestroyError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum CreateParamsError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(CreateParamsError, ClientError);
efrom!(CreateParamsError, ParseError, MsgParserError);
