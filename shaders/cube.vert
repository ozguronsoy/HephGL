#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_color;

layout(location = 0) out vec3 frag_color;

void main()
{
    vec3 position = in_position;

    float y_angle = 0.6;
    float cy = cos(y_angle);
    float sy = sin(y_angle);

    position = vec3(
         cy * position.x + sy * position.z,
         position.y,
        -sy * position.x + cy * position.z
    );

    float x_angle = -0.4;
    float cx = cos(x_angle);
    float sx = sin(x_angle);

    position = vec3(
        position.x,
        cx * position.y - sx * position.z,
        sx * position.y + cx * position.z
    );

    position *= 0.55;
    position.x += 0.25;
    position.z = position.z * 0.5 + 0.5;

    gl_Position = vec4(position, 1.0);
    frag_color = in_color;
}