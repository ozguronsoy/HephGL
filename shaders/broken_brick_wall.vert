#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;

layout(location = 0) out vec3 out_position;
layout(location = 1) out vec3 out_normal;
layout(location = 2) out vec2 out_uv;
layout(location = 3) out vec3 out_center;

// Match the sphere vertex count and centers in render_textures.rs.
const int SPHERE_VERTEX_COUNT = 32 * 64 * 6;
const float SPHERE_CENTERS_X[3] = float[](-1.2, -0.4, 0.8);

void main()
{
    // The tessellation shaders subdivide, displace, and project the vertices.
    out_position = in_position;
    out_normal = in_normal;
    out_uv = in_uv;
    out_center = vec3(SPHERE_CENTERS_X[gl_VertexIndex / SPHERE_VERTEX_COUNT], 0.0, 0.0);
}
