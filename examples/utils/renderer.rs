#![allow(dead_code)]

use heph_gl::{
    GraphicsDevice,
    GraphicsDeviceType::{DiscreteGpu, IntegratedGpu},
    MaterialProperties,
    renderers::{Msaa, Renderer},
};

#[cfg(feature = "vulkan")]
pub type ExampleRenderer = heph_gl::renderers::vulkan::VulkanRenderer;

#[cfg(not(any(
    feature = "vulkan",
    feature = "directx",
    feature = "metal",
    feature = "opengl"
)))]
pub type ExampleRenderer = heph_gl::renderers::vulkan::VulkanRenderer;

// [position.x, position.y, position.z, normal.x, normal.y, normal.z, tex_coord.u, tex_coord.v]
pub type SphereVertex = [f32; 8];
pub type HeightMap = image::ImageBuffer<image::Luma<u16>, Vec<u16>>;

const SPHERE_LATITUDE_SEGMENTS: usize = 512;
const SPHERE_LONGITUDE_SEGMENTS: usize = 1024;
// The equatorial circumference is twice the pole-to-pole distance, so repeat the square maps twice.
const SPHERE_TEXTURE_REPEATS_U: f32 = 2.0;

pub fn sphere_vertex_count() -> usize {
    SPHERE_LATITUDE_SEGMENTS * SPHERE_LONGITUDE_SEGMENTS * 6 * 3
}

pub fn sphere_vertices(radius: f32, x_offset: f32) -> Vec<SphereVertex> {
    let mut vertices = Vec::with_capacity(SPHERE_LATITUDE_SEGMENTS * SPHERE_LONGITUDE_SEGMENTS * 6);

    for y in 0..SPHERE_LATITUDE_SEGMENTS {
        let v0 = y as f32 / SPHERE_LATITUDE_SEGMENTS as f32;
        let v1 = (y + 1) as f32 / SPHERE_LATITUDE_SEGMENTS as f32;

        for x in 0..SPHERE_LONGITUDE_SEGMENTS {
            let u0 = x as f32 / SPHERE_LONGITUDE_SEGMENTS as f32;
            let u1 = (x + 1) as f32 / SPHERE_LONGITUDE_SEGMENTS as f32;

            let p00 = sphere_position(radius, x_offset, u0, v0);
            let p10 = sphere_position(radius, x_offset, u1, v0);
            let p01 = sphere_position(radius, x_offset, u0, v1);
            let p11 = sphere_position(radius, x_offset, u1, v1);

            let n00 = [
                (p00[0] - x_offset) / radius,
                p00[1] / radius,
                p00[2] / radius,
            ];
            let n10 = [
                (p10[0] - x_offset) / radius,
                p10[1] / radius,
                p10[2] / radius,
            ];
            let n01 = [
                (p01[0] - x_offset) / radius,
                p01[1] / radius,
                p01[2] / radius,
            ];
            let n11 = [
                (p11[0] - x_offset) / radius,
                p11[1] / radius,
                p11[2] / radius,
            ];
            let texture_u0 = u0 * SPHERE_TEXTURE_REPEATS_U;
            let texture_u1 = u1 * SPHERE_TEXTURE_REPEATS_U;

            vertices.extend_from_slice(&[
                [
                    p00[0], p00[1], p00[2], n00[0], n00[1], n00[2], texture_u0, v0,
                ],
                [
                    p10[0], p10[1], p10[2], n10[0], n10[1], n10[2], texture_u1, v0,
                ],
                [
                    p11[0], p11[1], p11[2], n11[0], n11[1], n11[2], texture_u1, v1,
                ],
                [
                    p00[0], p00[1], p00[2], n00[0], n00[1], n00[2], texture_u0, v0,
                ],
                [
                    p11[0], p11[1], p11[2], n11[0], n11[1], n11[2], texture_u1, v1,
                ],
                [
                    p01[0], p01[1], p01[2], n01[0], n01[1], n01[2], texture_u0, v1,
                ],
            ]);
        }
    }

    vertices
}

pub fn displace_vertices(
    vertices: &mut [SphereVertex],
    displacement: &HeightMap,
    material: &MaterialProperties,
) {
    for vertex in vertices {
        let u = vertex[6];
        let v = vertex[7];
        let u = if v == 0.0 || v == 1.0 { 0.0 } else { u };
        let x = u.rem_euclid(1.0) * displacement.width() as f32 - 0.5;
        let y =
            (v * displacement.height() as f32 - 0.5).clamp(0.0, (displacement.height() - 1) as f32);
        let x0 = (x.floor() as i32).rem_euclid(displacement.width() as i32) as u32;
        let x1 = (x0 + 1) % displacement.width();
        let y0 = y.floor() as u32;
        let y1 = (y0 + 1).min(displacement.height() - 1);
        let tx = x - x.floor();
        let ty = y - y.floor();
        let top = displacement.get_pixel(x0, y0)[0] as f32 * (1.0 - tx)
            + displacement.get_pixel(x1, y0)[0] as f32 * tx;
        let bottom = displacement.get_pixel(x0, y1)[0] as f32 * (1.0 - tx)
            + displacement.get_pixel(x1, y1)[0] as f32 * tx;

        let height = (top * (1.0 - ty) + bottom * ty) / u16::MAX as f32;
        let offset = height * material.displacement_scale + material.displacement_bias;
        for axis in 0..3 {
            vertex[axis] += vertex[axis + 3] * offset;
        }
    }
}

pub fn get_best_device(renderer: &ExampleRenderer) -> GraphicsDevice {
    let devices = renderer.enumerate_devices().unwrap();
    let mut device = devices.iter().find(|d| d.device_type == DiscreteGpu);
    if device.is_none() {
        device = devices.iter().find(|d| d.device_type == IntegratedGpu);
    }
    if device.is_none() {
        device = devices.first();
    }
    device.unwrap().clone()
}

pub fn set_best_msaa(renderer: &mut ExampleRenderer) {
    let supported_msaa_list = renderer.supported_msaa_list().unwrap();
    let msaa = if supported_msaa_list.contains(&Msaa::X64) {
        Msaa::X64
    } else if supported_msaa_list.contains(&Msaa::X32) {
        Msaa::X32
    } else if supported_msaa_list.contains(&Msaa::X16) {
        Msaa::X16
    } else if supported_msaa_list.contains(&Msaa::X8) {
        Msaa::X8
    } else if supported_msaa_list.contains(&Msaa::X4) {
        Msaa::X4
    } else if supported_msaa_list.contains(&Msaa::X2) {
        Msaa::X2
    } else {
        Msaa::X1
    };
    let mut settings = *renderer.get_settings();
    settings.msaa = msaa;
    renderer.set_settings(settings).unwrap();
}

fn sphere_position(radius: f32, x_offset: f32, u: f32, v: f32) -> [f32; 3] {
    if v == 0.0 {
        return [x_offset, radius, 0.0];
    }
    if v == 1.0 {
        return [x_offset, -radius, 0.0];
    }
    let theta = v * std::f32::consts::PI;
    let phi = u.rem_euclid(1.0) * std::f32::consts::TAU;
    let sin_theta = theta.sin();
    [
        x_offset + radius * sin_theta * phi.cos(),
        radius * theta.cos(),
        radius * sin_theta * phi.sin(),
    ]
}
