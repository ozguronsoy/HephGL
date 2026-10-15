#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;

layout(location = 0) out vec3 frag_position;
layout(location = 1) out vec3 frag_normal;
layout(location = 2) out vec2 frag_uv;

const float ASPECT_RATIO = 16.0 / 9.0;
const float ROTATION_X_DEGREES = 0.0;
const float ROTATION_Y_DEGREES = 0.0;
const float ROTATION_Z_DEGREES = 0.0;

// Match the sphere vertex count and centers in render_textures.rs.
const int SPHERE_VERTEX_COUNT = 512 * 1024 * 6;
const float SPHERE_CENTERS_X[3] = float[](-1.2, -0.4, 0.8);

void main()
{
    vec3 angles = radians(vec3(ROTATION_X_DEGREES, ROTATION_Y_DEGREES, ROTATION_Z_DEGREES));
    vec3 cosine = cos(angles);
    vec3 sine = sin(angles);
    mat3 rotation_x = mat3(
        vec3(1.0, 0.0, 0.0),
        vec3(0.0, cosine.x, sine.x),
        vec3(0.0, -sine.x, cosine.x)
    );
    mat3 rotation_y = mat3(
        vec3(cosine.y, 0.0, -sine.y),
        vec3(0.0, 1.0, 0.0),
        vec3(sine.y, 0.0, cosine.y)
    );
    mat3 rotation_z = mat3(
        vec3(cosine.z, sine.z, 0.0),
        vec3(-sine.z, cosine.z, 0.0),
        vec3(0.0, 0.0, 1.0)
    );

    mat3 rotation = rotation_z * rotation_y * rotation_x;
    vec3 center = vec3(SPHERE_CENTERS_X[gl_VertexIndex / SPHERE_VERTEX_COUNT], 0.0, 0.0);
    vec3 rotated_position = center + rotation * (in_position - center);
    vec3 position = rotated_position;
    position.x /= ASPECT_RATIO;

    gl_Position = vec4(
        position.x,
        -position.y,
        position.z * 0.5 + 0.5,
        1.0
    );

    frag_position = rotated_position;
    frag_normal = rotation * in_normal;
    frag_uv = in_uv;
}
