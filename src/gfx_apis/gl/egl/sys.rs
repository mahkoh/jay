use {crate::gfx_apis::gl::sys::GLenum, uapi::c};

pub type EGLint = i32;
pub type EGLenum = c::c_uint;
pub type EGLBoolean = c::c_uint;
pub type EGLuint64KHR = u64;
pub type EGLAttrib = isize;
pub type EGLSyncKHR = *mut u8;

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
pub const EGL_PLATFORM_GBM_KHR: EGLint = 0x31D7;
pub const EGL_CONTEXT_CLIENT_VERSION: EGLint = 0x3098;
pub const EGL_CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY_EXT: EGLint = 0x3138;
pub const EGL_LOSE_CONTEXT_ON_RESET_EXT: EGLint = 0x31BF;
pub const EGL_DEVICE_EXT: EGLint = 0x322C;

pub const GL_GUILTY_CONTEXT_RESET_ARB: GLenum = 0x8253;
pub const GL_INNOCENT_CONTEXT_RESET_ARB: GLenum = 0x8254;
pub const GL_UNKNOWN_CONTEXT_RESET_ARB: GLenum = 0x8255;

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
pub const EGL_SYNC_NATIVE_FENCE_ANDROID: EGLenum = 0x3144;
pub const EGL_SYNC_NATIVE_FENCE_FD_ANDROID: EGLint = 0x3145;

dynload! {
    EGL: Egl from "libEGL.so" {
        eglQueryString: unsafe fn(dpy: EGLDisplay, name: EGLint) -> *const c::c_char,
        eglGetProcAddress: unsafe fn(procname: *const c::c_char) -> *mut u8,
        eglBindAPI: unsafe fn(api: EGLenum) -> EGLBoolean,
        eglTerminate: unsafe fn(dpy: EGLDisplay) -> EGLBoolean,
        eglInitialize: unsafe fn(dpy: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> EGLBoolean,
        eglCreateContext: unsafe fn(
            dpy: EGLDisplay,
            config: EGLConfig,
            share_context: EGLContext,
            attrib_list: *const EGLint,
        ) -> EGLContext,
        eglDestroyContext: unsafe fn(dpy: EGLDisplay, ctx: EGLContext) -> EGLBoolean,
        eglMakeCurrent: unsafe fn(
            dpy: EGLDisplay,
            draw: EGLSurface,
            read: EGLSurface,
            ctx: EGLContext,
        ) -> EGLBoolean,
    }
}
