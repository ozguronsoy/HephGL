#version 450

layout(set = 0, binding = 0) uniform texture2D font_texture;
layout(set = 0, binding = 1) uniform sampler font_sampler;

layout(location = 0) in vec2 frag_uv;
layout(location = 1) in vec4 frag_color;

layout(location = 0) out vec4 out_color;

void main()
{
    float coverage = texture(sampler2D(font_texture, font_sampler), frag_uv).r;
    out_color = vec4(frag_color.xyz, frag_color.w * coverage);
}