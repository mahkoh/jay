use crate::client::{Client, DynEventFormatter};
use crate::drm::INVALID_MODIFIER;
use crate::globals::{Global, GlobalName};
use crate::ifs::zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1;
use crate::object::{Interface, Object};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

mod types;

id!(ZwpLinuxDmabufV1Id);

const DESTROY: u32 = 0;
const CREATE_PARAMS: u32 = 1;

const FORMAT: u32 = 0;
const MODIFIER: u32 = 1;

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

bind!(ZwpLinuxDmabufV1Global);

impl Global for ZwpLinuxDmabufV1Global {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn singleton(&self) -> bool {
        true
    }

    fn interface(&self) -> Interface {
        Interface::ZwpLinuxDmabufV1
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
            obj: self.clone(),
            format,
        })
    }

    fn modifier(self: &Rc<Self>, format: u32, modifier: u64) -> DynEventFormatter {
        Box::new(Modifier {
            obj: self.clone(),
            format,
            modifier,
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
