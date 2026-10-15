use heph_gl::{
    graphics_device::Feature::AnisotropicFiltering,
    light::Lighting,
    material::MaterialProperties,
    math::Vec3,
    renderers::{
        Renderer,
        resources::{BufferUsage, GpuBuffer, ResourceBinding, ResourceBindingType, TextureFormat},
        settings::{
            FeatureRequest, GraphicsPipelineOptions, SamplerAddressMode, SamplerOptions,
            TextureOptions,
        },
    },
    shader::Shader,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

use crate::utils::{ExampleRenderer, SHADERS_DIR, get_best_device, run_example, set_best_msaa};

mod utils;

// [position.x, position.y, position.z, normal.x, normal.y, normal.z, tex_coord.u, tex_coord.v]
type Vertex = [f32; 8];
type HeightMap = image::ImageBuffer<image::Luma<u16>, Vec<u16>>;

const IMAGE_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_diff_1k.jpg";
const DISPLACEMENT_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_disp_1k.png";
const ARM_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_arm_1k.exr";
const NORMAL_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_nor_gl_1k.exr";
const LATITUDE_SEGMENTS: usize = 128;
const LONGITUDE_SEGMENTS: usize = 256;
// The equatorial circumference is twice the pole-to-pole distance, so repeat the square maps twice.
const TEXTURE_REPEATS_U: f32 = 2.0;

// Decode the EXR channels and convert their floating-point values to the renderer's linear 8-bit
// format without sRGB conversion.
fn load_exr(path: &str, channel_names: [&str; 3]) -> image::RgbaImage {
    let layer = exr::prelude::read_first_flat_layer_from_file(path)
        .unwrap()
        .layer_data;
    let mut channels = channel_names.map(|name| {
        layer
            .channel_data
            .list
            .iter()
            .find(|channel| channel.name == *name)
            .unwrap()
            .sample_data
            .values_as_f32()
    });
    image::RgbaImage::from_fn(
        layer.size.width() as u32,
        layer.size.height() as u32,
        |_, _| {
            let [r, g, b] = std::array::from_fn(|i| {
                (channels[i].next().unwrap().clamp(0.0, 1.0) * 255.0).round() as u8
            });
            image::Rgba([r, g, b, 255])
        },
    )
}

// Move each vertex along its original sphere normal before uploading the mesh.
fn displace_vertices(
    vertices: &mut [Vertex],
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

// Calculates the position of a point on a sphere given its radius, x offset, and spherical
// coordinates (u, v).
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

// Calculates the sphere vertices for a given radius and x offset. The sphere is generated using
// latitude and longitude segments, creating a grid of vertices that form the surface of the sphere.
// Each vertex includes position, normal, and texture coordinates.
fn sphere_vertices(radius: f32, x_offset: f32) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(LATITUDE_SEGMENTS * LONGITUDE_SEGMENTS * 6);

    for y in 0..LATITUDE_SEGMENTS {
        let v0 = y as f32 / LATITUDE_SEGMENTS as f32;
        let v1 = (y + 1) as f32 / LATITUDE_SEGMENTS as f32;

        for x in 0..LONGITUDE_SEGMENTS {
            let u0 = x as f32 / LONGITUDE_SEGMENTS as f32;
            let u1 = (x + 1) as f32 / LONGITUDE_SEGMENTS as f32;

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
            let texture_u0 = u0 * TEXTURE_REPEATS_U;
            let texture_u1 = u1 * TEXTURE_REPEATS_U;

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

fn example(renderer: &mut ExampleRenderer, _: RawWindowHandle, _: RawDisplayHandle) {
    let device = get_best_device(renderer);
    renderer
        .set_device(
            Some(&device),
            &[FeatureRequest::new(AnisotropicFiltering, false)],
        )
        .unwrap();
    set_best_msaa(renderer);

    let vert_shader = Shader::from_file(format!(
        "{}/{}",
        SHADERS_DIR, "broken_brick_wall_material_vert.spv"
    ))
    .unwrap();
    let frag_shader = Shader::from_file(format!(
        "{}/{}",
        SHADERS_DIR, "broken_brick_wall_material_frag.spv"
    ))
    .unwrap();
    let pipeline = renderer
        .create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions::default(),
        )
        .unwrap();
    drop(vert_shader);
    drop(frag_shader);

    let image = image::open(IMAGE_PATH).unwrap().to_rgba8();
    let displacement = image::open(DISPLACEMENT_PATH).unwrap().to_luma16();
    let arm = load_exr(ARM_PATH, ["R", "G", "B"]);
    let normal = load_exr(NORMAL_PATH, ["R", "G", "B"]);
    let material = MaterialProperties {
        displacement_scale: 0.15,
        displacement_bias: -0.02,
        ..Default::default()
    };
    let lighting = Lighting {
        direction: Vec3::new(0.3, 0.65, 1.0),
        intensity: 2.0,
        ambient: RGB::new(0.45, 0.15, 0.15),
        ..Default::default()
    };

    let uniforms = [
        [
            material.base_color.r,
            material.base_color.g,
            material.base_color.b,
            material.base_color.a,
        ],
        [
            material.roughness,
            material.metallicness,
            material.normal_scale,
            0.0,
        ],
        [
            lighting.direction.x,
            lighting.direction.y,
            lighting.direction.z,
            0.0,
        ],
        [
            lighting.color.r,
            lighting.color.g,
            lighting.color.b,
            lighting.intensity,
        ],
        [
            lighting.ambient.r,
            lighting.ambient.g,
            lighting.ambient.b,
            0.0,
        ],
        [0.0, 0.0, -1.0, 0.0],
    ];
    let mut uniform_buffer = renderer
        .create_buffer(std::mem::size_of_val(&uniforms), BufferUsage::Uniform)
        .unwrap();
    renderer
        .write_buffer(&uniform_buffer, bytemuck::cast_slice(&uniforms))
        .unwrap();

    // Displace a unit sphere before scaling it, so all three sizes have the same relative relief.
    let mut sphere = sphere_vertices(1.0, 0.0);
    displace_vertices(&mut sphere, &displacement, &material);
    let vertex_count: usize = LATITUDE_SEGMENTS * LONGITUDE_SEGMENTS * 6 * 3;
    let mut vertices = Vec::with_capacity(vertex_count);
    for (radius, x_offset) in [(0.1, -1.2), (0.25, -0.4), (0.5, 0.8)] {
        vertices.extend(sphere.iter().map(|vertex| {
            [
                vertex[0] * radius + x_offset,
                vertex[1] * radius,
                vertex[2] * radius,
                vertex[3],
                vertex[4],
                vertex[5],
                vertex[6],
                vertex[7],
            ]
        }));
    }
    drop(sphere);
    let mut vertex_buffer = renderer
        .create_buffer(
            vertices.len() * std::mem::size_of::<Vertex>(),
            BufferUsage::Vertex,
        )
        .unwrap();
    renderer
        .write_buffer(&vertex_buffer, bytemuck::cast_slice(&vertices))
        .unwrap();
    drop(vertices);

    let mut sampler = renderer
        .create_sampler(&SamplerOptions {
            address_mode_u: SamplerAddressMode::Repeat,
            max_anisotropy: match device.supported_features.contains(&AnisotropicFiltering) {
                true => Some(device.max_sampler_anisotropy),
                false => None,
            },
            ..Default::default()
        })
        .unwrap();

    // Creating a texture creates a GPU command to transfer the raw pixel data from CPU to GPU, thus
    // we must begin a frame first. Once a texture is loaded, we can use it until we destroy it.
    renderer.begin_frame().unwrap();
    renderer.clear(RGB::default()).unwrap();
    let mut texture = renderer
        .create_texture(
            &TextureOptions {
                width: image.width(),
                height: image.height(),
                format: TextureFormat::Rgba8Srgb,
                mip_level_count: TextureOptions::max_mip_level_count(image.width(), image.height()),
            },
            image.as_raw(),
        )
        .unwrap();
    let mut arm_texture = renderer
        .create_texture(
            &TextureOptions {
                width: arm.width(),
                height: arm.height(),
                format: TextureFormat::Rgba8Unorm,
                mip_level_count: TextureOptions::max_mip_level_count(arm.width(), arm.height()),
            },
            arm.as_raw(),
        )
        .unwrap();
    let mut normal_texture = renderer
        .create_texture(
            &TextureOptions {
                width: image.width(),
                height: image.height(),
                format: TextureFormat::Rgba8Unorm,
                mip_level_count: TextureOptions::max_mip_level_count(image.width(), image.height()),
            },
            normal.as_raw(),
        )
        .unwrap();
    renderer.end_frame().unwrap();

    // Render the displaced spheres with the color, ARM, and normal maps applied.
    renderer.begin_frame().unwrap();
    renderer.clear(RGB::default()).unwrap();
    let bindings = [
        ResourceBinding {
            binding: 0,
            resource: ResourceBindingType::Buffer {
                handle: vertex_buffer,
                usage: BufferUsage::Vertex,
                offset: 0,
                size: vertex_buffer.size(),
            },
        },
        ResourceBinding {
            binding: 0,
            resource: ResourceBindingType::Texture { handle: texture },
        },
        ResourceBinding {
            binding: 1,
            resource: ResourceBindingType::Sampler { handle: sampler },
        },
        ResourceBinding {
            binding: 2,
            resource: ResourceBindingType::Texture {
                handle: arm_texture,
            },
        },
        ResourceBinding {
            binding: 3,
            resource: ResourceBindingType::Texture {
                handle: normal_texture,
            },
        },
        ResourceBinding {
            binding: 4,
            resource: ResourceBindingType::Buffer {
                handle: uniform_buffer,
                usage: BufferUsage::Uniform,
                offset: 0,
                size: uniform_buffer.size(),
            },
        },
    ];
    renderer
        .record_graphics_command(&pipeline, &[&bindings], vertex_count as u32, 1)
        .unwrap();
    renderer.end_frame().unwrap();

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_sampler(&mut sampler).unwrap();
    renderer.destroy_texture(&mut texture).unwrap();
    renderer.destroy_texture(&mut arm_texture).unwrap();
    renderer.destroy_texture(&mut normal_texture).unwrap();
    renderer.destroy_buffer(&mut uniform_buffer).unwrap();
    renderer.destroy_buffer(&mut vertex_buffer).unwrap();
    renderer.destroy_graphics_pipeline(&pipeline).unwrap();

    std::thread::sleep(std::time::Duration::from_secs(5));
    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Textures";
    const INIT_RENDERER: bool = true;
    run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
