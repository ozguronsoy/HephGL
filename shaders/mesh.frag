#version 450

layout(set = 0, binding = 0) uniform texture2D image_texture;
layout(set = 0, binding = 1) uniform sampler image_sampler;
layout(set = 0, binding = 2) uniform texture2D arm_texture;
layout(set = 0, binding = 3) uniform texture2D normal_texture;
layout(set = 0, binding = 4, std140) uniform MaterialData
{
    vec4 base_color;
    vec4 parameters;
    vec4 light_direction;
    vec4 light_color;
    vec4 ambient_color;
    vec4 view_direction;
} material;

layout(location = 0) in vec3 frag_position;
layout(location = 1) in vec3 frag_normal;
layout(location = 2) in vec2 frag_uv;

layout(location = 0) out vec4 out_color;

const float PI = 3.14159265358979323846;

vec3 normalize_or(vec3 vector, vec3 fallback)
{
    float length_squared = dot(vector, vector);
    return length_squared > 1e-12 ? vector * inversesqrt(length_squared) : fallback;
}

vec3 surface_normal()
{
    vec3 normal = normalize_or(frag_normal, vec3(0.0, 0.0, -1.0));
    vec3 tangent_normal = texture(sampler2D(normal_texture, image_sampler), frag_uv).xyz * 2.0 - 1.0;
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

void main()
{
    vec4 base_color = texture(sampler2D(image_texture, image_sampler), frag_uv) * material.base_color;
    vec3 arm = texture(sampler2D(arm_texture, image_sampler), frag_uv).rgb;
    float roughness = arm.g * material.parameters.x;
    float metallicness = clamp(arm.b * material.parameters.y, 0.0, 1.0);
    roughness = clamp(roughness, 0.045, 1.0);

    vec3 normal = surface_normal();
    // The chest's material is double-sided, so reverse the shading normal on back faces.
    if (!gl_FrontFacing)
    {
        normal = -normal;
    }
    vec3 view_direction = normalize_or(
        material.view_direction.xyz - frag_position * material.view_direction.w,
        normal
    );
    vec3 light_direction = normalize_or(-material.light_direction.xyz, vec3(0.0));
    vec3 half_direction = normalize_or(view_direction + light_direction, normal);
    float normal_dot_view = max(dot(normal, view_direction), 0.0);
    float normal_dot_light = max(dot(normal, light_direction), 0.0);
    float normal_dot_half = clamp(dot(normal, half_direction), 0.0, 1.0);
    float view_dot_half = clamp(dot(view_direction, half_direction), 0.0, 1.0);

    vec3 color = material.ambient_color.rgb * base_color.rgb * arm.r;
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
        vec3 radiance = material.light_color.rgb * material.light_color.w;
        color += (diffuse + specular) * radiance * normal_dot_light;
    }

    out_color = vec4(color, base_color.a);
}
