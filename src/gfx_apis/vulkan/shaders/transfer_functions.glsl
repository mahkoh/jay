#ifndef TRANSFER_FUNCTIONS_GLSL
#define TRANSFER_FUNCTIONS_GLSL

#include "frag_spec_const.glsl"

#define SRGB 0
#define LINEAR 1

vec3 eotf_srgb(vec3 c) {
    return mix(
        c * vec3(1.0 / 12.92),
        pow((c + vec3(0.055)) / vec3(1.055), vec3(2.4)),
        greaterThan(c, vec3(0.04045))
    );
}

vec3 oetf_srgb(vec3 c) {
    c = clamp(c, 0.0, 1.0);
    return mix(
        c * vec3(12.92),
        vec3(1.055) * pow(c, vec3(1/2.4)) - vec3(0.055),
        greaterThan(c, vec3(0.0031308))
    );
}

vec3 apply_eotf(vec3 c) {
    switch (eotf) {
        case SRGB: return eotf_srgb(c);
        case LINEAR: return c;
        default: return c;
    }
}

vec3 apply_oetf(vec3 c) {
    switch (oetf) {
        case SRGB: return oetf_srgb(c);
        case LINEAR: return c;
        default: return c;
    }
}

#endif
