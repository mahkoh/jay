use std::cell::Cell;
use {
    crate::{
        format::ARGB8888,
        portal::ptl_display::PortalDisplay,
        render::Framebuffer,
        utils::{
            clonecell::CloneCell, copyhashmap::CopyHashMap, errorfmt::ErrorFmt, numcell::NumCell,
        },
        video::{dmabuf::DmaBuf, gbm::GBM_BO_USE_RENDERING, ModifiedFormat, INVALID_MODIFIER},
        wl_usr::usr_ifs::{
            usr_linux_buffer_params::{UsrLinuxBufferParams, UsrLinuxBufferParamsOwner},
            usr_linux_dmabuf::UsrLinuxDmabuf,
            usr_wl_buffer::UsrWlBuffer,
        },
    },
    smallvec::SmallVec,
    std::{cell::RefCell, ops::Deref, rc::Rc},
};
use crate::fixed::Fixed;
use crate::ifs::wl_surface::zwlr_layer_surface_v1::KI_EXCLUSIVE;
use crate::ifs::zwlr_layer_shell_v1::OVERLAY;
use crate::portal::ptl_display::PortalOutput;
use crate::portal::ptl_render_ctx::PortalRenderCtx;
use crate::render::Renderer;
use crate::text;
use crate::theme::Color;
use crate::wire::WlSurfaceId;
use crate::wire::zwlr_layer_surface_v1::Configure;
use crate::wl_usr::usr_ifs::usr_wl_output::UsrWlOutput;
use crate::wl_usr::usr_ifs::usr_wl_surface::UsrWlSurface;
use crate::wl_usr::usr_ifs::usr_wlr_layer_surface::{UsrWlrLayerSurface, UsrWlrLayerSurfaceOwner};

pub struct SelectionGui {
    dpy: Rc<PortalDisplay>,
    surfaces: CopyHashMap<u32, Rc<SelectionGuiSurface>>,
}

pub struct SelectionGuiSurface {
    gui: Rc<SelectionGui>,
    output: Rc<PortalOutput>,
    ls: Rc<UsrWlrLayerSurface>,
    wl: Rc<UsrWlSurface>,
    bufs: [SelectionGuiBuf; NUM_BUFFERS],
}

pub struct SelectionGuiBuf {
    wl: Rc<UsrWlBuffer>,
    surface: CloneCell<Option<Rc<SelectionGuiSurface>>>,
    pub fb: Rc<Framebuffer>,
    free: Cell<bool>,
}

pub struct SelectionGuiBuilder {
    todo: NumCell<usize>,
    dpy: Rc<PortalDisplay>,
    outputs: Vec<Rc<PortalOutput>>,
    bufs: RefCell<Vec<Rc<SelectionGuiBufBuilder>>>,
    cb: Rc<dyn Fn(Option<Rc<SelectionGui>>)>,
}

struct SelectionGuiBufBuilder {
    builder: Rc<SelectionGuiBuilder>,
    wl: CloneCell<Option<Rc<UsrWlBuffer>>>,
    fb: Rc<Framebuffer>,
    params: Rc<UsrLinuxBufferParams>,
}

impl UsrWlrLayerSurfaceOwner for SelectionGuiSurface {
    fn configure(&self, _ev: &Configure) {
        self.wl.request_attach(&self.bufs[0].wl);
        self.wl.request_commit();
    }

    fn closed(&self) {
        // todo
    }
}

impl SelectionGuiBuilder {
    fn cancel_(&self, cb: bool) {
        for buf in self.bufs.borrow_mut().drain(..) {
            if let Some(wl) = buf.wl.take() {
                wl.con.remove_obj(wl.deref());
            }
            buf.params.con.remove_obj(buf.params.deref());
        }
        if cb {
            (self.cb)(None);
        }
    }

    fn complete(&self) {
        let mut bufs = self.bufs.borrow_mut();
        let gui = Rc::new(SelectionGui {
            dpy: self.dpy.clone(),
            surfaces: Default::default(),
        });
        macro_rules! buf {
            () => {{
                let buf = bufs.pop().unwrap();
                SelectionGuiBuf {
                    wl: buf.wl.get().unwrap(),
                    surface: Default::default(),
                    fb: buf.fb.clone(),
                    free: Cell::new(true),
                }
            }}
        }
        for output in &self.outputs {
            let wl = self.dpy.comp.create_surface();
            let ls = self.dpy.ls.get_layer_surface(&wl, &output.wl, OVERLAY);
            ls.request_set_size(WIDTH, HEIGHT);
            ls.request_set_keyboard_interactivity(KI_EXCLUSIVE);
            wl.request_commit();
            let sfc = Rc::new(SelectionGuiSurface {
                gui: gui.clone(),
                output: output.clone(),
                ls,
                wl,
                bufs: [buf!(), buf!()],
            });
            sfc.bufs[0].surface.set(Some(sfc.clone()));
            sfc.bufs[1].surface.set(Some(sfc.clone()));
            sfc.ls.owner.set(Some(sfc.clone()));
            gui.surfaces.set(output.linear_id.get(), sfc);
        }
        (self.cb)(Some(gui));
    }
}

impl UsrLinuxBufferParamsOwner for SelectionGuiBufBuilder {
    fn created(&self, buffer: Rc<UsrWlBuffer>) {
        buffer.con.add_object(buffer.clone());
        self.wl.set(Some(buffer));
        if self.builder.todo.fetch_sub(1) == 1 {
            self.builder.complete();
        }
    }

    fn failed(&self) {
        self.builder.cancel_(true);
    }
}

const NUM_BUFFERS: usize = 2;

const WIDTH: i32 = 800;
const HEIGHT: i32 = 600;

impl SelectionGui {
    pub fn build(
        dpy: &Rc<PortalDisplay>,
        cb: impl Fn(Option<Rc<SelectionGui>>) + 'static,
    ) -> Option<Rc<SelectionGuiBuilder>> {
        let dmabuf = dpy.dmabuf.get()?;
        let ctx = dpy.render_ctx.get()?;
        let num_buffers = NUM_BUFFERS * dpy.outputs.len();
        let builder = Rc::new(SelectionGuiBuilder {
            todo: NumCell::new(num_buffers),
            dpy: dpy.clone(),
            outputs: dpy.outputs.lock().values().cloned().collect(),
            bufs: RefCell::new(vec![]),
            cb: Rc::new(cb),
        });
        let mut bufs = vec![];
        if !Self::try_build(&builder, num_buffers, &dmabuf, &ctx, &mut bufs) {
            for buf in bufs {
                dpy.con.remove_obj(buf.params.deref());
            }
            return None;
        }
        *builder.bufs.borrow_mut() = bufs;
        Some(builder)
    }

    fn try_build(
        builder: &Rc<SelectionGuiBuilder>,
        num_buffers: usize,
        dmabuf: &UsrLinuxDmabuf,
        ctx: &PortalRenderCtx,
        bufs: &mut Vec<Rc<SelectionGuiBufBuilder>>,
    ) -> bool {
        for _ in 0..num_buffers  {
            let format = ModifiedFormat {
                format: ARGB8888,
                modifier: INVALID_MODIFIER,
            };
            let bo = match ctx
                .ctx
                .gbm
                .create_bo(WIDTH, HEIGHT, &format, GBM_BO_USE_RENDERING)
            {
                Ok(b) => b,
                Err(e) => {
                    log::error!("Could not allocate dmabuf: {}", ErrorFmt(e));
                    return false;
                }
            };
            let img = match ctx.ctx.dmabuf_img(bo.dmabuf()) {
                Ok(b) => b,
                Err(e) => {
                    log::error!("Could not import dmabuf into EGL: {}", ErrorFmt(e));
                    return false;
                }
            };
            let fb = match img.to_framebuffer() {
                Ok(b) => b,
                Err(e) => {
                    log::error!(
                        "Could not turns EGL image into framebuffer: {}",
                        ErrorFmt(e)
                    );
                    return false;
                }
            };
            fb.render_custom(Fixed::from_int(1), |r| {
                let white = Color::from_rgb(255, 255, 255);
                let black = Color::from_rgb(0, 0, 0);
                r.clear(&white);
                let tex = text::render_fitting(&ctx.ctx, 20, "monospace", "hello world", black, false, None).unwrap();
                r.render_texture(&tex, 0, 0, ARGB8888, None, None, Fixed::from_int(1));
            });
            // {
            //     ctx.ctx.ctx.with_current()
            //     fb.render()
            //     let renderer = fb.render()
            // }
            let params = dmabuf.create_params();
            params.request_create(bo.dmabuf());
            let buf = Rc::new(SelectionGuiBufBuilder {
                builder: builder.clone(),
                wl: Default::default(),
                fb,
                params: params.clone(),
            });
            buf.params.owner.set(Some(buf.clone()));
            bufs.push(buf);
        }
        true
    }
}
