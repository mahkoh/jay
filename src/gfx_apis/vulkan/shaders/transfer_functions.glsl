#ifndef TRANSFER_FUNCTIONS_GLSL
#define TRANSFER_FUNCTIONS_GLSL

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

#endif
