#define _GNU_SOURCE

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>

static char *fmt(const char *format, va_list args) {
    char *line;
    int ret = vasprintf(&line, format, args);
    if (ret < 0) {
        return 0;
    } else {
        return line;
    }
}

void jay_libinput_log_handler(
    void *libinput,
    int priority,
    const char *line
);

void jay_libinput_log_handler_bridge(
    void *libinput,
    int priority,
    const char *format,
    va_list args
) {
    char *line = fmt(format, args);
    jay_libinput_log_handler(libinput, priority, line);
    free(line);
}

void jay_xkbcommon_log_handler(
    void *ctx,
    int xkb_log_level,
    const char *line
);

void jay_xkbcommon_log_handler_bridge(
    void *ctx,
    int xkb_log_level,
    const char *format,
    va_list args
) {
    char *line = fmt(format, args);
    jay_xkbcommon_log_handler(ctx, xkb_log_level, line);
    free(line);
}
