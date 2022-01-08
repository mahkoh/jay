#![allow(dead_code)]

cenum! {
    XkbLogLevel, XKB_LOG_LEVEL;

    XKB_LOG_LEVEL_CRITICAL = 10,
    XKB_LOG_LEVEL_ERROR = 20,
    XKB_LOG_LEVEL_WARNING = 30,
    XKB_LOG_LEVEL_INFO = 40,
    XKB_LOG_LEVEL_DEBUG = 50,
}

cenum! {
    XkbContextFlags, XKB_CONTEXT_FLAGS;

    XKB_CONTEXT_NO_FLAGS = 0,
    XKB_CONTEXT_NO_DEFAULT_INCLUDES = 1 << 0,
    XKB_CONTEXT_NO_ENVIRONMENT_NAMES = 1 << 1,
}

bitor!(XkbContextFlags);

cenum! {
    XkbKeymapCompileFlags, XKB_KEYMAP_COMPILE_FLAGS;

    XKB_KEYMAP_COMPILE_NO_FLAGS = 0,
}

bitor!(XkbKeymapCompileFlags);

cenum! {
    XkbKeymapFormat, XKB_KEYMAP_FORMAT;

    XKB_KEYMAP_FORMAT_TEXT_V1 = 1,
}
