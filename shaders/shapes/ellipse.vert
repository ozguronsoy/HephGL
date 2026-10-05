#version 450

const float PI = 3.14159265358979323846;
const int SEGMENT_COUNT = 256;

const float radiusX = 0.3;
const float radiusY = 0.2;
const float aspectRatio = 1280.0 / 720.0;

void main()
{
    float angle =
        2.0 * PI * float(gl_VertexIndex) / float(SEGMENT_COUNT);

    vec2 position = vec2(
        cos(angle) * radiusX / aspectRatio,
        sin(angle) * radiusY
    );

    position.x -= 0.4;
    position.y -= 0.3;

    gl_Position = vec4(position, 0.0, 1.0);
}