use {
    crate::{
        format::Format,
        rect::Rect,
        render::{
            gl::frame_buffer::GlFrameBuffer,
            renderer::{context::RenderContext, gfx_apis::gl},
            Texture,
        },
        scale::Scale,
        theme::Color,
    },
    std::rc::Rc,
};

pub struct RendererBase<'a> {
    pub(super) ctx: &'a Rc<RenderContext>,
    pub(super) fb: &'a GlFrameBuffer,
    pub(super) scaled: bool,
    pub(super) scale: Scale,
    pub(super) scalef: f64,
}

impl RendererBase<'_> {
    pub fn scale(&self) -> Scale {
        self.scale
    }

    pub fn physical_extents(&self) -> Rect {
        Rect::new_sized(0, 0, self.fb.width, self.fb.height).unwrap()
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

    fn xf_to_f(&self, x: f32) -> f32 {
        2.0 * (x / self.fb.width as f32) - 1.0
    }

    fn yf_to_f(&self, y: f32) -> f32 {
        2.0 * (y / self.fb.height as f32) - 1.0
    }

    fn x_to_f(&self, x: i32) -> f32 {
        2.0 * (x as f32 / self.fb.width as f32) - 1.0
    }

    fn y_to_f(&self, y: i32) -> f32 {
        2.0 * (y as f32 / self.fb.height as f32) - 1.0
    }

    pub fn clear(&self, c: &Color) {
        gl::clear(c);
    }

    pub fn fill_boxes(&self, boxes: &[Rect], color: &Color) {
        self.fill_boxes2(boxes, color, 0, 0);
    }

    pub fn fill_boxes2(&self, boxes: &[Rect], color: &Color, dx: i32, dy: i32) {
        if boxes.is_empty() {
            return;
        }
        let (dx, dy) = self.scale_point(dx, dy);
        let mut pos = Vec::with_capacity(boxes.len() * 12);
        for bx in boxes {
            let bx = self.scale_rect(*bx);
            let x1 = self.x_to_f(bx.x1() + dx);
            let y1 = self.y_to_f(bx.y1() + dy);
            let x2 = self.x_to_f(bx.x2() + dx);
            let y2 = self.y_to_f(bx.y2() + dy);
            pos.extend_from_slice(&[
                // triangle 1
                x2, y1, // top right
                x1, y1, // top left
                x1, y2, // bottom left
                // triangle 2
                x2, y1, // top right
                x1, y2, // bottom left
                x2, y2, // bottom right
            ]);
        }
        self.fill_boxes3(&pos, color)
    }

    pub fn fill_boxes_f(&self, boxes: &[(f32, f32, f32, f32)], color: &Color) {
        self.fill_boxes2_f(boxes, color, 0.0, 0.0);
    }

    pub fn fill_boxes2_f(&self, boxes: &[(f32, f32, f32, f32)], color: &Color, dx: f32, dy: f32) {
        if boxes.is_empty() {
            return;
        }
        let (dx, dy) = self.scale_point_f(dx, dy);
        let mut pos = Vec::with_capacity(boxes.len() * 12);
        for bx in boxes {
            let (x1, y1, x2, y2) = self.scale_rect_f(*bx);
            let x1 = self.xf_to_f(x1 + dx);
            let y1 = self.yf_to_f(y1 + dy);
            let x2 = self.xf_to_f(x2 + dx);
            let y2 = self.yf_to_f(y2 + dy);
            pos.extend_from_slice(&[
                // triangle 1
                x2, y1, // top right
                x1, y1, // top left
                x1, y2, // bottom left
                // triangle 2
                x2, y1, // top right
                x1, y2, // bottom left
                x2, y2, // bottom right
            ]);
        }
        self.fill_boxes3(&pos, color)
    }

    fn fill_boxes3(&self, boxes: &[f32], color: &Color) {
        gl::fill_boxes3(&self.ctx, boxes, color);
    }

    pub fn render_texture(
        &mut self,
        texture: &Texture,
        x: i32,
        y: i32,
        format: &Format,
        tpoints: Option<&[f32; 8]>,
        tsize: Option<(i32, i32)>,
        tscale: Scale,
    ) {
        gl::render_texture(
            &self.ctx,
            &self.fb,
            texture,
            x,
            y,
            format,
            tpoints,
            tsize,
            tscale,
            self.scale,
            self.scalef,
        )
    }
}
