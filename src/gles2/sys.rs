pub use uapi::c;

macro_rules! egl_transparent {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name(pub *mut u8);

        impl $name {
            #[allow(dead_code)]
            pub const fn none() -> Self {
                Self(std::ptr::null_mut())
            }

            #[allow(dead_code)]
            pub fn is_none(self) -> bool {
                self.0.is_null()
            }
        }
    };
}

pub type EGLint = i32;
pub type EGLenum = c::c_uint;
pub type EGLBoolean = c::c_uint;
pub type EGLuint64KHR = u64;
pub type EGLAttrib = isize;

egl_transparent!(EGLDisplay);
egl_transparent!(EGLSurface);
egl_transparent!(EGLConfig);
egl_transparent!(EGLImageKHR);
egl_transparent!(EGLContext);
egl_transparent!(EGLClientBuffer);
egl_transparent!(EGLLabelKHR);
egl_transparent!(EGLDeviceEXT);

pub type EGLDEBUGPROCKHR = unsafe extern "C" fn(
    error: EGLenum,
    command: *const c::c_char,
    message_type: EGLint,
    thread_label: EGLLabelKHR,
    object_label: EGLLabelKHR,
    message: *const c::c_char,
);

pub const EGL_EXTENSIONS: EGLint = 0x3055;
pub const EGL_DEBUG_MSG_CRITICAL_KHR: EGLint = 0x33B9;
pub const EGL_DEBUG_MSG_ERROR_KHR: EGLint = 0x33BA;
pub const EGL_DEBUG_MSG_WARN_KHR: EGLint = 0x33BB;
pub const EGL_DEBUG_MSG_INFO_KHR: EGLint = 0x33BC;
pub const EGL_TRUE: EGLBoolean = 1;
pub const EGL_FALSE: EGLBoolean = 0;
pub const EGL_NONE: EGLint = 0x3038;
pub const EGL_SUCCESS: EGLint = 0x3000;
pub const EGL_NOT_INITIALIZED: EGLint = 0x3001;
pub const EGL_BAD_ACCESS: EGLint = 0x3002;
pub const EGL_BAD_ALLOC: EGLint = 0x3003;
pub const EGL_BAD_ATTRIBUTE: EGLint = 0x3004;
pub const EGL_BAD_CONFIG: EGLint = 0x3005;
pub const EGL_BAD_CONTEXT: EGLint = 0x3006;
pub const EGL_BAD_CURRENT_SURFACE: EGLint = 0x3007;
pub const EGL_BAD_DISPLAY: EGLint = 0x3008;
pub const EGL_BAD_MATCH: EGLint = 0x3009;
pub const EGL_BAD_NATIVE_PIXMAP: EGLint = 0x300A;
pub const EGL_BAD_NATIVE_WINDOW: EGLint = 0x300B;
pub const EGL_BAD_PARAMETER: EGLint = 0x300C;
pub const EGL_BAD_SURFACE: EGLint = 0x300D;
pub const EGL_CONTEXT_LOST: EGLint = 0x300E;
pub const EGL_BAD_DEVICE_EXT: EGLint = 0x322B;
pub const EGL_OPENGL_ES_API: EGLenum = 0x30A0;
pub const EGL_DRM_DEVICE_FILE_EXT: EGLint = 0x3233;
pub const EGL_PLATFORM_DEVICE_EXT: EGLint = 0x313F;
pub const EGL_CONTEXT_CLIENT_VERSION: EGLint = 0x3098;

pub const EGL_WIDTH: EGLint = 0x3057;
pub const EGL_HEIGHT: EGLint = 0x3056;
pub const EGL_LINUX_DRM_FOURCC_EXT: EGLint = 0x3271;
pub const EGL_DMA_BUF_PLANE0_FD_EXT: EGLint = 0x3272;
pub const EGL_DMA_BUF_PLANE0_OFFSET_EXT: EGLint = 0x3273;
pub const EGL_DMA_BUF_PLANE0_PITCH_EXT: EGLint = 0x3274;
pub const EGL_DMA_BUF_PLANE1_FD_EXT: EGLint = 0x3275;
pub const EGL_DMA_BUF_PLANE1_OFFSET_EXT: EGLint = 0x3276;
pub const EGL_DMA_BUF_PLANE1_PITCH_EXT: EGLint = 0x3277;
pub const EGL_DMA_BUF_PLANE2_FD_EXT: EGLint = 0x3278;
pub const EGL_DMA_BUF_PLANE2_OFFSET_EXT: EGLint = 0x3279;
pub const EGL_DMA_BUF_PLANE2_PITCH_EXT: EGLint = 0x327A;
pub const EGL_DMA_BUF_PLANE3_FD_EXT: EGLint = 0x3440;
pub const EGL_DMA_BUF_PLANE3_OFFSET_EXT: EGLint = 0x3441;
pub const EGL_DMA_BUF_PLANE3_PITCH_EXT: EGLint = 0x3442;
pub const EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT: EGLint = 0x3443;
pub const EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT: EGLint = 0x3444;
pub const EGL_DMA_BUF_PLANE1_MODIFIER_LO_EXT: EGLint = 0x3445;
pub const EGL_DMA_BUF_PLANE1_MODIFIER_HI_EXT: EGLint = 0x3446;
pub const EGL_DMA_BUF_PLANE2_MODIFIER_LO_EXT: EGLint = 0x3447;
pub const EGL_DMA_BUF_PLANE2_MODIFIER_HI_EXT: EGLint = 0x3448;
pub const EGL_DMA_BUF_PLANE3_MODIFIER_LO_EXT: EGLint = 0x3449;
pub const EGL_DMA_BUF_PLANE3_MODIFIER_HI_EXT: EGLint = 0x344A;
pub const EGL_IMAGE_PRESERVED_KHR: EGLint = 0x30D2;
pub const EGL_LINUX_DMA_BUF_EXT: EGLint = 0x3270;

pub type GLenum = c::c_uint;
pub type GLbitfield = c::c_uint;
pub type GLfloat = f32;
pub type GLubyte = u8;
pub type GLsizei = c::c_int;
pub type GLuint = c::c_uint;
pub type GLint = c::c_int;
pub type GLchar = c::c_char;
pub type GLboolean = c::c_uchar;

egl_transparent!(GLeglImageOES);

pub const GL_EXTENSIONS: GLenum = 0x1F03;
pub const GL_RENDERBUFFER: GLenum = 0x8D41;
pub const GL_FRAMEBUFFER: GLenum = 0x8D40;
pub const GL_COLOR_ATTACHMENT0: GLenum = 0x8CE0;
pub const GL_FRAMEBUFFER_COMPLETE: GLenum = 0x8CD5;

pub const GL_COLOR_BUFFER_BIT: GLbitfield = 0x00004000;
pub const GL_TEXTURE_2D: GLenum = 0x0DE1;
pub const GL_TEXTURE_WRAP_S: GLenum = 0x2802;
pub const GL_TEXTURE_WRAP_T: GLenum = 0x2803;
pub const GL_CLAMP_TO_EDGE: GLint = 0x812F;
pub const GL_UNPACK_ROW_LENGTH_EXT: GLenum = 0x0CF2;

pub const GL_BGRA_EXT: GLint = 0x80E1;
pub const GL_UNSIGNED_BYTE: GLint = 0x1401;
pub const GL_SCISSOR_TEST: GLenum = 0x0C11;

pub const GL_COMPILE_STATUS: GLenum = 0x8B81;
pub const GL_LINK_STATUS: GLenum = 0x8B82;

pub const GL_FALSE: GLboolean = 0;
pub const GL_FRAGMENT_SHADER: GLenum = 0x8B30;
pub const GL_VERTEX_SHADER: GLenum = 0x8B31;

pub const GL_TEXTURE0: GLenum = 0x84C0;
pub const GL_TEXTURE_MIN_FILTER: GLenum = 0x2801;
pub const GL_TEXTURE_MAG_FILTER: GLenum = 0x2800;
pub const GL_LINEAR: GLint = 0x2601;
pub const GL_FLOAT: GLenum = 0x1406;

pub const GL_TRIANGLE_STRIP: GLenum = 0x0005;

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
    pub fn glFramebufferTexture2D(
        target: GLenum,
        attachment: GLenum,
        textarget: GLenum,
        texture: GLenum,
        level: GLint,
    );
    pub fn glCheckFramebufferStatus(target: GLenum) -> GLenum;
    pub fn glClear(mask: GLbitfield);
    pub fn glClearColor(red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat);
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

    pub fn glScissor(x: GLint, y: GLint, width: GLsizei, height: GLsizei);
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
    pub fn glUniform1f(location: GLint, v0: GLfloat);
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

#[link(name = "EGL")]
extern "C" {
    pub fn eglQueryString(dpy: EGLDisplay, name: EGLint) -> *const c::c_char;
    pub fn eglGetProcAddress(procname: *const c::c_char) -> *mut u8;
    pub fn eglBindAPI(api: EGLenum) -> EGLBoolean;
    pub fn eglTerminate(dpy: EGLDisplay) -> EGLBoolean;
    pub fn eglInitialize(dpy: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> EGLBoolean;
    pub fn eglCreateContext(
        dpy: EGLDisplay,
        config: EGLConfig,
        share_context: EGLContext,
        attrib_list: *const EGLint,
    ) -> EGLContext;
    pub fn eglDestroyContext(dpy: EGLDisplay, ctx: EGLContext) -> EGLBoolean;
    pub fn eglMakeCurrent(
        dpy: EGLDisplay,
        draw: EGLSurface,
        read: EGLSurface,
        ctx: EGLContext,
    ) -> EGLBoolean;
}
