#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_color;
layout(location = 2) in vec3 in_offset;

layout(location = 0) out vec3 frag_color;
layout(location = 1) out vec3 frag_position;

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

    float scale = 1.0 - in_offset.z;
    position *= 0.55 * scale;
    position.xy += in_offset.xy;

    float near = 0.1;
    float far = 10.0;
    float depth = position.z + 2.0 + in_offset.z;
    float focal_length = 2.0;

    float z = far / (far - near) * depth - (far * near) / (far - near);

    frag_position = position;

    gl_Position = vec4(
        position.x * focal_length,
        position.y * focal_length,
        z,
        depth
    );

    frag_color = in_color;
}