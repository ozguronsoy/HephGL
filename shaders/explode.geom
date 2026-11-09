#version 450

layout(triangles) in;
layout(triangle_strip, max_vertices = 3) out;

layout(set = 0, binding = 5, std140) uniform Matrices
{
    mat4 model;
    mat4 view_projection;
    mat4 normal;
} matrices;

layout(set = 0, binding = 6, std140) uniform Explosion
{
    vec3 impact_point;
    float radius;
    float time;
} explosion;

layout(location = 0) in vec3 in_position[];
layout(location = 1) in vec3 in_normal[];
layout(location = 2) in vec2 in_uv[];

layout(location = 0) out vec3 frag_position;
layout(location = 1) out vec3 frag_normal;
layout(location = 2) out vec2 frag_uv;

const float SPEED = 1.5;
const float GRAVITY = 0.8;
const float SPIN = 8.0;
const float LIFETIME = 2.5;

float random(uint seed)
{
    seed = (seed << 13u) ^ seed;
    seed = seed * (seed * seed * 15731u + 789221u) + 1376312589u;
    return float(seed & 0x7fffffffu) / float(0x7fffffff);
}

vec3 rotate(vec3 vector, vec3 axis, float angle)
{
    return vector * cos(angle) + cross(axis, vector) * sin(angle)
        + axis * dot(axis, vector) * (1.0 - cos(angle));
}

void emit(vec3 position, vec3 normal, vec2 uv)
{
    frag_position = position;
    frag_normal = normal;
    frag_uv = uv;
    gl_Position = matrices.view_projection * vec4(position, 1.0);
    EmitVertex();
}

void main()
{
    vec3 center = (in_position[0] + in_position[1] + in_position[2]) / 3.0;
    float strength = 1.0 - distance(center, explosion.impact_point) / explosion.radius;
    float time = max(explosion.time, 0.0);
    float scale = 1.0 - time / LIFETIME;
    // Triangles outside the blast radius are not affected.
    if (strength <= 0.0)
    {
        for (int i = 0; i < 3; ++i)
        {
            emit(in_position[i], in_normal[i], in_uv[i]);
        }
        EndPrimitive();
        return;
    }
    // Debris disappears at the end of its lifetime, leaving a hole behind.
    if (scale <= 0.0)
    {
        return;
    }

    vec3 face_normal = cross(in_position[1] - in_position[0], in_position[2] - in_position[0]);
    float length_squared = dot(face_normal, face_normal);
    face_normal = length_squared > 1e-12 ? face_normal * inversesqrt(length_squared) : vec3(0.0);
    // Point the face normal to the same side as the vertex normals regardless of winding order.
    if (dot(face_normal, in_normal[0] + in_normal[1] + in_normal[2]) < 0.0)
    {
        face_normal = -face_normal;
    }

    // Throw each triangle away from the impact point with a random speed and spin, and let
    // gravity pull it down.
    uint seed = uint(gl_PrimitiveIDIn) * 4u;
    vec3 direction = normalize(normalize(center - explosion.impact_point) + face_normal);
    float speed = SPEED * strength * (0.5 + random(seed));
    vec3 offset = direction * speed * time + vec3(0.0, -0.5 * GRAVITY * time * time, 0.0);
    vec3 axis = normalize(vec3(random(seed + 1u), random(seed + 2u), random(seed + 3u)) - 0.5);
    float angle = SPIN * strength * time;

    for (int i = 0; i < 3; ++i)
    {
        vec3 position = rotate(in_position[i] - center, axis, angle) * scale + center + offset;
        emit(position, rotate(in_normal[i], axis, angle), in_uv[i]);
    }
    EndPrimitive();
}
