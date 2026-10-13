#version 450

layout(location = 1) in vec4 frag_color;
layout(location = 2) in vec3 frag_normal;

layout(location = 0) out vec4 out_color;

void main()
{
    vec3 normal = normalize(frag_normal);
    vec3 light_direction = normalize(vec3(-0.5, 0.7, -1.0));

    float diffuse = max(dot(normal, light_direction), 0.0);
    float lighting = 0.2 + diffuse * 0.8;

    out_color = vec4(frag_color.xyz * lighting, frag_color.w);
}
