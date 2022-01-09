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

cenum! {
    XkbStateComponent, XKB_STATE_COMPONENT;

    XKB_STATE_MODS_DEPRESSED = 1 << 0,
    XKB_STATE_MODS_LATCHED = 1 << 1,
    XKB_STATE_MODS_LOCKED = 1 << 2,
    XKB_STATE_MODS_EFFECTIVE = 1 << 3,
    XKB_STATE_LAYOUT_DEPRESSED = 1 << 4,
    XKB_STATE_LAYOUT_LATCHED = 1 << 5,
    XKB_STATE_LAYOUT_LOCKED = 1 << 6,
    XKB_STATE_LAYOUT_EFFECTIVE = 1 << 7,
    XKB_STATE_LEDS = 1 << 8,
}

bitor!(XkbStateComponent);

cenum! {
    XkbKeyDirection, XKB_KEY_DIRECTION;

    XKB_KEY_UP = 0,
    XKB_KEY_DOWN = 1,
}
