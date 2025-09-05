#ifndef TRANSFER_FUNCTIONS_GLSL
#define TRANSFER_FUNCTIONS_GLSL

#include "frag_spec_const.glsl"

#define TF_SRGB 0
#define TF_LINEAR 1
#define TF_ST2084_PQ 2
#define TF_GAMMA24 3
#define TF_GAMMA22 4
#define TF_GAMMA28 5
#define TF_ST240 6
#define TF_EXT_SRGB 7
#define TF_LOG100 8
#define TF_LOG316 9
#define TF_ST428 10

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

vec3 eotf_ext_srgb(vec3 c) {
    return mix(
        -pow((c - vec3(0.055)) / vec3(-1.055), vec3(2.4)),
        mix(
            c * vec3(1.0 / 12.92),
            pow((c + vec3(0.055)) / vec3(1.055), vec3(2.4)),
            greaterThan(c, vec3(0.04045))
        ),
        greaterThan(c, vec3(-0.04045))
    );
}

vec3 oetf_ext_srgb(vec3 c) {
    c = clamp(c, -0.6038, 7.5913);
    return mix(
        vec3(-1.055) * pow(-c, vec3(1/2.4)) + vec3(0.055),
        mix(
            c * vec3(12.92),
            vec3(1.055) * pow(c, vec3(1/2.4)) - vec3(0.055),
            greaterThan(c, vec3(0.0031308))
        ),
        greaterThan(c, vec3(-0.0031308))
    );
}

vec3 eotf_st2084_pq(vec3 c) {
    c = clamp(c, 0.0, 1.0);
    vec3 cp = pow(c, vec3(1.0 / 78.84375));
    vec3 num = max(cp - vec3(0.8359375), 0.0);
    vec3 den = vec3(18.8515625) - vec3(18.6875) * cp;
    return pow(num / den, vec3(1.0 / 0.1593017578125));
}

vec3 oetf_st2084_pq(vec3 c) {
    c = clamp(c, 0.0, 1.0);
    vec3 num = vec3(0.8359375) + vec3(18.8515625) * pow(c, vec3(0.1593017578125));
    vec3 den = vec3(1.0) + vec3(18.6875) * pow(c, vec3(0.1593017578125));
    return pow(num / den, vec3(78.84375));
}

vec3 eotf_st240(vec3 c) {
    return mix(
        c * vec3(1.0 / 4.0),
        pow((c + vec3(0.1115)) * vec3(1.0 / 1.1115), vec3(1.0 / 0.45)),
        greaterThanEqual(c, vec3(0.0913))
    );
}

vec3 oetf_st240(vec3 c) {
    return mix(
        vec3(4.0) * c,
        vec3(1.1115) * pow(c, vec3(0.45)) - vec3(0.1115),
        greaterThanEqual(c, vec3(0.0228))
    );
}

vec3 eotf_log100(vec3 c) {
    return pow(vec3(10), vec3(2.0) * (c - vec3(1.0)));
}

vec3 oetf_log100(vec3 c) {
    c = clamp(c, 0.0, 1.0);
    return mix(
        vec3(0.0),
        vec3(1.0) + log2(c) / vec3(log2(10)) / vec3(2.0),
        greaterThanEqual(c, vec3(0.01))
    );
}

vec3 eotf_log316(vec3 c) {
    return pow(vec3(10), vec3(2.5) * (c - vec3(1.0)));
}

vec3 oetf_log316(vec3 c) {
    c = clamp(c, 0.0, 1.0);
    return mix(
        vec3(0.0),
        vec3(1.0) + log2(c) / vec3(log2(10)) / vec3(2.5),
        greaterThanEqual(c, vec3(sqrt(10) / 1000.0))
    );
}

vec3 eotf_st428(vec3 c) {
    c = max(c, 0.0);
    return pow(c, vec3(2.6)) * vec3(52.37 / 48.0);
}

vec3 oetf_st428(vec3 c) {
    c = max(c, 0.0);
    return pow(vec3(48.0) * c / vec3(52.37), vec3(1.0 / 2.6));
}

vec3 apply_eotf(vec3 c) {
    switch (eotf) {
        case TF_SRGB: return eotf_srgb(c);
        case TF_LINEAR: return c;
        case TF_ST2084_PQ: return eotf_st2084_pq(c);
        case TF_GAMMA24: return sign(c) * pow(abs(c), vec3(2.4));
        case TF_GAMMA22: return sign(c) * pow(abs(c), vec3(2.2));
        case TF_GAMMA28: return sign(c) * pow(abs(c), vec3(2.8));
        case TF_ST240: return eotf_st240(c);
        case TF_EXT_SRGB: return eotf_ext_srgb(c);
        case TF_LOG100: return eotf_log100(c);
        case TF_LOG316: return eotf_log316(c);
        case TF_ST428: return eotf_st428(c);
        default: return c;
    }
}

vec3 apply_oetf(vec3 c) {
    switch (oetf) {
        case TF_SRGB: return oetf_srgb(c);
        case TF_LINEAR: return c;
        case TF_ST2084_PQ: return oetf_st2084_pq(c);
        case TF_GAMMA24: return sign(c) * pow(abs(c), vec3(1.0 / 2.4));
        case TF_GAMMA22: return sign(c) * pow(abs(c), vec3(1.0 / 2.2));
        case TF_GAMMA28: return sign(c) * pow(abs(c), vec3(1.0 / 2.8));
        case TF_ST240: return oetf_st240(c);
        case TF_EXT_SRGB: return oetf_ext_srgb(c);
        case TF_LOG100: return oetf_log100(c);
        case TF_LOG316: return oetf_log316(c);
        case TF_ST428: return oetf_st428(c);
        default: return c;
    }
}

#endif
