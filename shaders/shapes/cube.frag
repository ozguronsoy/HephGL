#version 450

layout(location = 0) in vec4 frag_color;
layout(location = 1) in vec3 frag_position;

layout(location = 0) out vec4 out_color;

void main()
{
    vec3 normal = normalize(cross(dFdx(frag_position), dFdy(frag_position)));
    float brightness = 0.4 + 0.5 * dot(
        normal,
        normalize(vec3(0.4, 0.7, 0.5))
    );
    out_color = vec4(frag_color.xyz * brightness, frag_color.w);
}