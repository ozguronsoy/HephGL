#version 450

// Passes points through unchanged, used by the pipeline tests.

layout(points) in;
layout(points, max_vertices = 1) out;

layout(location = 0) in vec3 in_position[];
layout(location = 1) in vec3 in_normal[];
layout(location = 2) in vec2 in_uv[];

layout(location = 0) out vec3 frag_position;
layout(location = 1) out vec3 frag_normal;
layout(location = 2) out vec2 frag_uv;

void main()
{
    gl_Position = gl_in[0].gl_Position;
    frag_position = in_position[0];
    frag_normal = in_normal[0];
    frag_uv = in_uv[0];
    EmitVertex();
    EndPrimitive();
}
