use crate::format::ARGB8888;
use crate::gfx_api::AcquireSync;
use crate::gfx_api::GfxFramebuffer;
use crate::gfx_api::GfxTexture;
use crate::gfx_api::ReleaseSync;
use crate::gfx_api::ScalingFilter;
use crate::ifs::wl_surface::WlSurface;
use crate::renderer::renderer_base::RenderTexture;
use crate::renderer::renderer_base::RendererBase;
use crate::scale::Scale;
use crate::state::GfxCtxChangedListener;
use crate::state::ScalesChangedListener;
use crate::theme::Color;
use crate::tree::Transform;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::event_listener::EventListener;
use crate::utils::lazy_event_source::LazyEventSourceListener;
use crate::utils::smallmap::SmallMapMut;
use crate::utils::type_view::TypeView;
use crate::utils::type_view::tv_wrap_weak;
use std::cell::Cell;
use std::cell::RefCell;
use std::mem;
use std::rc::Rc;

pub struct SurfaceRenderCache {
    surface: Rc<WlSurface>,
    _gfx_ctx_listener: EventListener<dyn GfxCtxChangedListener>,
    _commit_listener: EventListener<dyn LazyEventSourceListener>,
    _scale_listener: EventListener<dyn ScalesChangedListener>,
    size: Cell<[i32; 2]>,
    scaled: RefCell<SmallMapMut<Scale, Option<Scaled>, 2>>,
}

struct Scaled {
    need_render: bool,
    tex: Rc<dyn GfxTexture>,
    fb: Rc<dyn GfxFramebuffer>,
}

impl SurfaceRenderCache {
    #[expect(unused)]
    pub fn new(surface: &Rc<WlSurface>) -> Rc<Self> {
        let state = &surface.state;
        Rc::<Self>::new_cyclic(|slf| Self {
            surface: surface.clone(),
            _gfx_ctx_listener: EventListener::attached(slf.clone(), &state.gfx_ctx_changed),
            _scale_listener: EventListener::attached(slf.clone(), &state.scales_changed),
            _commit_listener: EventListener::attached(
                tv_wrap_weak::<_, CommitView>(slf.clone()),
                &surface
                    .tree_committed_listeners
                    .get_or_init(|| state.lazy_event_sources.create_source()),
            ),
            size: Default::default(),
            scaled: Default::default(),
        })
    }

    fn clear_tex(&self) {
        self.scaled.borrow_mut().clear();
    }

    #[expect(unused)]
    pub fn set_size(&self, width: i32, height: i32) {
        let size = [width, height];
        if self.size.replace(size) == size {
            return;
        }
        self.clear_tex();
    }

    #[expect(unused)]
    pub fn render(&self, renderer: &mut RendererBase, x: i32, y: i32, grayscale: bool) {
        let state = &self.surface.state;
        let scale = renderer.scale;
        let scales = &mut *self.scaled.borrow_mut();
        let scaled = scales.get_or_insert_with(scale, || {
            let [w, h] = scale.pixel_size(self.size.get());
            if w <= 0 || h <= 0 {
                return None;
            }
            let ctx = state.render_ctx.get()?;
            let img = ctx.create_read_write_img(&state.dma_buf_ids, w, h, ARGB8888);
            let (fb, tex) = match img {
                Ok(v) => v,
                Err(e) => {
                    log::error!("Could not allocate read/write image: {}", ErrorFmt(e));
                    return None;
                }
            };
            let scaled = Scaled {
                need_render: true,
                tex,
                fb,
            };
            Some(scaled)
        });
        let Some(scaled) = scaled else {
            return;
        };
        if mem::take(&mut scaled.need_render) {
            let mut rp = scaled.fb.create_render_pass(
                &*self.surface,
                state,
                None,
                scale,
                ScalingFilter::Linear,
                false,
                false,
                false,
                false,
                Transform::None,
                None,
                false,
            );
            rp.clear = Some(Color::TRANSPARENT);
            let cd = &self.surface.color_description();
            let res = scaled.fb.perform_render_pass(
                AcquireSync::Unnecessary,
                ReleaseSync::None,
                cd,
                &rp,
                &scaled.fb.full_region(),
                None,
                cd,
            );
            if let Err(e) = res {
                log::error!("Could not render surface icon: {}", ErrorFmt(e));
            }
        }
        let (x, y) = renderer.scale_point(x, y);
        renderer.render_texture(
            &scaled.tex,
            x,
            y,
            RenderTexture {
                acquire_sync: AcquireSync::None,
                release_sync: ReleaseSync::None,
                cd: Some(&self.surface.color_description()),
                grayscale,
                ..Default::default()
            },
        )
    }
}

impl GfxCtxChangedListener for SurfaceRenderCache {
    fn handle_gfx_context_change(self: Rc<Self>) {
        self.clear_tex();
    }
}

struct CommitView;
impl LazyEventSourceListener for TypeView<SurfaceRenderCache, CommitView> {
    fn triggered(self: Rc<Self>) {
        for (_, v) in self.scaled.borrow_mut().iter_mut() {
            if let Some(v) = v {
                v.need_render = true;
            }
        }
    }
}

impl ScalesChangedListener for SurfaceRenderCache {
    fn changed(self: Rc<Self>) {
        self.clear_tex();
    }
}
