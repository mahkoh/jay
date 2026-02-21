#version 450

layout(location = 0) in vec2 if_pos;
layout(location = 1) in vec2 it_pos;
layout(location = 2) in vec4 i_color;

layout(location = 0) out vec4 o_color;
layout(location = 1) out vec2 ot_pos;

void main() {
    o_color = i_color;
    o_color.rgb = mix(
        o_color.rgb / vec3(12.92),
        pow((o_color.rgb + vec3(0.055)) / vec3(1.055), vec3(2.4)),
        greaterThan(o_color.rgb, vec3(0.04045))
    );
    ot_pos = it_pos;
    gl_Position = vec4(if_pos.x, if_pos.y, 0.0, 1.0);
}
