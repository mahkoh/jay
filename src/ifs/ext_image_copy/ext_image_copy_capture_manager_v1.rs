use {
    crate::{
        client::{Client, ClientCaps, ClientError, CAP_SCREENCOPY_MANAGER},
        globals::{Global, GlobalName},
        ifs::{
            ext_image_capture_source_v1::ImageCaptureSource,
            ext_image_copy::{
                ext_image_copy_capture_cursor_session_v1::ExtImageCopyCaptureCursorSessionV1,
                ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
            },
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ext_image_copy_capture_manager_v1::*, ExtImageCopyCaptureManagerV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtImageCopyCaptureManagerV1Global {
    pub name: GlobalName,
}

impl ExtImageCopyCaptureManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ExtImageCopyCaptureManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ExtImageCopyCaptureManagerV1Error> {
        let obj = Rc::new(ExtImageCopyCaptureManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

pub struct ExtImageCopyCaptureManagerV1 {
    pub(super) id: ExtImageCopyCaptureManagerV1Id,
    pub(super) client: Rc<Client>,
    pub(super) tracker: Tracker<Self>,
    pub(super) version: Version,
}

impl ExtImageCopyCaptureManagerV1RequestHandler for ExtImageCopyCaptureManagerV1 {
    type Error = ExtImageCopyCaptureManagerV1Error;

    fn create_session(&self, req: CreateSession, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let source = self.client.lookup(req.source)?;
        let obj = Rc::new_cyclic(|slf| {
            ExtImageCopyCaptureSessionV1::new(
                req.session,
                &self.client,
                self.version,
                &source.ty,
                slf,
            )
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        'send_constraints: {
            let id = (self.client.id, obj.id);
            match &source.ty {
                ImageCaptureSource::Output(o) => {
                    let Some(node) = o.node() else {
                        obj.send_stopped();
                        break 'send_constraints;
                    };
                    node.ext_copy_sessions.set(id, obj.clone());
                }
                ImageCaptureSource::Toplevel(tl) => {
                    let Some(node) = tl.get() else {
                        obj.send_stopped();
                        break 'send_constraints;
                    };
                    let data = node.tl_data();
                    data.ext_copy_sessions.set(id, obj.clone());
                    if data.visible.get() {
                        obj.latch_listener.attach(&data.output().latch_event);
                    }
                }
            }
            let Some(ctx) = self.client.state.render_ctx.get() else {
                obj.send_stopped();
                break 'send_constraints;
            };
            obj.send_current_buffer_size();
            obj.send_shm_formats();
            if let Some(drm) = ctx.allocator().drm() {
                obj.send_dmabuf_device(drm.dev());
                for format in ctx.formats().values() {
                    if format.write_modifiers.is_empty() {
                        continue;
                    }
                    let modifiers: Vec<_> = format.write_modifiers.keys().copied().collect();
                    obj.send_dmabuf_format(format.format, &modifiers);
                }
            }
            obj.send_done();
        }
        Ok(())
    }

    fn create_pointer_cursor_session(
        &self,
        req: CreatePointerCursorSession,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let source = self.client.lookup(req.source)?;
        let obj = Rc::new(ExtImageCopyCaptureCursorSessionV1 {
            id: req.session,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            source: source.ty.clone(),
            have_session: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

global_base!(
    ExtImageCopyCaptureManagerV1Global,
    ExtImageCopyCaptureManagerV1,
    ExtImageCopyCaptureManagerV1Error
);

impl Global for ExtImageCopyCaptureManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_SCREENCOPY_MANAGER
    }
}

simple_add_global!(ExtImageCopyCaptureManagerV1Global);

object_base! {
    self = ExtImageCopyCaptureManagerV1;
    version = self.version;
}

impl Object for ExtImageCopyCaptureManagerV1 {}

simple_add_obj!(ExtImageCopyCaptureManagerV1);

#[derive(Debug, Error)]
pub enum ExtImageCopyCaptureManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ExtImageCopyCaptureManagerV1Error, ClientError);
