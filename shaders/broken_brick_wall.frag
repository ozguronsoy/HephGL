#version 450

layout(set = 0, binding = 0) uniform texture2D image_texture;
layout(set = 0, binding = 1) uniform sampler image_sampler;

layout(location = 0) in vec3 frag_normal;
layout(location = 1) in vec2 frag_uv;

layout(location = 0) out vec4 out_color;

void main()
{
    vec3 normal = normalize(frag_normal);
    vec3 light_direction = normalize(vec3(-0.5, 0.7, -1.0));

    float diffuse = max(dot(normal, light_direction), 0.0);
    float lighting = 0.2 + diffuse * 0.8;

    vec4 color = texture(sampler2D(image_texture, image_sampler), frag_uv);

    out_color = vec4(color.xyz * lighting, color.w);
}