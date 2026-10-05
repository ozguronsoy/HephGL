#version 450

const float PI = 3.14159265358979323846;

vec2 pentagon_vertex(int index)
{
    float angle = -0.5 * PI + 2.0 * PI * float(index) / 5.0;

    return vec2(
        cos(angle),
        sin(angle)
    ) * 0.3;
}

void main()
{
    int order[5] = int[](
        0,
        1,
        4,
        2,
        3
    );

    vec2 position = pentagon_vertex(order[gl_VertexIndex]);
    position += vec2(0.5, -0.4);

    gl_Position = vec4(position, 0.0, 1.0);
}