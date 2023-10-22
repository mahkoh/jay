use {
    crate::{
        format::Format,
        render::{
            gfx_api::{BufferPoints, CopyTexture, FillRect, GfxApiOpt},
            gl::texture::image_target,
            sys::{
                glActiveTexture, glBindTexture, glClear, glClearColor, glDisable,
                glDisableVertexAttribArray, glDrawArrays, glEnable, glEnableVertexAttribArray,
                glTexParameteri, glUniform1i, glUniform4f, glUseProgram, glVertexAttribPointer,
                GL_BLEND, GL_COLOR_BUFFER_BIT, GL_FALSE, GL_FLOAT, GL_LINEAR, GL_TEXTURE0,
                GL_TEXTURE_MIN_FILTER, GL_TRIANGLES, GL_TRIANGLE_STRIP,
            },
            Framebuffer, RenderContext, Texture,
        },
        theme::Color,
        utils::{rc_eq::rc_eq, vecstorage::VecStorage},
    },
    isnt::std_1::vec::IsntVecExt,
    std::cell::RefCell,
};

#[derive(Default)]
pub struct GfxGlState {
    triangles: RefCell<Vec<f32>>,
    fill_rect: VecStorage<&'static FillRect>,
    copy_tex: VecStorage<&'static CopyTexture>,
}

pub fn run_ops(fb: &Framebuffer, ops: &[GfxApiOpt]) {
    let mut state = fb.ctx.gl_state.borrow_mut();
    let state = &mut *state;
    let mut fill_rect = state.fill_rect.take();
    let fill_rect = &mut *fill_rect;
    let mut copy_tex = state.copy_tex.take();
    let copy_tex = &mut *copy_tex;
    let mut triangles = state.triangles.borrow_mut();
    let triangles = &mut *triangles;
    let width = fb.gl.width as f32;
    let height = fb.gl.height as f32;
    let mut i = 0;
    while i < ops.len() {
        macro_rules! has_ops {
            () => {
                fill_rect.is_not_empty() || copy_tex.is_not_empty()
            };
        }
        fill_rect.clear();
        copy_tex.clear();
        while i < ops.len() {
            match &ops[i] {
                GfxApiOpt::Sync => {
                    i += 1;
                    if has_ops!() {
                        break;
                    }
                }
                GfxApiOpt::Clear(c) => {
                    if has_ops!() {
                        break;
                    }
                    clear(&c.color);
                    i += 1;
                }
                GfxApiOpt::FillRect(f) => {
                    fill_rect.push(f);
                    i += 1;
                }
                GfxApiOpt::CopyTexture(c) => {
                    copy_tex.push(c);
                    i += 1;
                }
            }
        }
        if fill_rect.is_not_empty() {
            fill_rect.sort_unstable_by_key(|f| f.color);
            let mut i = 0;
            while i < fill_rect.len() {
                triangles.clear();
                let mut color = None;
                while i < fill_rect.len() {
                    let fr = fill_rect[i];
                    match color {
                        None => color = Some(fr.color),
                        Some(c) if c == fr.color => {}
                        _ => break,
                    }
                    let x1 = 2.0 * (fr.rect.x1 / width) - 1.0;
                    let x2 = 2.0 * (fr.rect.x2 / width) - 1.0;
                    let y1 = 2.0 * (fr.rect.y1 / height) - 1.0;
                    let y2 = 2.0 * (fr.rect.y2 / height) - 1.0;
                    triangles.extend_from_slice(&[
                        // triangle 1
                        x2, y1, // top right
                        x1, y1, // top left
                        x1, y2, // bottom left
                        // triangle 2
                        x2, y1, // top right
                        x1, y2, // bottom left
                        x2, y2, // bottom right
                    ]);
                    i += 1;
                }
                if let Some(color) = color {
                    fill_boxes3(&fb.ctx, triangles, &color);
                }
            }
        }
        for tex in &*copy_tex {
            let x1 = 2.0 * (tex.target.x1 / width) - 1.0;
            let y1 = 2.0 * (tex.target.y1 / height) - 1.0;
            let x2 = 2.0 * (tex.target.x2 / width) - 1.0;
            let y2 = 2.0 * (tex.target.y2 / height) - 1.0;
            render_texture(&fb.ctx, &tex.tex, tex.format, x1, y1, x2, y2, &tex.source)
        }
    }
}

fn clear(c: &Color) {
    unsafe {
        glClearColor(c.r, c.g, c.b, c.a);
        glClear(GL_COLOR_BUFFER_BIT);
    }
}

fn fill_boxes3(ctx: &RenderContext, boxes: &[f32], color: &Color) {
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

fn render_texture(
    ctx: &RenderContext,
    texture: &Texture,
    format: &Format,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    src: &BufferPoints,
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

        let texcoord = [
            src.top_right.x,
            src.top_right.y,
            src.top_left.x,
            src.top_left.y,
            src.bottom_right.x,
            src.bottom_right.y,
            src.bottom_left.x,
            src.bottom_left.y,
        ];

        let pos = [
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
