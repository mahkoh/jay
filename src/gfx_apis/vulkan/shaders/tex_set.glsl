#ifndef TEX_SET_GLSL
#define TEX_SET_GLSL

layout(set = TEX_SET, binding = 0) uniform texture2D tex;
layout(set = TEX_SET, binding = 1, row_major, std430) uniform ColorManagementData {
    mat4x4 matrix;
} cm_data;

#endif
