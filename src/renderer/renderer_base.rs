use {
    crate::{
        format::Format,
        gfx_api::{
            AbsoluteRect, BufferPoint, BufferPoints, Clear, CopyTexture, FillRect, GfxApiOpt,
            GfxTexture,
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

    pub fn clear(&mut self, c: &Color) {
        self.ops.push(GfxApiOpt::Clear(Clear { color: *c }))
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
        format: &'static Format,
        tpoints: Option<BufferPoints>,
        tsize: Option<(i32, i32)>,
        tscale: Scale,
        max_width: i32,
        max_height: i32,
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
            let (mut w, mut h) = (texture.width(), texture.height());
            if tscale != self.scale {
                let tscale = tscale.to_f64();
                w = (w as f64 * self.scalef / tscale).round() as _;
                h = (h as f64 * self.scalef / tscale).round() as _;
            }
            (w, h)
        };

        macro_rules! clamp {
            ($desired:ident, $max:ident, $([$far:ident, $near:ident]),*) => {
                if $desired > $max {
                    let $desired = $desired as f32;
                    let $max = $max as f32;
                    let factor = $max / $desired;
                    $(
                        let dx = (texcoord.$far.x - texcoord.$near.x) * factor;
                        texcoord.$far.x = texcoord.$near.x + dx;
                        let dy = (texcoord.$far.y - texcoord.$near.y) * factor;
                        texcoord.$far.y = texcoord.$near.y + dy;
                    )*
                    $max
                } else {
                    $desired as f32
                }
            };
        }

        let twidth = clamp!(
            twidth,
            max_width,
            [top_right, top_left],
            [bottom_right, bottom_left]
        );
        let theight = clamp!(
            theight,
            max_height,
            [bottom_left, top_left],
            [bottom_right, top_right]
        );

        let x = x as f32;
        let y = y as f32;

        self.ops.push(GfxApiOpt::CopyTexture(CopyTexture {
            tex: texture.clone(),
            format,
            source: texcoord,
            target: AbsoluteRect {
                x1: x,
                y1: y,
                x2: x + twidth,
                y2: y + theight,
            },
        }));
    }
}
