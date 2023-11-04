pub use uapi::c;

pub type GLbitfield = c::c_uint;
pub type GLboolean = c::c_uchar;
pub type GLchar = c::c_char;
pub type GLenum = c::c_uint;
pub type GLfloat = f32;
pub type GLint = c::c_int;
pub type GLsizei = c::c_int;
#[allow(dead_code)]
pub type GLubyte = u8;
pub type GLuint = c::c_uint;

egl_transparent!(GLeglImageOES);

pub const GL_RGBA: GLint = 0x1908;
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
#[allow(dead_code)]
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

#[link(name = "GLESv2")]
extern "C" {
    pub fn glGetString(name: GLenum) -> *const u8;
    pub fn glGenRenderbuffers(n: GLsizei, renderbuffers: *mut GLuint);
    pub fn glDeleteRenderbuffers(n: GLsizei, renderbuffers: *const GLuint);
    pub fn glBindRenderbuffer(target: GLenum, renderbuffer: GLuint);
    pub fn glGenFramebuffers(n: GLsizei, framebuffers: *mut GLuint);
    pub fn glDeleteFramebuffers(n: GLsizei, framebuffers: *const GLuint);
    pub fn glBindFramebuffer(target: GLenum, framebuffer: GLuint);
    pub fn glFramebufferRenderbuffer(
        target: GLenum,
        attachment: GLenum,
        renderbuffertarget: GLenum,
        renderbuffer: GLuint,
    );
    #[allow(dead_code)]
    pub fn glFramebufferTexture2D(
        target: GLenum,
        attachment: GLenum,
        textarget: GLenum,
        texture: GLenum,
        level: GLint,
    );
    pub fn glCheckFramebufferStatus(target: GLenum) -> GLenum;
    pub fn glClear(mask: GLbitfield);
    pub fn glBlendFunc(sfactor: GLenum, dfactor: GLenum);
    pub fn glClearColor(red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat);
    #[allow(dead_code)]
    pub fn glFlush();

    pub fn glGenTextures(n: GLsizei, textures: *mut GLuint);
    pub fn glDeleteTextures(n: GLsizei, textures: *const GLuint);
    pub fn glBindTexture(target: GLenum, texture: GLuint);
    pub fn glTexParameteri(target: GLenum, pname: GLenum, param: GLint);

    pub fn glPixelStorei(pname: GLenum, param: GLint);

    pub fn glTexImage2D(
        target: GLenum,
        level: GLint,
        internalformat: GLint,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
        format: GLenum,
        ty: GLenum,
        pixels: *const c::c_void,
    );

    pub fn glEnable(cap: GLenum);
    pub fn glDisable(cap: GLenum);
    pub fn glViewport(x: GLint, y: GLint, width: GLsizei, height: GLsizei);

    pub fn glCreateShader(ty: GLenum) -> GLuint;
    pub fn glDeleteShader(shader: GLuint);
    pub fn glShaderSource(
        shader: GLuint,
        count: GLsizei,
        string: *const *const GLchar,
        length: *const GLint,
    );
    pub fn glCompileShader(shader: GLuint);
    pub fn glGetShaderiv(shader: GLuint, pname: GLenum, params: *mut GLint);

    pub fn glCreateProgram() -> GLuint;
    pub fn glDeleteProgram(prog: GLuint);
    pub fn glAttachShader(prog: GLuint, shader: GLuint);
    pub fn glDetachShader(prog: GLuint, shader: GLuint);
    pub fn glLinkProgram(prog: GLuint);
    pub fn glGetProgramiv(program: GLuint, pname: GLenum, params: *mut GLint);
    pub fn glUseProgram(program: GLuint);

    pub fn glGetUniformLocation(prog: GLuint, name: *const GLchar) -> GLint;
    pub fn glGetAttribLocation(prog: GLuint, name: *const GLchar) -> GLint;
    pub fn glUniform1i(location: GLint, v0: GLint);
    #[allow(dead_code)]
    pub fn glUniform1f(location: GLint, v0: GLfloat);
    pub fn glUniform4f(location: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat, v3: GLfloat);
    pub fn glVertexAttribPointer(
        index: GLuint,
        size: GLint,
        ty: GLenum,
        normalized: GLboolean,
        stride: GLsizei,
        pointer: *const u8,
    );

    pub fn glActiveTexture(texture: GLuint);

    pub fn glEnableVertexAttribArray(idx: GLuint);
    pub fn glDisableVertexAttribArray(idx: GLuint);
    pub fn glDrawArrays(mode: GLenum, first: GLint, count: GLsizei);
}
