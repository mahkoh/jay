use {
    crate::{
        cmm::cmm_transfer_function::TransferFunction,
        cpu_worker::{AsyncCpuWork, CpuJob, CpuWork, CpuWorker, PendingJob},
        format::ARGB8888,
        gfx_api::{
            AsyncShmGfxTexture, AsyncShmGfxTextureCallback, GfxContext, GfxError, GfxStagingBuffer,
            GfxTexture, PendingShmTransfer, STAGING_UPLOAD,
        },
        pango::{
            CairoContext, CairoImageSurface, PangoCairoContext, PangoError, PangoFontDescription,
            PangoLayout,
            consts::{
                CAIRO_FORMAT_ARGB32, CAIRO_OPERATOR_SOURCE, PANGO_ELLIPSIZE_END, PANGO_SCALE,
            },
        },
        rect::{Rect, Region},
        theme::Color,
        utils::{
            clonecell::CloneCell, double_buffered::DoubleBuffered, on_drop_event::OnDropEvent,
        },
    },
    std::{
        borrow::Cow,
        cell::{Cell, RefCell},
        mem,
        ops::Neg,
        rc::{Rc, Weak},
        sync::Arc,
    },
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum TextError {
    #[error("Could not create a cairo image")]
    CreateImage(#[source] PangoError),
    #[error("Could not create a cairo context")]
    CairoContext(#[source] PangoError),
    #[error("Could not create a pango context")]
    PangoContext(#[source] PangoError),
    #[error("Could not create a pango layout")]
    CreateLayout(#[source] PangoError),
    #[error("Could not access the cairo image data")]
    ImageData(#[source] PangoError),
    #[error("Texture upload failed")]
    Upload(#[source] GfxError),
    #[error("Could not create a texture")]
    CreateTexture(#[source] GfxError),
    #[error("Rendering is not scheduled or not yet completed")]
    NotScheduled,
}

impl<'a> Config<'a> {
    fn to_static(self) -> Config<'static> {
        match self {
            Config::None => Config::None,
            Config::RenderFitting {
                height,
                font,
                text,
                color,
                markup,
                scale,
            } => Config::RenderFitting {
                height,
                font,
                text: text.into_owned().into(),
                color,
                markup,
                scale,
            },
            Config::Render {
                x,
                y,
                width,
                height,
                padding,
                font,
                text,
                color,
                ellipsize,
                markup,
                scale,
            } => Config::Render {
                x,
                y,
                width,
                height,
                padding,
                font,
                text: text.into_owned().into(),
                color,
                ellipsize,
                markup,
                scale,
            },
        }
    }
}

struct Data {
    image: Rc<CairoImageSurface>,
    cctx: Rc<CairoContext>,
    _pctx: Rc<PangoCairoContext>,
    _fd: PangoFontDescription,
    layout: PangoLayout,
}

fn create_data(font: &str, width: i32, height: i32, scale: Option<f64>) -> Result<Data, TextError> {
    let image = match CairoImageSurface::new_image_surface(CAIRO_FORMAT_ARGB32, width, height) {
        Ok(s) => s,
        Err(e) => return Err(TextError::CreateImage(e)),
    };
    let cctx = match image.create_context() {
        Ok(c) => c,
        Err(e) => return Err(TextError::CairoContext(e)),
    };
    let pctx = match cctx.create_pango_context() {
        Ok(c) => c,
        Err(e) => return Err(TextError::PangoContext(e)),
    };
    let mut fd = PangoFontDescription::from_string(font);
    if let Some(scale) = scale {
        fd.set_size((fd.size() as f64 * scale).round() as _);
    }
    let layout = match pctx.create_layout() {
        Ok(l) => l,
        Err(e) => return Err(TextError::CreateLayout(e)),
    };
    layout.set_font_description(&fd);
    Ok(Data {
        image,
        cctx,
        _pctx: pctx,
        _fd: fd,
        layout,
    })
}

fn measure(
    font: &str,
    text: &str,
    markup: bool,
    scale: Option<f64>,
) -> Result<TextMeasurement, TextError> {
    let data = create_data(font, 1, 1, scale)?;
    if markup {
        data.layout.set_markup(text);
    } else {
        data.layout.set_text(text);
    }
    let mut res = TextMeasurement::default();
    res.ink_rect = data.layout.inc_pixel_rect();
    Ok(res)
}

fn render(
    x: i32,
    y: Option<i32>,
    width: i32,
    height: i32,
    padding: i32,
    font: &str,
    text: &str,
    color: Color,
    ellipsize: bool,
    markup: bool,
    scale: Option<f64>,
) -> Result<RenderedText, TextError> {
    if width == 0 || height == 0 {
        return Ok(RenderedText {
            width,
            height,
            stride: width * 4,
            data: vec![],
        });
    }
    let data = create_data(font, width, height, scale)?;
    if ellipsize {
        data.layout
            .set_width((width - 2 * padding).max(0) * PANGO_SCALE);
        data.layout.set_ellipsize(PANGO_ELLIPSIZE_END);
    }
    if markup {
        data.layout.set_markup(text);
    } else {
        data.layout.set_text(text);
    }
    let font_height = data.layout.pixel_size().1;
    let [r, g, b, a] = color.to_array(TransferFunction::Srgb);
    data.cctx.set_operator(CAIRO_OPERATOR_SOURCE);
    data.cctx.set_source_rgba(r as _, g as _, b as _, a as _);
    let y = y.unwrap_or((height - font_height) / 2);
    data.cctx.move_to(x as f64, y as f64);
    data.layout.show_layout();
    data.image.flush();
    Ok(RenderedText {
        width,
        height,
        stride: data.image.stride(),
        data: data.image.data().map_err(TextError::ImageData)?.to_vec(),
    })
}

fn render_fitting(
    height: Option<i32>,
    font: &str,
    text: &str,
    color: Color,
    markup: bool,
    scale: Option<f64>,
) -> Result<RenderedText, TextError> {
    let measurement = measure(font, text, markup, scale)?;
    let x = measurement.ink_rect.x1().neg();
    let y = match height {
        Some(_) => None,
        _ => Some(measurement.ink_rect.y1().neg()),
    };
    let width = measurement.ink_rect.width();
    let height = height.unwrap_or(measurement.ink_rect.height());
    render(
        x, y, width, height, 0, font, text, color, false, markup, scale,
    )
}

#[derive(Debug, Copy, Clone, Default)]
pub struct TextMeasurement {
    pub ink_rect: Rect,
}

struct RenderedText {
    width: i32,
    height: i32,
    stride: i32,
    data: Vec<Cell<u8>>,
}

#[derive(Default)]
struct RenderWork {
    config: Config<'static>,
    result: Option<Result<RenderedText, TextError>>,
}

struct RenderJob {
    work: RenderWork,
    data: Weak<Shared>,
}

impl CpuWork for RenderWork {
    fn run(&mut self) -> Option<Box<dyn AsyncCpuWork>> {
        self.result = Some(self.render());
        None
    }
}

impl RenderWork {
    fn render(&mut self) -> Result<RenderedText, TextError> {
        match self.config {
            Config::None => unreachable!(),
            Config::RenderFitting {
                height,
                ref font,
                ref text,
                color,
                markup,
                scale,
            } => render_fitting(height, font, text, color, markup, scale),
            Config::Render {
                x,
                y,
                width,
                height,
                padding,
                ref font,
                ref text,
                color,
                ellipsize,
                markup,
                scale,
            } => render(
                x, y, width, height, padding, font, text, color, ellipsize, markup, scale,
            ),
        }
    }
}

pub struct TextTexture {
    data: Rc<Shared>,
}

impl Drop for TextTexture {
    fn drop(&mut self) {
        if let Some(pending) = self.data.pending_render.take() {
            pending.detach();
        }
        self.data.pending_upload.take();
        self.data.render_job.take();
        self.data.waiter.take();
    }
}

struct Shared {
    cpu_worker: Rc<CpuWorker>,
    ctx: Rc<dyn GfxContext>,
    staging: CloneCell<Option<Rc<dyn GfxStagingBuffer>>>,
    textures: DoubleBuffered<TextBuffer>,
    pending_render: Cell<Option<PendingJob>>,
    pending_upload: Cell<Option<PendingShmTransfer>>,
    render_job: Cell<Option<Box<RenderJob>>>,
    result: Cell<Option<Result<(), TextError>>>,
    waiter: Cell<Option<Rc<dyn OnCompleted>>>,
    busy: Cell<bool>,
    flip_is_noop: Cell<bool>,
}

impl Shared {
    fn complete(&self, res: Result<(), TextError>) {
        if res.is_err() {
            self.textures.back().config.take();
        }
        self.busy.set(false);
        self.result.set(Some(res));
        if let Some(waiter) = self.waiter.take() {
            waiter.completed();
        }
    }
}

#[derive(PartialEq, Default)]
enum Config<'a> {
    #[default]
    None,
    RenderFitting {
        height: Option<i32>,
        font: Arc<String>,
        text: Cow<'a, str>,
        color: Color,
        markup: bool,
        scale: Option<f64>,
    },
    Render {
        x: i32,
        y: Option<i32>,
        width: i32,
        height: i32,
        padding: i32,
        font: Arc<String>,
        text: Cow<'a, str>,
        color: Color,
        ellipsize: bool,
        markup: bool,
        scale: Option<f64>,
    },
}

#[derive(Default)]
struct TextBuffer {
    config: RefCell<Config<'static>>,
    tex: CloneCell<Option<Rc<dyn AsyncShmGfxTexture>>>,
}

pub trait OnCompleted {
    fn completed(self: Rc<Self>);
}

impl TextTexture {
    pub fn new(cpu_worker: &Rc<CpuWorker>, ctx: &Rc<dyn GfxContext>) -> Self {
        let data = Rc::new(Shared {
            cpu_worker: cpu_worker.clone(),
            ctx: ctx.clone(),
            staging: Default::default(),
            textures: Default::default(),
            pending_render: Default::default(),
            pending_upload: Default::default(),
            render_job: Default::default(),
            result: Default::default(),
            waiter: Default::default(),
            busy: Default::default(),
            flip_is_noop: Default::default(),
        });
        Self { data }
    }

    pub fn texture(&self) -> Option<Rc<dyn GfxTexture>> {
        self.data
            .textures
            .front()
            .tex
            .get()
            .map(|t| t.into_texture())
    }

    fn apply_config(&self, on_completed: Rc<dyn OnCompleted>, config: Config<'_>) {
        if self.data.busy.replace(true) {
            unreachable!();
        }
        self.data.waiter.set(Some(on_completed));
        self.data.flip_is_noop.set(false);
        if *self.data.textures.front().config.borrow() == config {
            self.data.flip_is_noop.set(true);
            self.data.complete(Ok(()));
            return;
        }
        if *self.data.textures.back().config.borrow() == config {
            self.data.complete(Ok(()));
            return;
        }
        let mut job = self.data.render_job.take().unwrap_or_else(|| {
            Box::new(RenderJob {
                work: Default::default(),
                data: Rc::downgrade(&self.data),
            })
        });
        job.work = RenderWork {
            config: config.to_static(),
            result: None,
        };
        let pending = self.data.cpu_worker.submit(job);
        self.data.pending_render.set(Some(pending));
    }

    pub fn schedule_render(
        &self,
        on_completed: Rc<dyn OnCompleted>,
        x: i32,
        y: Option<i32>,
        width: i32,
        height: i32,
        padding: i32,
        font: &Arc<String>,
        text: &str,
        color: Color,
        ellipsize: bool,
        markup: bool,
        scale: Option<f64>,
    ) {
        let config = Config::Render {
            x,
            y,
            width,
            height,
            padding,
            font: font.clone(),
            text: Cow::Borrowed(text),
            color,
            ellipsize,
            markup,
            scale,
        };
        self.apply_config(on_completed, config)
    }

    pub fn schedule_render_fitting(
        &self,
        on_completed: Rc<dyn OnCompleted>,
        height: Option<i32>,
        font: &Arc<String>,
        text: &str,
        color: Color,
        markup: bool,
        scale: Option<f64>,
    ) {
        let config = Config::RenderFitting {
            height,
            font: font.clone(),
            text: text.into(),
            color,
            markup,
            scale,
        };
        self.apply_config(on_completed, config)
    }

    pub fn flip(&self) -> Result<(), TextError> {
        let res = self
            .data
            .result
            .take()
            .unwrap_or(Err(TextError::NotScheduled));
        if res.is_ok() && !self.data.flip_is_noop.get() {
            self.data.textures.flip();
        }
        res
    }
}

impl CpuJob for RenderJob {
    fn work(&mut self) -> &mut dyn CpuWork {
        &mut self.work
    }

    fn completed(mut self: Box<Self>) {
        let Some(data) = self.data.upgrade() else {
            return;
        };
        let result = self.work.result.take().unwrap();
        *data.textures.back().config.borrow_mut() = mem::take(&mut self.work.config);
        data.render_job.set(Some(self));
        let rt = match result {
            Ok(d) => d,
            Err(e) => {
                data.complete(Err(e));
                return;
            }
        };
        let mut tex = data.textures.back().tex.take();
        if rt.width == 0 || rt.height == 0 {
            data.complete(Ok(()));
            return;
        }
        if let Some(t) = &tex {
            if !t.compatible_with(ARGB8888, rt.width, rt.height, rt.stride) {
                tex = None;
            }
        }
        let tex = match tex {
            Some(t) => t,
            _ => {
                let tex = data
                    .ctx
                    .clone()
                    .async_shmem_texture(ARGB8888, rt.width, rt.height, rt.stride, &data.cpu_worker)
                    .map_err(TextError::CreateTexture);
                match tex {
                    Ok(t) => t,
                    Err(e) => {
                        data.complete(Err(e));
                        return;
                    }
                }
            }
        };
        let mut staging_opt = data.staging.take();
        if let Some(staging) = &staging_opt {
            if staging.size() != tex.staging_size() {
                staging_opt = None;
            }
        }
        let staging = match staging_opt {
            Some(s) => s,
            None => data
                .ctx
                .create_staging_buffer(tex.staging_size(), STAGING_UPLOAD),
        };
        let pending = tex
            .clone()
            .async_upload(
                &staging,
                data.clone(),
                Rc::new(rt.data),
                Region::new2(Rect::new_sized_unchecked(0, 0, rt.width, rt.height)),
            )
            .map_err(TextError::Upload);
        if pending.is_ok() {
            data.textures.back().tex.set(Some(tex));
            data.staging.set(Some(staging));
        }
        match pending {
            Ok(Some(p)) => data.pending_upload.set(Some(p)),
            Ok(None) => data.complete(Ok(())),
            Err(e) => data.complete(Err(e)),
        }
    }
}

impl AsyncShmGfxTextureCallback for Shared {
    fn completed(self: Rc<Self>, res: Result<(), GfxError>) {
        self.pending_upload.take();
        self.complete(res.map_err(TextError::Upload));
    }
}

impl OnCompleted for OnDropEvent {
    fn completed(self: Rc<Self>) {
        // nothing
    }
}
