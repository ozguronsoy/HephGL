#version 450

layout(location = 0) in vec3 frag_color;

layout(location = 0) out vec4 out_color;
layout(location = 1) out vec4 out_inverted_color;
layout(location = 2) out vec4 out_swizzled_color;
layout(location = 3) out float out_luminance;

void main()
{
    out_color = vec4(frag_color, 1.0);
    out_inverted_color = vec4(1.0 - frag_color, 1.0);
    out_swizzled_color = vec4(frag_color.brg, 1.0);
    out_luminance = dot(frag_color, vec3(0.2126, 0.7152, 0.0722));
}
