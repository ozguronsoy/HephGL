#version 450

const float PI = 3.14159265358979323846;
const int SEGMENT_COUNT = 64;

void main()
{
    float angle =
        2.0 * PI * float(gl_VertexIndex) / float(SEGMENT_COUNT);

    vec2 position = vec2(
        cos(angle) * 0.4,
        sin(angle) * 0.25
    );

    position.y -= 0.15;

    gl_Position = vec4(position, 0.0, 1.0);
}