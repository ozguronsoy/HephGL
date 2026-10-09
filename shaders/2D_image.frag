#version 450

layout(set = 0, binding = 0) uniform texture2D image_texture;
layout(set = 0, binding = 1) uniform sampler image_sampler;

layout(location = 0) in vec2 frag_uv;
layout(location = 0) out vec4 out_color;

void main()
{
    out_color = texture(sampler2D(image_texture, image_sampler), frag_uv);
}