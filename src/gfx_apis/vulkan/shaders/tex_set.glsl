#ifndef TEX_SET_GLSL
#define TEX_SET_GLSL

layout(set = TEX_SET, binding = 0) uniform texture2D tex;
layout(set = TEX_SET, binding = 1, row_major, std430) uniform ColorManagementData {
    mat4x4 matrix;
} cm_data;
layout(set = TEX_SET, binding = 2, row_major, std430) uniform EotfArgs {
    float arg1;
    float arg2;
    float arg3;
    float arg4;
} cm_eotf_args;
layout(set = TEX_SET, binding = 3, row_major, std430) uniform InvEotfArgs {
    float arg1;
    float arg2;
    float arg3;
    float arg4;
} cm_inv_eotf_args;

#endif
