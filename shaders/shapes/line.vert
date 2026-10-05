#version 450

vec2 positions[2] = vec2[](
    vec2(-0.4, 0.6),
    vec2(0.4, 0.6)
);

void main()
{
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
}