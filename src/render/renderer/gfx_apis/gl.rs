use crate::{
    format::Format,
    render::{
        gl::{frame_buffer::GlFrameBuffer, texture::image_target},
        sys::{
            glActiveTexture, glBindTexture, glClear, glClearColor, glDisable,
            glDisableVertexAttribArray, glDrawArrays, glEnable, glEnableVertexAttribArray,
            glTexParameteri, glUniform1i, glUniform4f, glUseProgram, glVertexAttribPointer,
            GL_BLEND, GL_COLOR_BUFFER_BIT, GL_FALSE, GL_FLOAT, GL_LINEAR, GL_TEXTURE0,
            GL_TEXTURE_MIN_FILTER, GL_TRIANGLES, GL_TRIANGLE_STRIP,
        },
        RenderContext, Texture,
    },
    scale::Scale,
    theme::Color,
    utils::rc_eq::rc_eq,
};

pub fn clear(c: &Color) {
    unsafe {
        glClearColor(c.r, c.g, c.b, c.a);
        glClear(GL_COLOR_BUFFER_BIT);
    }
}

pub fn fill_boxes3(ctx: &RenderContext, boxes: &[f32], color: &Color) {
    unsafe {
        glUseProgram(ctx.fill_prog.prog);
        glUniform4f(ctx.fill_prog_color, color.r, color.g, color.b, color.a);
        glVertexAttribPointer(
            ctx.fill_prog_pos as _,
            2,
            GL_FLOAT,
            GL_FALSE,
            0,
            boxes.as_ptr() as _,
        );
        glEnableVertexAttribArray(ctx.fill_prog_pos as _);
        glDrawArrays(GL_TRIANGLES, 0, (boxes.len() / 2) as _);
        glDisableVertexAttribArray(ctx.fill_prog_pos as _);
    }
}

pub fn render_texture(
    ctx: &RenderContext,
    fb: &GlFrameBuffer,
    texture: &Texture,
    x: i32,
    y: i32,
    format: &Format,
    tpoints: Option<&[f32; 8]>,
    tsize: Option<(i32, i32)>,
    tscale: Scale,
    scale: Scale,
    scalef: f64,
) {
    assert!(rc_eq(&ctx.ctx, &texture.ctx.ctx));
    unsafe {
        glActiveTexture(GL_TEXTURE0);

        let target = image_target(texture.gl.external_only);

        glBindTexture(target, texture.gl.tex);
        glTexParameteri(target, GL_TEXTURE_MIN_FILTER, GL_LINEAR);

        let progs = match texture.gl.external_only {
            true => match &ctx.tex_external {
                Some(p) => p,
                _ => {
                    log::error!("Trying to render an external-only texture but context does not support the required extension");
                    return;
                }
            },
            false => &ctx.tex_internal,
        };
        let prog = match format.has_alpha {
            true => {
                glEnable(GL_BLEND);
                &progs.alpha
            }
            false => {
                glDisable(GL_BLEND);
                &progs.solid
            }
        };

        glUseProgram(prog.prog.prog);

        glUniform1i(prog.tex, 0);

        static DEFAULT_TEXCOORD: [f32; 8] = [1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0];

        let texcoord: &[f32; 8] = match tpoints {
            None => &DEFAULT_TEXCOORD,
            Some(tp) => tp,
        };

        let f_width = fb.width as f32;
        let f_height = fb.height as f32;

        let (twidth, theight) = if let Some(size) = tsize {
            size
        } else {
            let (mut w, mut h) = (texture.gl.width, texture.gl.height);
            if tscale != scale {
                let tscale = tscale.to_f64();
                w = (w as f64 * scalef / tscale).round() as _;
                h = (h as f64 * scalef / tscale).round() as _;
            }
            (w, h)
        };

        let x1 = 2.0 * (x as f32 / f_width) - 1.0;
        let y1 = 2.0 * (y as f32 / f_height) - 1.0;
        let x2 = 2.0 * ((x + twidth) as f32 / f_width) - 1.0;
        let y2 = 2.0 * ((y + theight) as f32 / f_height) - 1.0;

        let pos: [f32; 8] = [
            x2, y1, // top right
            x1, y1, // top left
            x2, y2, // bottom right
            x1, y2, // bottom left
        ];

        glVertexAttribPointer(
            prog.texcoord as _,
            2,
            GL_FLOAT,
            GL_FALSE,
            0,
            texcoord.as_ptr() as _,
        );
        glVertexAttribPointer(prog.pos as _, 2, GL_FLOAT, GL_FALSE, 0, pos.as_ptr() as _);

        glEnableVertexAttribArray(prog.texcoord as _);
        glEnableVertexAttribArray(prog.pos as _);

        glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);

        glDisableVertexAttribArray(prog.texcoord as _);
        glDisableVertexAttribArray(prog.pos as _);

        glBindTexture(target, 0);
    }
}
