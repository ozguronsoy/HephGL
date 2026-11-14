#version 450

layout(set = 0, binding = 0) uniform texture2D image_texture;
layout(set = 0, binding = 1) uniform sampler image_sampler;

layout(location = 2) in vec2 frag_uv;

void main()
{
    // Alpha-masked surfaces, such as leaves, only cast shadows where they are opaque.
    if (texture(sampler2D(image_texture, image_sampler), frag_uv).a < 0.5)
    {
        discard;
    }
}
