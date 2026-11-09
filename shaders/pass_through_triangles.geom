#version 450

// Passes triangles through unchanged, used by the pipeline tests.

layout(triangles) in;
layout(triangle_strip, max_vertices = 3) out;

layout(location = 0) in vec3 in_position[];
layout(location = 1) in vec3 in_normal[];
layout(location = 2) in vec2 in_uv[];

layout(location = 0) out vec3 frag_position;
layout(location = 1) out vec3 frag_normal;
layout(location = 2) out vec2 frag_uv;

void main()
{
    for (int i = 0; i < 3; ++i)
    {
        gl_Position = gl_in[i].gl_Position;
        frag_position = in_position[i];
        frag_normal = in_normal[i];
        frag_uv = in_uv[i];
        EmitVertex();
    }
    EndPrimitive();
}
