use {
    crate::{
        allocator::{Allocator, AllocatorError, BufferObject, BufferUsage},
        cpu_worker::CpuWorker,
        format::{Format, ARGB8888, XRGB8888},
        gfx_api::{
            AcquireSync, AsyncShmGfxTexture, AsyncShmGfxTextureCallback, CopyTexture, FillRect,
            FramebufferRect, GfxApiOpt, GfxContext, GfxError, GfxFormat, GfxFramebuffer, GfxImage,
            GfxStagingBuffer, GfxTexture, GfxWriteModifier, PendingShmTransfer, ReleaseSync,
            ResetStatus, ShmGfxTexture, ShmMemory, SyncFile,
        },
        rect::{Rect, Region},
        theme::Color,
        video::{dmabuf::DmaBuf, drm::sync_obj::SyncObjCtx, LINEAR_MODIFIER},
    },
    ahash::AHashMap,
    indexmap::IndexSet,
    jay_config::video::GfxApi,
    std::{
        any::Any,
        cell::{Cell, RefCell},
        error::Error,
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
    MapDmaBuf(#[source] AllocatorError),
    #[error("Could not import dmabuf")]
    ImportDmaBuf(#[source] AllocatorError),
    #[error("Could not access the client memory")]
    AccessFailed(#[source] Box<dyn Error + Sync + Send>),
}

impl From<TestGfxError> for GfxError {
    fn from(value: TestGfxError) -> Self {
        Self(Box::new(value))
    }
}

pub struct TestGfxCtx {
    formats: Rc<AHashMap<u32, GfxFormat>>,
    allocator: Rc<dyn Allocator>,
}

impl TestGfxCtx {
    pub fn new(allocator: Rc<dyn Allocator>) -> Result<Rc<Self>, GfxError> {
        let mut modifiers = IndexSet::new();
        modifiers.insert(LINEAR_MODIFIER);
        let mut formats = AHashMap::new();
        for f in [XRGB8888, ARGB8888] {
            formats.insert(
                f.drm,
                GfxFormat {
                    format: f,
                    read_modifiers: modifiers.clone(),
                    write_modifiers: modifiers
                        .iter()
                        .copied()
                        .map(|m| {
                            (
                                m,
                                GfxWriteModifier {
                                    needs_render_usage: false,
                                },
                            )
                        })
                        .collect(),
                },
            );
        }
        Ok(Rc::new(Self {
            formats: Rc::new(formats),
            allocator,
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

    fn render_node(&self) -> Option<Rc<CString>> {
        None
    }

    fn formats(&self) -> Rc<AHashMap<u32, GfxFormat>> {
        self.formats.clone()
    }

    fn dmabuf_img(self: Rc<Self>, buf: &DmaBuf) -> Result<Rc<dyn GfxImage>, GfxError> {
        Ok(Rc::new(TestGfxImage::DmaBuf(TestDmaBufGfxImage {
            buf: buf.clone(),
            bo: self
                .allocator
                .import_dmabuf(buf, BufferUsage::none())
                .map_err(TestGfxError::ImportDmaBuf)?,
        })))
    }

    fn shmem_texture(
        self: Rc<Self>,
        _old: Option<Rc<dyn ShmGfxTexture>>,
        data: &[Cell<u8>],
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
        _damage: Option<&[Rect]>,
    ) -> Result<Rc<dyn ShmGfxTexture>, GfxError> {
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

    fn async_shmem_texture(
        self: Rc<Self>,
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
        _cpu_worker: &Rc<CpuWorker>,
    ) -> Result<Rc<dyn AsyncShmGfxTexture>, GfxError> {
        assert!(stride >= width * 4);
        let size = (stride * height) as usize;
        Ok(Rc::new(TestGfxImage::Shm(TestShmGfxImage {
            data: RefCell::new(vec![0; size]),
            width,
            height,
            stride,
            format,
        })))
    }

    fn allocator(&self) -> Rc<dyn Allocator> {
        self.allocator.clone()
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

    fn sync_obj_ctx(&self) -> Option<&Rc<SyncObjCtx>> {
        None
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
    bo: Rc<dyn BufferObject>,
}

impl TestGfxImage {
    fn read_pixels(&self, shm: &[Cell<u8>]) -> Result<(), GfxError> {
        let copy = |height: i32, stride: i32, src: *const u8, dst: *mut u8| unsafe {
            let size = (height * stride) as usize;
            assert!(shm.len() >= size);
            ptr::copy_nonoverlapping(src, dst, size);
        };
        match self {
            TestGfxImage::Shm(s) => {
                copy(
                    s.height,
                    s.stride,
                    s.data.borrow().as_ptr(),
                    shm.as_ptr() as _,
                );
            }
            TestGfxImage::DmaBuf(d) => {
                let map = d.bo.clone().map_read().map_err(TestGfxError::MapDmaBuf)?;
                unsafe {
                    copy(
                        d.buf.height,
                        map.stride(),
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

impl ShmGfxTexture for TestGfxImage {
    fn into_texture(self: Rc<Self>) -> Rc<dyn GfxTexture> {
        self
    }
}

impl AsyncShmGfxTexture for TestGfxImage {
    fn async_upload(
        self: Rc<Self>,
        _staging: &Rc<dyn GfxStagingBuffer>,
        _callback: Rc<dyn AsyncShmGfxTextureCallback>,
        mem: Rc<dyn ShmMemory>,
        _damage: Region,
    ) -> Result<Option<PendingShmTransfer>, GfxError> {
        let mut res = Ok(());
        mem.access(&mut |d| {
            res = self.clone().sync_upload(d, Region::default());
        })
        .map_err(TestGfxError::AccessFailed)?;
        res.map(|_| None)
    }

    fn sync_upload(self: Rc<Self>, mem: &[Cell<u8>], _damage: Region) -> Result<(), GfxError> {
        let TestGfxImage::Shm(shm) = &*self else {
            unreachable!();
        };
        let data = &mut *shm.data.borrow_mut();
        assert!(mem.len() >= data.len());
        unsafe {
            ptr::copy_nonoverlapping(mem.as_ptr() as _, data.as_mut_ptr(), data.len());
        }
        Ok(())
    }

    fn compatible_with(
        &self,
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
    ) -> bool {
        let TestGfxImage::Shm(shm) = &self else {
            unreachable!();
        };
        shm.format == format && shm.width == width && shm.height == height && shm.stride == stride
    }

    fn into_texture(self: Rc<Self>) -> Rc<dyn GfxTexture> {
        self
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
    fn physical_size(&self) -> (i32, i32) {
        match &*self.img {
            TestGfxImage::Shm(v) => (v.width, v.height),
            TestGfxImage::DmaBuf(v) => (v.buf.width, v.buf.height),
        }
    }

    fn render(
        &self,
        _acquire_sync: AcquireSync,
        _release_sync: ReleaseSync,
        ops: &[GfxApiOpt],
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
                        let map = d.bo.clone().map_read().map_err(TestGfxError::MapDmaBuf)?;
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
                let map = d.bo.clone().map_write().map_err(TestGfxError::MapDmaBuf)?;
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

    fn copy_to_shm(self: Rc<Self>, shm: &[Cell<u8>]) -> Result<(), GfxError> {
        self.img.deref().read_pixels(shm)
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
