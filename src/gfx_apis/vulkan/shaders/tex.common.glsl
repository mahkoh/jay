#extension GL_EXT_buffer_reference : require

struct Vertex {
	vec2 pos[4];
	vec2 tex_pos[4];
};

layout(buffer_reference, buffer_reference_align = 8, std430) readonly buffer Vertices {
	Vertex vertices[];
};

layout(push_constant, std430) uniform Data {
	Vertices vertices;
	float mul;
} data;
