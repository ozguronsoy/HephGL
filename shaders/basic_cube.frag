#version 450

layout(location = 0) flat in int face_index;
layout(location = 0) out vec4 out_color;

void main()
{
    float red = 0.25 + float(face_index) * 0.05;
    out_color = vec4(red, 0.0, 0.0, 1.0);
}