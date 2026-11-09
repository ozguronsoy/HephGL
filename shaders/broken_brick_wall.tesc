#version 450

layout(vertices = 3) out;

layout(location = 0) in vec3 in_position[];
layout(location = 1) in vec3 in_normal[];
layout(location = 2) in vec2 in_uv[];
layout(location = 3) in vec3 in_center[];

layout(location = 0) out vec3 out_position[];
layout(location = 1) out vec3 out_normal[];
layout(location = 2) out vec2 out_uv[];
layout(location = 3) out vec3 out_center[];

// The number of segments per unit of edge length, so larger spheres are subdivided further.
const float SEGMENTS_PER_UNIT = 320.0;
// The minimum tessellation level every device must support.
const float MAX_LEVEL = 64.0;

float edge_level(vec3 start, vec3 end)
{
    return clamp(distance(start, end) * SEGMENTS_PER_UNIT, 1.0, MAX_LEVEL);
}

void main()
{
    out_position[gl_InvocationID] = in_position[gl_InvocationID];
    out_normal[gl_InvocationID] = in_normal[gl_InvocationID];
    out_uv[gl_InvocationID] = in_uv[gl_InvocationID];
    out_center[gl_InvocationID] = in_center[gl_InvocationID];

    if (gl_InvocationID == 0)
    {
        // Each outer level belongs to the edge opposite to the vertex with the same index.
        // Neighboring patches compute the same level for a shared edge, which avoids cracks.
        float level_0 = edge_level(in_position[1], in_position[2]);
        float level_1 = edge_level(in_position[2], in_position[0]);
        float level_2 = edge_level(in_position[0], in_position[1]);
        gl_TessLevelOuter[0] = level_0;
        gl_TessLevelOuter[1] = level_1;
        gl_TessLevelOuter[2] = level_2;
        gl_TessLevelInner[0] = max(max(level_0, level_1), level_2);
    }
}
