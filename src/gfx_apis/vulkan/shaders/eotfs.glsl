#ifndef EOTFS_GLSL
#define EOTFS_GLSL

#include "frag_spec_const.glsl"
#include "tex_set.glsl"

#define TF_LINEAR 1
#define TF_ST2084_PQ 2
#define TF_BT1886 3
#define TF_GAMMA22 4
#define TF_GAMMA28 5
#define TF_ST240 6
#define TF_LOG100 8
#define TF_LOG316 9
#define TF_ST428 10
#define TF_POW 11
#define TF_GAMMA24 12

vec3 eotf_bt1886(vec3 c) {
    c = clamp(c, 0.0, 1.0);
    float a1 = cm_eotf_args.arg1;
    float a2 = cm_eotf_args.arg2;
    float a3 = cm_eotf_args.arg3;
    float a4 = cm_eotf_args.arg4;
    return a1 * (pow(a2 * c + a3, vec3(2.4)) - a4);
}

vec3 inv_eotf_bt1886(vec3 c) {
    c = clamp(c, 0.0, 1.0);
    float a1 = cm_inv_eotf_args.arg1;
    float a2 = cm_inv_eotf_args.arg2;
    float a3 = cm_inv_eotf_args.arg3;
    float a4 = cm_inv_eotf_args.arg4;
    return a1 * (pow(a2 * c + a3, vec3(1.0 / 2.4)) - a4);
}

vec3 eotf_st2084_pq(vec3 c) {
    c = clamp(c, 0.0, 1.0);
    vec3 cp = pow(c, vec3(1.0 / 78.84375));
    vec3 num = max(cp - vec3(0.8359375), 0.0);
    vec3 den = vec3(18.8515625) - vec3(18.6875) * cp;
    return pow(num / den, vec3(1.0 / 0.1593017578125));
}

vec3 inv_eotf_st2084_pq(vec3 c) {
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

vec3 inv_eotf_st240(vec3 c) {
    return mix(
        vec3(4.0) * c,
        vec3(1.1115) * pow(c, vec3(0.45)) - vec3(0.1115),
        greaterThanEqual(c, vec3(0.0228))
    );
}

vec3 eotf_log100(vec3 c) {
    return pow(vec3(10), vec3(2.0) * (c - vec3(1.0)));
}

vec3 inv_eotf_log100(vec3 c) {
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

vec3 inv_eotf_log316(vec3 c) {
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

vec3 inv_eotf_st428(vec3 c) {
    c = max(c, 0.0);
    return pow(vec3(48.0) * c / vec3(52.37), vec3(1.0 / 2.6));
}

vec3 apply_eotf(vec3 c) {
    switch (eotf) {
        case TF_LINEAR: return c;
        case TF_ST2084_PQ: return eotf_st2084_pq(c);
        case TF_BT1886: return eotf_bt1886(c);
        case TF_GAMMA22: return sign(c) * pow(abs(c), vec3(2.2));
        case TF_GAMMA24: return sign(c) * pow(abs(c), vec3(2.4));
        case TF_GAMMA28: return sign(c) * pow(abs(c), vec3(2.8));
        case TF_ST240: return eotf_st240(c);
        case TF_LOG100: return eotf_log100(c);
        case TF_LOG316: return eotf_log316(c);
        case TF_ST428: return eotf_st428(c);
        case TF_POW: return sign(c) * pow(abs(c), vec3(cm_eotf_args.arg1));
        default: return c;
    }
}

vec3 apply_inv_eotf(vec3 c) {
    switch (inv_eotf) {
        case TF_LINEAR: return c;
        case TF_ST2084_PQ: return inv_eotf_st2084_pq(c);
        case TF_BT1886: return inv_eotf_bt1886(c);
        case TF_GAMMA22: return sign(c) * pow(abs(c), vec3(1.0 / 2.2));
        case TF_GAMMA24: return sign(c) * pow(abs(c), vec3(1.0 / 2.4));
        case TF_GAMMA28: return sign(c) * pow(abs(c), vec3(1.0 / 2.8));
        case TF_ST240: return inv_eotf_st240(c);
        case TF_LOG100: return inv_eotf_log100(c);
        case TF_LOG316: return inv_eotf_log316(c);
        case TF_ST428: return inv_eotf_st428(c);
        case TF_POW: return sign(c) * pow(abs(c), vec3(cm_inv_eotf_args.arg1));
        default: return c;
    }
}

#endif
