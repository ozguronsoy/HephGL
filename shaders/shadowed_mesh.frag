#version 450

layout(set = 0, binding = 0) uniform texture2D image_texture;
layout(set = 0, binding = 1) uniform sampler image_sampler;
layout(set = 0, binding = 2) uniform texture2D arm_texture;
layout(set = 0, binding = 3) uniform texture2D normal_texture;
layout(set = 0, binding = 4, std140) uniform MaterialData
{
    vec4 base_color;
    vec4 parameters;
} material;
layout(set = 0, binding = 6, std140) uniform SceneData
{
    mat4 light_view_projection;
    vec4 light_direction;
    vec4 light_color;
    vec4 ambient_color;
    vec4 camera_position;
} scene;
layout(set = 0, binding = 7) uniform texture2D shadow_map;
layout(set = 0, binding = 8) uniform samplerShadow shadow_sampler;

layout(location = 0) in vec3 frag_position;
layout(location = 1) in vec3 frag_normal;
layout(location = 2) in vec2 frag_uv;

layout(location = 0) out vec4 out_color;

const float PI = 3.14159265358979323846;
// Moves the shadow lookup along the surface normal, so surfaces don't shadow themselves.
const float SHADOW_NORMAL_OFFSET = 0.04;

vec3 normalize_or(vec3 vector, vec3 fallback)
{
    float length_squared = dot(vector, vector);
    return length_squared > 1e-12 ? vector * inversesqrt(length_squared) : fallback;
}

vec3 surface_normal(vec3 normal)
{
    // Two-channel normal textures store only X and Y, Z is reconstructed from them.
    vec3 tangent_normal;
    tangent_normal.xy = texture(sampler2D(normal_texture, image_sampler), frag_uv).xy * 2.0 - 1.0;
    tangent_normal.z = sqrt(max(1.0 - dot(tangent_normal.xy, tangent_normal.xy), 0.0));
    tangent_normal.xy *= material.parameters.z;

    vec3 position_dx = dFdx(frag_position);
    vec3 position_dy = dFdy(frag_position);
    vec2 uv_dx = dFdx(frag_uv);
    vec2 uv_dy = dFdy(frag_uv);
    float determinant = uv_dx.x * uv_dy.y - uv_dx.y * uv_dy.x;
    if (abs(determinant) <= 1e-10)
    {
        return normal;
    }

    vec3 tangent = (position_dx * uv_dy.y - position_dy * uv_dx.y) / determinant;
    vec3 bitangent = (position_dy * uv_dx.x - position_dx * uv_dy.x) / determinant;
    tangent = normalize_or(tangent - normal * dot(normal, tangent), vec3(0.0));
    bitangent -= normal * dot(normal, bitangent) + tangent * dot(tangent, bitangent);
    bitangent = normalize_or(bitangent, vec3(0.0));
    if (dot(tangent, tangent) == 0.0 || dot(bitangent, bitangent) == 0.0)
    {
        return normal;
    }

    return normalize_or(mat3(tangent, bitangent, normal) * tangent_normal, normal);
}

// Returns how much of the fragment the sun reaches, between 0 (fully shadowed) and 1 (fully lit).
float sun_visibility(vec3 geometry_normal)
{
    vec4 light_position = scene.light_view_projection
        * vec4(frag_position + geometry_normal * SHADOW_NORMAL_OFFSET, 1.0);
    vec3 shadow_coordinates = light_position.xyz / light_position.w;
    shadow_coordinates.xy = shadow_coordinates.xy * 0.5 + 0.5;
    // Everything outside the shadow map is lit.
    if (!(shadow_coordinates.x >= 0.0 && shadow_coordinates.x <= 1.0
        && shadow_coordinates.y >= 0.0 && shadow_coordinates.y <= 1.0
        && shadow_coordinates.z <= 1.0))
    {
        return 1.0;
    }

    // The comparison sampler compares the fragment's depth with the shadow map and filters the
    // results of the four nearest texels. Averaging a 3x3 grid of lookups softens the edges more.
    vec2 texel_size = 1.0 / vec2(textureSize(sampler2DShadow(shadow_map, shadow_sampler), 0));
    float visibility = 0.0;
    for (int y = -1; y <= 1; y++)
    {
        for (int x = -1; x <= 1; x++)
        {
            vec2 offset = vec2(x, y) * texel_size;
            visibility += texture(
                sampler2DShadow(shadow_map, shadow_sampler),
                vec3(shadow_coordinates.xy + offset, shadow_coordinates.z)
            );
        }
    }
    return visibility / 9.0;
}

// Maps high dynamic range colors into the displayable range (Narkowicz's ACES approximation).
vec3 tone_map(vec3 color)
{
    return clamp((color * (2.51 * color + 0.03)) / (color * (2.43 * color + 0.59) + 0.14), 0.0, 1.0);
}

void main()
{
    vec4 base_color = texture(sampler2D(image_texture, image_sampler), frag_uv) * material.base_color;
    // Alpha-masked surfaces, such as leaves, are cut out instead of blended.
    if (base_color.a < 0.5)
    {
        discard;
    }
    vec3 arm = texture(sampler2D(arm_texture, image_sampler), frag_uv).rgb;
    // Single-channel textures store only the roughness.
    if (material.parameters.w != 0.0)
    {
        arm = vec3(1.0, arm.r, 0.0);
    }
    float roughness = clamp(arm.g * material.parameters.x, 0.045, 1.0);
    float metallicness = clamp(arm.b * material.parameters.y, 0.0, 1.0);

    vec3 geometry_normal = normalize_or(frag_normal, vec3(0.0, 1.0, 0.0));
    vec3 view_direction = normalize_or(scene.camera_position.xyz - frag_position, geometry_normal);
    // Reverse the normals on back faces for double-sided materials.
    if (dot(geometry_normal, view_direction) < 0.0)
    {
        geometry_normal = -geometry_normal;
    }
    vec3 normal = surface_normal(geometry_normal);

    vec3 light_direction = normalize_or(-scene.light_direction.xyz, vec3(0.0, 1.0, 0.0));
    vec3 half_direction = normalize_or(view_direction + light_direction, normal);
    float normal_dot_view = max(dot(normal, view_direction), 0.0);
    float normal_dot_light = max(dot(normal, light_direction), 0.0);
    float normal_dot_half = clamp(dot(normal, half_direction), 0.0, 1.0);
    float view_dot_half = clamp(dot(view_direction, half_direction), 0.0, 1.0);

    vec3 color = scene.ambient_color.rgb * base_color.rgb * arm.r;
    if (normal_dot_view > 0.0 && normal_dot_light > 0.0)
    {
        float alpha = roughness * roughness;
        float alpha_squared = alpha * alpha;
        float denominator = normal_dot_half * normal_dot_half * (alpha_squared - 1.0) + 1.0;
        float distribution = alpha_squared / (PI * denominator * denominator);
        float view_mask = sqrt(normal_dot_view * normal_dot_view * (1.0 - alpha_squared) + alpha_squared);
        float light_mask = sqrt(normal_dot_light * normal_dot_light * (1.0 - alpha_squared) + alpha_squared);
        float visibility = 0.5 / max(normal_dot_light * view_mask + normal_dot_view * light_mask, 1e-7);
        vec3 reflectance = mix(vec3(0.04), base_color.rgb, metallicness);
        vec3 fresnel = reflectance + (vec3(1.0) - reflectance) * pow(1.0 - view_dot_half, 5.0);
        vec3 diffuse = (vec3(1.0) - fresnel) * (1.0 - metallicness) * base_color.rgb / PI;
        vec3 specular = distribution * visibility * fresnel;
        vec3 radiance = scene.light_color.rgb * scene.light_color.w;
        color += (diffuse + specular) * radiance * normal_dot_light * sun_visibility(geometry_normal);
    }

    out_color = vec4(tone_map(color), 1.0);
}
