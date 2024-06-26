use {
    crate::{
        format::{Format, ARGB8888, XRGB8888},
        gfx_api::{
            CopyTexture, FillRect, FramebufferRect, GfxApiOpt, GfxContext, GfxError, GfxFormat,
            GfxFramebuffer, GfxImage, GfxTexture, ResetStatus, SyncFile,
        },
        rect::Rect,
        theme::Color,
        video::{
            dmabuf::DmaBuf,
            drm::{sync_obj::SyncObjCtx, Drm, DrmError},
            gbm::{GbmBo, GbmDevice, GbmError},
            LINEAR_MODIFIER,
        },
    },
    ahash::AHashMap,
    indexmap::IndexSet,
    jay_config::video::GfxApi,
    std::{
        any::Any,
        cell::{Cell, RefCell},
        ffi::CString,
        fmt::{Debug, Formatter},
        ops::Deref,
        ptr,
        rc::Rc,
    },
    thiserror::Error,
};

#[derive(Error, Debug)]
enum TestGfxError {
    #[error("Could not map dmabuf")]
    MapDmaBuf(#[source] GbmError),
    #[error("Could not import dmabuf")]
    ImportDmaBuf(#[source] GbmError),
    #[error("Could not create a gbm device")]
    CreateGbmDevice(#[source] GbmError),
    #[error("Could not retrieve the render node path")]
    GetRenderNode(#[source] DrmError),
    #[error("Drm device does not have a render node")]
    NoRenderNode,
}

impl From<TestGfxError> for GfxError {
    fn from(value: TestGfxError) -> Self {
        Self(Box::new(value))
    }
}

pub struct TestGfxCtx {
    formats: Rc<AHashMap<u32, GfxFormat>>,
    sync_obj_ctx: Rc<SyncObjCtx>,
    gbm: GbmDevice,
    render_node: Rc<CString>,
}

impl TestGfxCtx {
    pub fn new(drm: &Drm) -> Result<Rc<Self>, GfxError> {
        let render_node = drm
            .get_render_node()
            .map_err(TestGfxError::GetRenderNode)?
            .ok_or(TestGfxError::NoRenderNode)?;
        let gbm = GbmDevice::new(drm).map_err(TestGfxError::CreateGbmDevice)?;
        let ctx = Rc::new(SyncObjCtx::new(drm.fd()));
        let mut modifiers = IndexSet::new();
        modifiers.insert(LINEAR_MODIFIER);
        let mut formats = AHashMap::new();
        for f in [XRGB8888, ARGB8888] {
            formats.insert(
                f.drm,
                GfxFormat {
                    format: f,
                    read_modifiers: modifiers.clone(),
                    write_modifiers: modifiers.clone(),
                },
            );
        }
        Ok(Rc::new(Self {
            formats: Rc::new(formats),
            sync_obj_ctx: ctx,
            gbm,
            render_node: Rc::new(render_node),
        }))
    }
}

impl Debug for TestGfxCtx {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestGfxCtx").finish_non_exhaustive()
    }
}

impl GfxContext for TestGfxCtx {
    fn reset_status(&self) -> Option<ResetStatus> {
        None
    }

    fn render_node(&self) -> Rc<CString> {
        self.render_node.clone()
    }

    fn formats(&self) -> Rc<AHashMap<u32, GfxFormat>> {
        self.formats.clone()
    }

    fn dmabuf_img(self: Rc<Self>, buf: &DmaBuf) -> Result<Rc<dyn GfxImage>, GfxError> {
        Ok(Rc::new(TestGfxImage::DmaBuf(TestDmaBufGfxImage {
            buf: buf.clone(),
            bo: self
                .gbm
                .import_dmabuf(buf, 0)
                .map(Rc::new)
                .map_err(TestGfxError::ImportDmaBuf)?,
        })))
    }

    fn shmem_texture(
        self: Rc<Self>,
        _old: Option<Rc<dyn GfxTexture>>,
        data: &[Cell<u8>],
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
        _damage: Option<&[Rect]>,
    ) -> Result<Rc<dyn GfxTexture>, GfxError> {
        assert!(stride >= width * 4);
        let size = (stride * height) as usize;
        assert!(data.len() >= size);
        let mut buf = vec![0; size];
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr() as _, buf.as_mut_ptr(), size);
        }
        Ok(Rc::new(TestGfxImage::Shm(TestShmGfxImage {
            data: RefCell::new(buf),
            width,
            height,
            stride,
            format,
        })))
    }

    fn gbm(&self) -> &GbmDevice {
        &self.gbm
    }

    fn gfx_api(&self) -> GfxApi {
        GfxApi::OpenGl
    }

    fn create_fb(
        self: Rc<Self>,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
    ) -> Result<Rc<dyn GfxFramebuffer>, GfxError> {
        assert!(stride >= width * 4);
        Ok(Rc::new(TestGfxFb {
            img: Rc::new(TestGfxImage::Shm(TestShmGfxImage {
                data: RefCell::new(vec![0; (stride * height) as usize]),
                width,
                height,
                stride,
                format,
            })),
            staging: RefCell::new(vec![Color::TRANSPARENT; (width * height) as usize]),
        }))
    }

    fn sync_obj_ctx(&self) -> &Rc<SyncObjCtx> {
        &self.sync_obj_ctx
    }
}

enum TestGfxImage {
    Shm(TestShmGfxImage),
    DmaBuf(TestDmaBufGfxImage),
}

struct TestGfxFb {
    img: Rc<TestGfxImage>,
    staging: RefCell<Vec<Color>>,
}

struct TestShmGfxImage {
    data: RefCell<Vec<u8>>,
    width: i32,
    height: i32,
    stride: i32,
    format: &'static Format,
}

struct TestDmaBufGfxImage {
    buf: DmaBuf,
    bo: Rc<GbmBo>,
}

impl TestGfxImage {
    fn read_pixels(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
        shm: &[Cell<u8>],
    ) -> Result<(), GfxError> {
        assert!(x >= 0);
        assert!(y >= 0);
        assert!(width >= 0);
        assert!(height >= 0);
        assert!(stride >= 0);
        assert!(x + width <= self.width());
        assert!(y + height <= self.height());
        assert!(stride >= width * 4);
        let size = (stride * height) as usize;
        assert!(shm.len() >= size);
        let copy = |src_stride: i32, src_format: &Format, mut src: *const u8, mut dst: *mut u8| unsafe {
            src = src.add((y * src_stride + x * 4) as usize);
            for _ in 0..height {
                ptr::copy_nonoverlapping(src, dst, (width * 4) as usize);
                if !src_format.has_alpha && format.has_alpha {
                    for i in 0..width {
                        *dst.add((i * 4 + 3) as usize) = 255;
                    }
                }
                src = src.add(src_stride as usize);
                dst = dst.add(stride as usize);
            }
        };
        match self {
            TestGfxImage::Shm(s) => {
                copy(
                    s.stride,
                    s.format,
                    s.data.borrow().as_ptr(),
                    shm.as_ptr() as _,
                );
            }
            TestGfxImage::DmaBuf(d) => {
                let map = d.bo.map_read().map_err(TestGfxError::MapDmaBuf)?;
                unsafe {
                    copy(
                        map.stride(),
                        d.buf.format,
                        map.data().as_ptr(),
                        shm.as_ptr() as _,
                    );
                }
            }
        }
        Ok(())
    }
}

impl Debug for TestGfxImage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestGfxTexture").finish_non_exhaustive()
    }
}

impl Debug for TestGfxFb {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestGfxFb").finish_non_exhaustive()
    }
}

impl GfxTexture for TestGfxImage {
    fn size(&self) -> (i32, i32) {
        match self {
            TestGfxImage::Shm(v) => (v.width, v.height),
            TestGfxImage::DmaBuf(v) => (v.buf.width, v.buf.height),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }

    fn read_pixels(
        self: Rc<Self>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
        shm: &[Cell<u8>],
    ) -> Result<(), GfxError> {
        self.deref()
            .read_pixels(x, y, width, height, stride, format, shm)
    }

    fn dmabuf(&self) -> Option<&DmaBuf> {
        match self {
            TestGfxImage::Shm(_) => None,
            TestGfxImage::DmaBuf(v) => Some(&v.buf),
        }
    }

    fn format(&self) -> &'static Format {
        &ARGB8888
    }
}

impl GfxImage for TestGfxImage {
    fn to_framebuffer(self: Rc<Self>) -> Result<Rc<dyn GfxFramebuffer>, GfxError> {
        Ok(Rc::new(TestGfxFb {
            staging: RefCell::new(vec![
                Color::TRANSPARENT;
                (self.width() * self.height()) as usize
            ]),
            img: self,
        }))
    }

    fn to_texture(self: Rc<Self>) -> Result<Rc<dyn GfxTexture>, GfxError> {
        Ok(self)
    }

    fn width(&self) -> i32 {
        match self {
            TestGfxImage::Shm(v) => v.width,
            TestGfxImage::DmaBuf(v) => v.buf.width,
        }
    }

    fn height(&self) -> i32 {
        match self {
            TestGfxImage::Shm(v) => v.height,
            TestGfxImage::DmaBuf(v) => v.buf.height,
        }
    }
}

impl GfxFramebuffer for TestGfxFb {
    fn take_render_ops(&self) -> Vec<GfxApiOpt> {
        vec![]
    }

    fn physical_size(&self) -> (i32, i32) {
        match &*self.img {
            TestGfxImage::Shm(v) => (v.width, v.height),
            TestGfxImage::DmaBuf(v) => (v.buf.width, v.buf.height),
        }
    }

    fn render(
        &self,
        ops: Vec<GfxApiOpt>,
        clear: Option<&Color>,
    ) -> Result<Option<SyncFile>, GfxError> {
        let fb_points = |width: i32, height: i32, rect: &FramebufferRect| {
            let points = rect.to_points();
            let x1 = points[1][0];
            let y1 = points[1][1];
            let x2 = points[2][0];
            let y2 = points[2][1];
            let x1 = (((x1 + 1.0) * width as f32 / 2.0).round() as i32)
                .max(0)
                .min(width);
            let x2 = (((x2 + 1.0) * width as f32 / 2.0).round() as i32)
                .max(0)
                .min(width);
            let y1 = (((y1 + 1.0) * height as f32 / 2.0).round() as i32)
                .max(0)
                .min(height);
            let y2 = (((y2 + 1.0) * height as f32 / 2.0).round() as i32)
                .max(0)
                .min(height);
            (x1, y1, x2, y2)
        };
        let apply = |data: *mut u8,
                     width: i32,
                     height: i32,
                     stride: i32,
                     format: &Format|
         -> Result<(), GfxError> {
            let copy_to_staging = |staging: &mut [Color]| match clear {
                Some(clear) => {
                    staging.fill(*clear);
                }
                None => unsafe {
                    let mut data = data;
                    for y in 0..height {
                        for x in 0..width {
                            let [b, g, r, mut a] = *data.add((x * 4) as usize).cast::<[u8; 4]>();
                            if !format.has_alpha {
                                a = 255;
                            }
                            staging[(y * width + x) as usize] =
                                Color::from_rgba_premultiplied(r, g, b, a);
                        }
                        data = data.add(stride as usize);
                    }
                },
            };
            let copy_from_staging = |staging: &mut [Color]| unsafe {
                let mut data = data;
                for y in 0..height {
                    for x in 0..width {
                        let [r, g, b, a] =
                            staging[(y * width + x) as usize].to_rgba_premultiplied();
                        *data.add((x * 4) as usize).cast::<[u8; 4]>() = [b, g, r, a];
                    }
                    data = data.add(stride as usize);
                }
            };
            let fill_rect = |f: &FillRect, staging: &mut [Color]| {
                let (x1, y1, x2, y2) = fb_points(width, height, &f.rect);
                for y in y1..y2 {
                    for x in x1..x2 {
                        let dst = &mut staging[(y * width + x) as usize];
                        *dst = dst.and_then(&f.color);
                    }
                }
            };
            let copy_texture = |c: &CopyTexture, staging: &mut [Color]| -> Result<(), GfxError> {
                let (fb_x1, fb_y1, fb_x2, fb_y2) = fb_points(width, height, &c.target);
                if fb_x1 >= fb_x2 || fb_y1 >= fb_y2 {
                    return Ok(());
                }
                let mut copy = |t_data: *const u8,
                                t_width: i32,
                                t_height: i32,
                                t_stride: i32,
                                t_format: &Format| unsafe {
                    if t_width == 0 || t_height == 0 {
                        return;
                    }
                    let points = c.source.to_points();
                    let t_x1 = points[1][0];
                    let t_y1 = points[1][1];
                    let t_x2 = points[2][0];
                    let t_y2 = points[2][1];
                    let nearest =
                        |fb_i: i32, fb_lo: i32, fb_hi: i32, t_lo: f32, t_hi: f32, t_size: i32| {
                            ((((fb_i - fb_lo) as f32 / (fb_hi - fb_lo) as f32 * (t_hi - t_lo)
                                + t_lo)
                                * t_size as f32)
                                .round() as i32)
                                .max(0)
                                .min(t_size - 1)
                        };
                    for f_y in fb_y1..fb_y2 {
                        let t_y = nearest(f_y, fb_y1, fb_y2, t_y1, t_y2, t_height);
                        for f_x in fb_x1..fb_x2 {
                            let t_x = nearest(f_x, fb_x1, fb_x2, t_x1, t_x2, t_width);
                            let [b, g, r, mut a] = *t_data
                                .add((t_y * t_stride + t_x * 4) as usize)
                                .cast::<[u8; 4]>();
                            if !t_format.has_alpha {
                                a = 255;
                            }
                            let mut color = Color::from_rgba_premultiplied(r, g, b, a);
                            if let Some(alpha) = c.alpha {
                                color = color * alpha;
                            }
                            let dst = &mut staging[(f_y * width + f_x) as usize];
                            *dst = dst.and_then(&color);
                        }
                    }
                };
                match c.tex.as_native() {
                    TestGfxImage::Shm(s) => copy(
                        s.data.borrow().as_ptr(),
                        s.width,
                        s.height,
                        s.stride,
                        s.format,
                    ),
                    TestGfxImage::DmaBuf(d) => {
                        let map = d.bo.map_read().map_err(TestGfxError::MapDmaBuf)?;
                        copy(
                            map.data_ptr(),
                            d.buf.width,
                            d.buf.height,
                            map.stride(),
                            d.buf.format,
                        );
                    }
                }
                Ok(())
            };
            let staging = &mut *self.staging.borrow_mut();
            copy_to_staging(staging);
            for op in ops {
                match op {
                    GfxApiOpt::Sync => {}
                    GfxApiOpt::FillRect(f) => fill_rect(&f, staging),
                    GfxApiOpt::CopyTexture(c) => copy_texture(&c, staging)?,
                }
            }
            copy_from_staging(staging);
            Ok(())
        };
        match &*self.img {
            TestGfxImage::Shm(s) => apply(
                s.data.borrow_mut().as_mut_ptr(),
                s.width,
                s.height,
                s.stride,
                s.format,
            )?,
            TestGfxImage::DmaBuf(d) => {
                let map = d.bo.map_write().map_err(TestGfxError::MapDmaBuf)?;
                apply(
                    map.data_ptr(),
                    d.buf.width,
                    d.buf.height,
                    map.stride(),
                    d.buf.format,
                )?;
            }
        }
        Ok(None)
    }

    fn copy_to_shm(
        self: Rc<Self>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        stride: i32,
        format: &'static Format,
        shm: &[Cell<u8>],
    ) -> Result<(), GfxError> {
        self.img
            .deref()
            .read_pixels(x, y, width, height, stride, format, shm)
    }

    fn format(&self) -> &'static Format {
        &ARGB8888
    }
}

impl dyn GfxTexture {
    fn as_native(&self) -> &TestGfxImage {
        self.as_any()
            .downcast_ref()
            .expect("Non-test texture passed into vulkan")
    }
}
