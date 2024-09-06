pub use uapi::c;

pub type GLbitfield = c::c_uint;
pub type GLboolean = c::c_uchar;
pub type GLchar = c::c_char;
pub type GLenum = c::c_uint;
pub type GLfloat = f32;
pub type GLint = c::c_int;
pub type GLsizei = c::c_int;
#[expect(dead_code)]
pub type GLubyte = u8;
pub type GLuint = c::c_uint;

egl_transparent!(GLeglImageOES);

pub const GL_RGBA: GLint = 0x1908;
pub const GL_RGBA8: GLenum = 0x8058;
pub const GL_BGRA_EXT: GLint = 0x80E1;
pub const GL_CLAMP_TO_EDGE: GLint = 0x812F;
pub const GL_COLOR_ATTACHMENT0: GLenum = 0x8CE0;
pub const GL_COLOR_BUFFER_BIT: GLbitfield = 0x00004000;
pub const GL_COMPILE_STATUS: GLenum = 0x8B81;
pub const GL_EXTENSIONS: GLenum = 0x1F03;
pub const GL_FALSE: GLboolean = 0;
pub const GL_FLOAT: GLenum = 0x1406;
pub const GL_FRAGMENT_SHADER: GLenum = 0x8B30;
pub const GL_FRAMEBUFFER_COMPLETE: GLenum = 0x8CD5;
pub const GL_FRAMEBUFFER: GLenum = 0x8D40;
pub const GL_LINEAR: GLint = 0x2601;
pub const GL_LINK_STATUS: GLenum = 0x8B82;
pub const GL_RENDERBUFFER: GLenum = 0x8D41;
pub const GL_TEXTURE0: GLenum = 0x84C0;
pub const GL_TEXTURE_2D: GLenum = 0x0DE1;
pub const GL_TEXTURE_EXTERNAL_OES: GLenum = 0x8D65;
#[expect(dead_code)]
pub const GL_TEXTURE_MAG_FILTER: GLenum = 0x2800;
pub const GL_TEXTURE_MIN_FILTER: GLenum = 0x2801;
pub const GL_TEXTURE_WRAP_S: GLenum = 0x2802;
pub const GL_TEXTURE_WRAP_T: GLenum = 0x2803;
pub const GL_TRIANGLE_STRIP: GLenum = 0x0005;
pub const GL_TRIANGLES: GLenum = 0x0004;
pub const GL_UNPACK_ROW_LENGTH_EXT: GLenum = 0x0CF2;
pub const GL_UNSIGNED_BYTE: GLint = 0x1401;
pub const GL_VERTEX_SHADER: GLenum = 0x8B31;
pub const GL_BLEND: GLenum = 0x0BE2;
pub const GL_ONE: GLenum = 1;
pub const GL_ONE_MINUS_SRC_ALPHA: GLenum = 0x0303;

dynload! {
    GLESV2: GlesV2 from "libGLESv2.so" {
        glGetString: unsafe fn(name: GLenum) -> *const u8,
        glGenRenderbuffers: unsafe fn(n: GLsizei, renderbuffers: *mut GLuint),
        glRenderbufferStorage: unsafe fn(target: GLenum, format: GLenum, width: GLsizei, height: GLsizei),
        glDeleteRenderbuffers: unsafe fn(n: GLsizei, renderbuffers: *const GLuint),
        glBindRenderbuffer: unsafe fn(target: GLenum, renderbuffer: GLuint),
        glGenFramebuffers: unsafe fn(n: GLsizei, framebuffers: *mut GLuint),
        glDeleteFramebuffers: unsafe fn(n: GLsizei, framebuffers: *const GLuint),
        glBindFramebuffer: unsafe fn(target: GLenum, framebuffer: GLuint),
        glFramebufferRenderbuffer: unsafe fn(
            target: GLenum,
            attachment: GLenum,
            renderbuffertarget: GLenum,
            renderbuffer: GLuint,
        ),
        glCheckFramebufferStatus: unsafe fn(target: GLenum) -> GLenum,
        glClear: unsafe fn(mask: GLbitfield),
        glBlendFunc: unsafe fn(sfactor: GLenum, dfactor: GLenum),
        glClearColor: unsafe fn(red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat),
        glFlush: unsafe fn(),

        glReadnPixels: unsafe fn(
            x: GLint,
            y: GLint,
            width: GLsizei,
            height: GLsizei,
            format: GLenum,
            ty: GLenum,
            buf_size: GLsizei,
            data: *mut c::c_void,
        ),

        glGenTextures: unsafe fn(n: GLsizei, textures: *mut GLuint),
        glDeleteTextures: unsafe fn(n: GLsizei, textures: *const GLuint),
        glBindTexture: unsafe fn(target: GLenum, texture: GLuint),
        glTexParameteri: unsafe fn(target: GLenum, pname: GLenum, param: GLint),

        glPixelStorei: unsafe fn(pname: GLenum, param: GLint),

        glTexImage2D: unsafe fn(
            target: GLenum,
            level: GLint,
            internalformat: GLint,
            width: GLsizei,
            height: GLsizei,
            border: GLint,
            format: GLenum,
            ty: GLenum,
            pixels: *const c::c_void,
        ),

        glEnable: unsafe fn(cap: GLenum),
        glDisable: unsafe fn(cap: GLenum),
        glViewport: unsafe fn(x: GLint, y: GLint, width: GLsizei, height: GLsizei),

        glCreateShader: unsafe fn(ty: GLenum) -> GLuint,
        glDeleteShader: unsafe fn(shader: GLuint),
        glShaderSource: unsafe fn(
            shader: GLuint,
            count: GLsizei,
            string: *const *const GLchar,
            length: *const GLint,
        ),
        glCompileShader: unsafe fn(shader: GLuint),
        glGetShaderiv: unsafe fn(shader: GLuint, pname: GLenum, params: *mut GLint),

        glCreateProgram: unsafe fn() -> GLuint,
        glDeleteProgram: unsafe fn(prog: GLuint),
        glAttachShader: unsafe fn(prog: GLuint, shader: GLuint),
        glDetachShader: unsafe fn(prog: GLuint, shader: GLuint),
        glLinkProgram: unsafe fn(prog: GLuint),
        glGetProgramiv: unsafe fn(program: GLuint, pname: GLenum, params: *mut GLint),
        glUseProgram: unsafe fn(program: GLuint),

        glGetUniformLocation: unsafe fn(prog: GLuint, name: *const GLchar) -> GLint,
        glGetAttribLocation: unsafe fn(prog: GLuint, name: *const GLchar) -> GLint,
        glUniform1i: unsafe fn(location: GLint, v0: GLint),
        glUniform1f: unsafe fn(location: GLint, v0: GLfloat),
        glUniform4f: unsafe fn(location: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat, v3: GLfloat),
        glVertexAttribPointer: unsafe fn(
            index: GLuint,
            size: GLint,
            ty: GLenum,
            normalized: GLboolean,
            stride: GLsizei,
            pointer: *const u8,
        ),

        glActiveTexture: unsafe fn(texture: GLuint),

        glEnableVertexAttribArray: unsafe fn(idx: GLuint),
        glDisableVertexAttribArray: unsafe fn(idx: GLuint),
        glDrawArrays: unsafe fn(mode: GLenum, first: GLint, count: GLsizei),
    }
}
