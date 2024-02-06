use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::zwlr_layer_surface_v1::{ZwlrLayerSurfaceV1, ZwlrLayerSurfaceV1Error},
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{zwlr_layer_shell_v1::*, ZwlrLayerShellV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub const BACKGROUND: u32 = 0;
pub const BOTTOM: u32 = 1;
pub const TOP: u32 = 2;
pub const OVERLAY: u32 = 3;

pub struct ZwlrLayerShellV1Global {
    name: GlobalName,
}

pub struct ZwlrLayerShellV1 {
    pub id: ZwlrLayerShellV1Id,
    pub client: Rc<Client>,
    pub version: u32,
    pub tracker: Tracker<Self>,
}

impl ZwlrLayerShellV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwlrLayerShellV1Id,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), ZwlrLayerShellV1Error> {
        let obj = Rc::new(ZwlrLayerShellV1 {
            id,
            client: client.clone(),
            version,
            tracker: Default::default(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

impl ZwlrLayerShellV1 {
    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrLayerSurfaceV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_layer_surface(
        self: &Rc<Self>,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwlrLayerShellV1Error> {
        let req: GetLayerSurface = self.client.parse(&**self, parser)?;
        let surface = self.client.lookup(req.surface)?;
        let output = 'get_output: {
            if req.output.is_some() {
                self.client.lookup(req.output)?.global.node.get().unwrap()
            } else {
                for seat in self.client.state.seat_queue.rev_iter() {
                    let output = seat.get_output();
                    if !output.is_dummy {
                        break 'get_output output;
                    }
                }
                let outputs = self.client.state.outputs.lock();
                if let Some(output) = outputs.values().next() {
                    break 'get_output output.node.clone();
                }
                return Err(ZwlrLayerShellV1Error::NoOutputs);
            }
        };
        if req.layer > OVERLAY {
            return Err(ZwlrLayerShellV1Error::UnknownLayer(req.layer));
        }
        let surface = Rc::new(ZwlrLayerSurfaceV1::new(
            req.id,
            self,
            &surface,
            &output,
            req.layer,
            req.namespace,
        ));
        track!(self.client, surface);
        self.client.add_client_obj(&surface)?;
        surface.install()?;
        Ok(())
    }
}

global_base!(
    ZwlrLayerShellV1Global,
    ZwlrLayerShellV1,
    ZwlrLayerShellV1Error
);

impl Global for ZwlrLayerShellV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        4
    }
}

simple_add_global!(ZwlrLayerShellV1Global);

object_base! {
    self = ZwlrLayerShellV1;

    GET_LAYER_SURFACE => get_layer_surface,
    DESTROY => destroy if self.version >= 3,
}

simple_add_obj!(ZwlrLayerShellV1);

impl Object for ZwlrLayerShellV1 {}

#[derive(Debug, Error)]
pub enum ZwlrLayerShellV1Error {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Unknown layer {0}")]
    UnknownLayer(u32),
    #[error("There are no outputs")]
    NoOutputs,
    #[error(transparent)]
    ZwlrLayerSurfaceV1Error(Box<ZwlrLayerSurfaceV1Error>),
}
efrom!(ZwlrLayerShellV1Error, ClientError);
efrom!(ZwlrLayerShellV1Error, MsgParserError);
efrom!(ZwlrLayerShellV1Error, ZwlrLayerSurfaceV1Error);
