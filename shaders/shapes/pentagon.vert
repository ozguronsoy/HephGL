#version 450

const float PI = 3.14159265358979323846;
const int SIDE_COUNT = 5;

void main()
{
    float angle =
        -0.5 * PI
        + 2.0 * PI * float(gl_VertexIndex) / float(SIDE_COUNT);

    vec2 position = vec2(
        cos(angle),
        sin(angle)
    ) * 0.3;

    position.y -= 0.5;

    gl_Position = vec4(position, 0.0, 1.0);
}