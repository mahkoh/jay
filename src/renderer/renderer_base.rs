use {
    crate::{
        gfx_api::{
            AbsoluteRect, BufferPoint, BufferPoints, CopyTexture, FillRect, GfxApiOpt, GfxTexture,
        },
        rect::Rect,
        scale::Scale,
        theme::Color,
    },
    std::rc::Rc,
};

pub struct RendererBase<'a> {
    pub ops: &'a mut Vec<GfxApiOpt>,
    pub scaled: bool,
    pub scale: Scale,
    pub scalef: f64,
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
        if boxes.is_empty() {
            return;
        }
        let (dx, dy) = self.scale_point(dx, dy);
        for bx in boxes {
            let bx = self.scale_rect(*bx);
            self.ops.push(GfxApiOpt::FillRect(FillRect {
                rect: AbsoluteRect {
                    x1: (bx.x1() + dx) as f32,
                    y1: (bx.y1() + dy) as f32,
                    x2: (bx.x2() + dx) as f32,
                    y2: (bx.y2() + dy) as f32,
                },
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
        if boxes.is_empty() {
            return;
        }
        let (dx, dy) = self.scale_point_f(dx, dy);
        for bx in boxes {
            let (x1, y1, x2, y2) = self.scale_rect_f(*bx);
            self.ops.push(GfxApiOpt::FillRect(FillRect {
                rect: AbsoluteRect {
                    x1: x1 + dx,
                    y1: y1 + dy,
                    x2: x2 + dx,
                    y2: y2 + dy,
                },
                color: *color,
            }));
        }
    }

    pub fn render_texture(
        &mut self,
        texture: &Rc<dyn GfxTexture>,
        x: i32,
        y: i32,
        tpoints: Option<BufferPoints>,
        tsize: Option<(i32, i32)>,
        tscale: Scale,
        bounds: Option<&Rect>,
    ) {
        let mut texcoord = tpoints.unwrap_or(BufferPoints {
            top_left: BufferPoint { x: 0.0, y: 0.0 },
            top_right: BufferPoint { x: 1.0, y: 0.0 },
            bottom_left: BufferPoint { x: 0.0, y: 1.0 },
            bottom_right: BufferPoint { x: 1.0, y: 1.0 },
        });

        let (twidth, theight) = if let Some(size) = tsize {
            size
        } else {
            let (mut w, mut h) = texture.size();
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
            #[cold]
            fn cold() {}

            let bounds_x = [bounds.x1(), bounds.x2()];
            let bounds_y = [bounds.y1(), bounds.y2()];

            macro_rules! clamp {
                ($desired:ident, $bounds:ident, $test_idx:expr, $test_cmp:ident, $test_cmp_eq:ident, $([$modify:ident, $keep:ident],)*) => {{
                    let desired_test = $desired[$test_idx];
                    let desired_other = $desired[1 - $test_idx];
                    let bound = $bounds[$test_idx];
                    if desired_test.$test_cmp(&bound) {
                        cold();
                        if desired_other.$test_cmp_eq(&bound) {
                            return;
                        }
                        let max = (desired_other - bound) as f32;
                        let desired = ($desired[1] - $desired[0]) as f32;
                        let factor = max.abs() / desired;
                        $(
                            let dx = (texcoord.$modify.x - texcoord.$keep.x) * factor;
                            texcoord.$modify.x = texcoord.$keep.x + dx;
                            let dy = (texcoord.$modify.y - texcoord.$keep.y) * factor;
                            texcoord.$modify.y = texcoord.$keep.y + dy;
                        )*
                        $desired[$test_idx] = bound;
                    }
                }};
            }

            clamp!(
                target_x,
                bounds_x,
                0,
                lt,
                le,
                [top_left, top_right],
                [bottom_left, bottom_right],
            );

            clamp!(
                target_x,
                bounds_x,
                1,
                gt,
                ge,
                [top_right, top_left],
                [bottom_right, bottom_left],
            );

            clamp!(
                target_y,
                bounds_y,
                0,
                lt,
                le,
                [top_left, bottom_left],
                [top_right, bottom_right],
            );

            clamp!(
                target_y,
                bounds_y,
                1,
                gt,
                ge,
                [bottom_left, top_left],
                [bottom_right, top_right],
            );
        }

        self.ops.push(GfxApiOpt::CopyTexture(CopyTexture {
            tex: texture.clone(),
            source: texcoord,
            target: AbsoluteRect {
                x1: target_x[0] as f32,
                y1: target_y[0] as f32,
                x2: target_x[1] as f32,
                y2: target_y[1] as f32,
            },
        }));
    }
}
