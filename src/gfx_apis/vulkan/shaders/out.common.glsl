#extension GL_EXT_buffer_reference : require

layout(buffer_reference, buffer_reference_align = 8, std430) readonly buffer Vertices {
	vec2 pos[][4];
};

layout(push_constant, std430) uniform Data {
	Vertices vertices;
} data;
