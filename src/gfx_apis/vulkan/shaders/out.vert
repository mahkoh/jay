#version 450

#include "out.common.glsl"

void main() {
	vec2 pos = data.vertices.pos[gl_InstanceIndex][gl_VertexIndex];
	gl_Position = vec4(pos, 0.0, 1.0);
}
