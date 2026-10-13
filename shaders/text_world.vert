#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
layout(location = 3) in vec4 in_color;

layout(set = 0, binding = 2, std140) uniform Matrices
{
    mat4 model;
    mat4 view_projection;
    mat4 normal;
} matrices;

layout(location = 0) out vec2 frag_uv;
layout(location = 1) out vec4 frag_color;
layout(location = 2) out vec3 frag_normal;

void main()
{
    gl_Position = matrices.view_projection * matrices.model * vec4(in_position, 1.0);
    frag_uv = in_uv;
    frag_color = in_color;
    frag_normal = mat3(matrices.normal) * in_normal;
}
