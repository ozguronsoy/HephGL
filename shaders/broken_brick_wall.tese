#version 450

layout(triangles, equal_spacing, ccw) in;

layout(set = 0, binding = 1) uniform sampler image_sampler;
layout(set = 0, binding = 4, std140) uniform MaterialData
{
    vec4 base_color;
    vec4 parameters;
    vec4 light_direction;
    vec4 light_color;
    vec4 ambient_color;
    vec4 view_direction;
    vec4 displacement;
} material;
layout(set = 0, binding = 5) uniform texture2D displacement_texture;

layout(location = 0) in vec3 in_position[];
layout(location = 1) in vec3 in_normal[];
layout(location = 2) in vec2 in_uv[];
layout(location = 3) in vec3 in_center[];

layout(location = 0) out vec3 frag_position;
layout(location = 1) out vec3 frag_normal;
layout(location = 2) out vec2 frag_uv;

const float ASPECT_RATIO = 16.0 / 9.0;
const float ROTATION_X_DEGREES = 0.0;
const float ROTATION_Y_DEGREES = 0.0;
const float ROTATION_Z_DEGREES = 0.0;

void main()
{
    vec3 weights = gl_TessCoord;
    vec3 normal = normalize(
        weights.x * in_normal[0] + weights.y * in_normal[1] + weights.z * in_normal[2]
    );
    vec2 uv = weights.x * in_uv[0] + weights.y * in_uv[1] + weights.z * in_uv[2];

    // Every triangle touching a pole has its own U coordinate there, so sample the same texel for
    // all of them to keep the pole closed.
    vec2 height_uv = uv;
    if (uv.y <= 0.0 || uv.y >= 1.0)
    {
        height_uv.x = 0.0;
    }
    float height = textureLod(sampler2D(displacement_texture, image_sampler), height_uv, 0.0).r;
    float offset = height * material.displacement.x + material.displacement.y;

    // Project the subdivided point back onto the sphere, then displace it relative to the radius
    // so all sphere sizes have the same relative relief.
    vec3 center = in_center[0];
    float radius = distance(in_position[0], center);
    vec3 displaced_position = center + normal * radius * (1.0 + offset);

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
    vec3 rotated_position = center + rotation * (displaced_position - center);
    vec3 position = rotated_position;
    position.x /= ASPECT_RATIO;

    gl_Position = vec4(
        position.x,
        -position.y,
        position.z * 0.5 + 0.5,
        1.0
    );

    frag_position = rotated_position;
    frag_normal = rotation * normal;
    frag_uv = uv;
}
