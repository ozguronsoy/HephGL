#version 450

layout(location = 0) flat out int face_index;

vec3 vertices[8] = vec3[](
    vec3(-0.5, -0.5, -0.5),
    vec3( 0.5, -0.5, -0.5),
    vec3( 0.5,  0.5, -0.5),
    vec3(-0.5,  0.5, -0.5),

    vec3(-0.5, -0.5,  0.5),
    vec3( 0.5, -0.5,  0.5),
    vec3( 0.5,  0.5,  0.5),
    vec3(-0.5,  0.5,  0.5)
);

int indices[36] = int[](
    0, 1, 2,
    2, 3, 0,
    4, 6, 5,
    6, 4, 7,
    0, 3, 7,
    7, 4, 0,
    1, 5, 6,
    6, 2, 1,
    0, 4, 5,
    5, 1, 0,
    3, 2, 6,
    6, 7, 3
);

void main()
{
    vec3 position = vertices[indices[gl_VertexIndex]];

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

    face_index = gl_VertexIndex / 6;
}