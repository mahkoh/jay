use {
    crate::{
        gfx_api::{
            BufferResv, CopyTexture, FillRect, FramebufferRect, GfxApiOpt, GfxTexture, SampleRect,
        },
        rect::Rect,
        scale::Scale,
        theme::Color,
        utils::transform_ext::TransformExt,
    },
    jay_config::video::Transform,
    std::rc::Rc,
};

pub struct RendererBase<'a> {
    pub ops: &'a mut Vec<GfxApiOpt>,
    pub scaled: bool,
    pub scale: Scale,
    pub scalef: f64,
    pub transform: Transform,
    pub fb_width: f32,
    pub fb_height: f32,
}

impl RendererBase<'_> {
    pub fn scale(&self) -> Scale {
        self.scale
    }

    pub fn scale_point(&self, mut x: i32, mut y: i32) -> (i32, i32) {
        if self.scaled {
            x = (x as f64 * self.scalef).round() as _;
            y = (y as f64 * self.scalef).round() as _;
        }
        (x, y)
    }

    pub fn scale_point_f(&self, mut x: f32, mut y: f32) -> (f32, f32) {
        if self.scaled {
            x = (x as f64 * self.scalef) as _;
            y = (y as f64 * self.scalef) as _;
        }
        (x, y)
    }

    pub fn scale_rect(&self, mut rect: Rect) -> Rect {
        if self.scaled {
            let x1 = (rect.x1() as f64 * self.scalef).round() as _;
            let y1 = (rect.y1() as f64 * self.scalef).round() as _;
            let x2 = (rect.x2() as f64 * self.scalef).round() as _;
            let y2 = (rect.y2() as f64 * self.scalef).round() as _;
            rect = Rect::new(x1, y1, x2, y2).unwrap();
        }
        rect
    }

    pub fn scale_rect_f(&self, mut rect: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
        if self.scaled {
            let x1 = (rect.0 as f64 * self.scalef).round() as _;
            let y1 = (rect.1 as f64 * self.scalef).round() as _;
            let x2 = (rect.2 as f64 * self.scalef).round() as _;
            let y2 = (rect.3 as f64 * self.scalef).round() as _;
            rect = (x1, y1, x2, y2)
        }
        rect
    }

    pub fn fill_boxes(&mut self, boxes: &[Rect], color: &Color) {
        self.fill_boxes2(boxes, color, 0, 0);
    }

    pub fn fill_boxes2(&mut self, boxes: &[Rect], color: &Color, dx: i32, dy: i32) {
        if boxes.is_empty() || *color == Color::TRANSPARENT {
            return;
        }
        let (dx, dy) = self.scale_point(dx, dy);
        for bx in boxes {
            let bx = self.scale_rect(*bx);
            self.ops.push(GfxApiOpt::FillRect(FillRect {
                rect: FramebufferRect::new(
                    (bx.x1() + dx) as f32,
                    (bx.y1() + dy) as f32,
                    (bx.x2() + dx) as f32,
                    (bx.y2() + dy) as f32,
                    self.transform,
                    self.fb_width,
                    self.fb_height,
                ),
                color: *color,
            }));
        }
    }

    pub fn fill_boxes_f(&mut self, boxes: &[(f32, f32, f32, f32)], color: &Color) {
        self.fill_boxes2_f(boxes, color, 0.0, 0.0);
    }

    pub fn fill_boxes2_f(
        &mut self,
        boxes: &[(f32, f32, f32, f32)],
        color: &Color,
        dx: f32,
        dy: f32,
    ) {
        if boxes.is_empty() || *color == Color::TRANSPARENT {
            return;
        }
        let (dx, dy) = self.scale_point_f(dx, dy);
        for bx in boxes {
            let (x1, y1, x2, y2) = self.scale_rect_f(*bx);
            self.ops.push(GfxApiOpt::FillRect(FillRect {
                rect: FramebufferRect::new(
                    x1 + dx,
                    y1 + dy,
                    x2 + dx,
                    y2 + dy,
                    self.transform,
                    self.fb_width,
                    self.fb_height,
                ),
                color: *color,
            }));
        }
    }

    pub fn render_texture(
        &mut self,
        texture: &Rc<dyn GfxTexture>,
        x: i32,
        y: i32,
        tpoints: Option<SampleRect>,
        tsize: Option<(i32, i32)>,
        tscale: Scale,
        bounds: Option<&Rect>,
        buffer_resv: Option<Rc<dyn BufferResv>>,
    ) {
        let mut texcoord = tpoints.unwrap_or_else(SampleRect::identity);

        let (twidth, theight) = if let Some(size) = tsize {
            size
        } else {
            let (mut w, mut h) = texcoord.buffer_transform.maybe_swap(texture.size());
            if tscale != self.scale {
                let tscale = tscale.to_f64();
                w = (w as f64 * self.scalef / tscale).round() as _;
                h = (h as f64 * self.scalef / tscale).round() as _;
            }
            (w, h)
        };

        let mut target_x = [x, x + twidth];
        let mut target_y = [y, y + theight];

        if let Some(bounds) = bounds {
            if bound_target(&mut target_x, &mut target_y, &mut texcoord, bounds) {
                return;
            }
        }

        let target = FramebufferRect::new(
            target_x[0] as f32,
            target_y[0] as f32,
            target_x[1] as f32,
            target_y[1] as f32,
            self.transform,
            self.fb_width,
            self.fb_height,
        );

        self.ops.push(GfxApiOpt::CopyTexture(CopyTexture {
            tex: texture.clone(),
            source: texcoord,
            target,
            buffer_resv,
        }));
    }
}

#[inline]
fn bound_target(
    target_x: &mut [i32; 2],
    target_y: &mut [i32; 2],
    texcoord: &mut SampleRect,
    bounds: &Rect,
) -> bool {
    let bounds_x = [bounds.x1(), bounds.x2()];
    let bounds_y = [bounds.y1(), bounds.y2()];

    if target_x[0] >= bounds_x[0]
        && target_x[1] <= bounds_x[1]
        && target_y[0] >= bounds_y[0]
        && target_y[1] <= bounds_y[1]
    {
        return false;
    }

    #[cold]
    fn cold() {}
    cold();

    let SampleRect {
        x1: ref mut t_x1,
        x2: ref mut t_x2,
        y1: ref mut t_y1,
        y2: ref mut t_y2,
        ..
    } = texcoord;

    macro_rules! clamp {
        ($desired:ident, $bounds:ident, $test_idx:expr, $test_cmp:ident, $test_cmp_eq:ident, $modify:ident, $keep:ident) => {{
            let desired_test = $desired[$test_idx];
            let desired_other = $desired[1 - $test_idx];
            let bound = $bounds[$test_idx];
            if desired_test.$test_cmp(&bound) {
                cold();
                if desired_other.$test_cmp_eq(&bound) {
                    return true;
                }
                let max = (desired_other - bound) as f32;
                let desired = ($desired[1] - $desired[0]) as f32;
                let factor = max.abs() / desired;
                *$modify = *$keep + (*$modify - *$keep) * factor;
                $desired[$test_idx] = bound;
            }
        }};
    }

    clamp!(target_x, bounds_x, 0, lt, le, t_x1, t_x2);
    clamp!(target_x, bounds_x, 1, gt, ge, t_x2, t_x1);
    clamp!(target_y, bounds_y, 0, lt, le, t_y1, t_y2);
    clamp!(target_y, bounds_y, 1, gt, ge, t_y2, t_y1);

    false
}
