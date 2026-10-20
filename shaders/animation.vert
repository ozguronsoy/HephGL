#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;
layout(location = 3) in vec4 in_joints;
layout(location = 4) in vec4 in_weights;

layout(set = 0, binding = 5, std140) uniform Matrices
{
    mat4 model;
    mat4 view_projection;
    mat4 normal;
} matrices;

layout(set = 0, binding = 6, std430) readonly buffer JointMatrices
{
    mat4 values[];
} joints;

layout(location = 0) out vec3 frag_position;
layout(location = 1) out vec3 frag_normal;
layout(location = 2) out vec2 frag_uv;

void main()
{
    ivec4 indices = ivec4(in_joints);
    mat4 skin = joints.values[indices.x] * in_weights.x
        + joints.values[indices.y] * in_weights.y
        + joints.values[indices.z] * in_weights.z
        + joints.values[indices.w] * in_weights.w;
    vec4 world_position = matrices.model * skin * vec4(in_position, 1.0);
    gl_Position = matrices.view_projection * world_position;
    mat3 skin_normal = mat3(skin);
    if (abs(determinant(skin_normal)) > 1e-8)
    {
        skin_normal = transpose(inverse(skin_normal));
    }
    frag_position = world_position.xyz;
    frag_normal = mat3(matrices.normal) * skin_normal * in_normal;
    frag_uv = in_uv;
}
