use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        leaks::Tracker,
        object::Object,
        rect::Rect,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{
            zwlr_screencopy_manager_v1::*, WlOutputId, ZwlrScreencopyFrameV1Id,
            ZwlrScreencopyManagerV1Id,
        },
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ZwlrScreencopyManagerV1Global {
    pub name: GlobalName,
}

impl ZwlrScreencopyManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwlrScreencopyManagerV1Id,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), ZwlrScreencopyManagerV1Error> {
        let mgr = Rc::new(ZwlrScreencopyManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, mgr);
        client.add_client_obj(&mgr)?;
        Ok(())
    }
}

global_base!(
    ZwlrScreencopyManagerV1Global,
    ZwlrScreencopyManagerV1,
    ZwlrScreencopyManagerV1Error
);

simple_add_global!(ZwlrScreencopyManagerV1Global);

impl Global for ZwlrScreencopyManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        3
    }
}

pub struct ZwlrScreencopyManagerV1 {
    pub id: ZwlrScreencopyManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: u32,
}

impl ZwlrScreencopyManagerV1 {
    fn capture_output(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwlrScreencopyManagerV1Error> {
        let req: CaptureOutput = self.client.parse(self, parser)?;
        self.do_capture_output(req.output, req.overlay_cursor != 0, req.frame, None)
    }

    fn capture_output_region(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ZwlrScreencopyManagerV1Error> {
        let req: CaptureOutputRegion = self.client.parse(self, parser)?;
        let region = match Rect::new_sized(req.x, req.y, req.width, req.height) {
            Some(r) => r,
            _ => return Err(ZwlrScreencopyManagerV1Error::InvalidRegion),
        };
        self.do_capture_output(req.output, req.overlay_cursor != 0, req.frame, Some(region))
    }

    fn do_capture_output(
        &self,
        output: WlOutputId,
        overlay_cursor: bool,
        frame: ZwlrScreencopyFrameV1Id,
        region: Option<Rect>,
    ) -> Result<(), ZwlrScreencopyManagerV1Error> {
        let output = self.client.lookup(output)?;
        let mode = output.global.mode.get();
        let mut rect = Rect::new_sized(0, 0, mode.width, mode.height).unwrap();
        if let Some(region) = region {
            let scale = output.global.persistent.scale.get().to_f64();
            let x1 = (region.x1() as f64 * scale).round() as i32;
            let y1 = (region.y1() as f64 * scale).round() as i32;
            let x2 = (region.x2() as f64 * scale).round() as i32;
            let y2 = (region.y2() as f64 * scale).round() as i32;
            let region = Rect::new(x1, y1, x2, y2).unwrap();
            rect = rect.intersect(region);
        }
        let frame = Rc::new(ZwlrScreencopyFrameV1 {
            id: frame,
            client: self.client.clone(),
            tracker: Default::default(),
            output: output.global.clone(),
            rect,
            overlay_cursor,
            used: Cell::new(false),
            with_damage: Cell::new(false),
            output_link: Cell::new(None),
            buffer: Cell::new(None),
            version: self.version,
        });
        track!(self.client, frame);
        self.client.add_client_obj(&frame)?;
        frame.send_buffer();
        if self.version >= 3 {
            frame.send_linux_dmabuf();
            frame.send_buffer_done();
        }
        frame
            .output_link
            .set(Some(output.global.unused_captures.add_last(frame.clone())));
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), ZwlrScreencopyManagerV1Error> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwlrScreencopyManagerV1;

    CAPTURE_OUTPUT => capture_output,
    CAPTURE_OUTPUT_REGION => capture_output_region,
    DESTROY => destroy,
}

impl Object for ZwlrScreencopyManagerV1 {}

simple_add_obj!(ZwlrScreencopyManagerV1);

#[derive(Debug, Error)]
pub enum ZwlrScreencopyManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("The passed region is invalid")]
    InvalidRegion,
}
efrom!(ZwlrScreencopyManagerV1Error, ClientError);
efrom!(ZwlrScreencopyManagerV1Error, MsgParserError);
