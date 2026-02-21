#version 450

layout(location = 0) in vec4 color;
layout(location = 1) in vec2 pos;

layout(binding = 0, set = 0) uniform sampler2D tex;

layout(location = 0) out vec4 res;

void main() {
    vec4 src = texture(tex, pos);
    res = color * src;
}
