#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;

layout(location = 0) out vec3 frag_normal;
layout(location = 1) out vec2 frag_uv;

const float ASPECT_RATIO = 16.0 / 9.0;

void main()
{
    vec3 position = in_position;
    position.x /= ASPECT_RATIO;

    gl_Position = vec4(
        position.x,
        position.y,
        position.z * 0.5 + 0.5,
        1.0
    );

    frag_normal = in_normal;
    frag_uv = in_uv;
}