#version 450

#include "tex.common.glsl"

layout(location = 0) out vec2 tex_pos;

void main() {
	Vertex vertex = data.vertices.vertices[gl_InstanceIndex];
	gl_Position = vec4(vertex.pos[gl_VertexIndex], 0.0, 1.0);
	tex_pos = vertex.tex_pos[gl_VertexIndex];
}
