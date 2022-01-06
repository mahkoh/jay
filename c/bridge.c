#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <xkbcommon/xkbcommon.h>

extern void i4_xkbcommon_log_fn(enum xkb_log_level level, unsigned char *bytes, size_t len);

void i4_xkbcommon_log_fn_bridge(
    struct xkb_context *context,
    enum xkb_log_level level,
    const char *format, va_list args)
{
    char *buf;
    int len = vasprintf(&buf, format, args);
    if (len < 0) {
        abort();
    }
    i4_xkbcommon_log_fn(level, (unsigned char *)buf, len);
}
