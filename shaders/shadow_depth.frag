#version 450

layout(set = 0, binding = 0) uniform texture2D image_texture;
layout(set = 0, binding = 1) uniform sampler image_sampler;
layout(set = 0, binding = 4, std140) uniform MaterialData
{
    vec4 base_color;
    vec4 parameters;
    float alpha_cutoff;
} material;

layout(location = 2) in vec2 frag_uv;

void main()
{
    // Alpha-masked surfaces, such as leaves, only cast shadows where they are opaque.
    float alpha = texture(sampler2D(image_texture, image_sampler), frag_uv).a * material.base_color.a;
    if (alpha < material.alpha_cutoff)
    {
        discard;
    }
}
