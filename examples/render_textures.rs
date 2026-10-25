use heph_gl::{
    GraphicsDeviceFeature::AnisotropicFiltering,
    Lighting, MaterialProperties, Shader,
    math::Vec3,
    renderers::{
        BufferUsage, FeatureRequest, GraphicsPipelineOptions, Renderer, ResourceBinding,
        ResourceBindingType, SamplerAddressMode, SamplerOptions, TextureFormat, TextureOptions,
    },
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

mod utils;

const IMAGE_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_diff_1k.jpg";
const DISPLACEMENT_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_disp_1k.png";
const ARM_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_arm_1k.exr";
const NORMAL_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_nor_gl_1k.exr";

fn example(
    renderer: &mut utils::renderer::ExampleRenderer,
    _: RawWindowHandle,
    _: RawDisplayHandle,
) {
    let device = utils::renderer::get_best_device(renderer);
    renderer
        .set_device(
            Some(&device),
            &[FeatureRequest::new(AnisotropicFiltering, false)],
        )
        .unwrap();
    utils::renderer::set_best_msaa(renderer);

    let vert_shader = Shader::from_file(format!(
        "{}/{}",
        utils::SHADERS_DIR,
        "broken_brick_wall_vert.spv"
    ))
    .unwrap();
    let frag_shader = Shader::from_file(format!(
        "{}/{}",
        utils::SHADERS_DIR,
        "broken_brick_wall_frag.spv"
    ))
    .unwrap();
    let mut pipeline = renderer
        .create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions::default(),
        )
        .unwrap();
    drop(vert_shader);
    drop(frag_shader);

    let image = image::open(IMAGE_PATH).unwrap().to_rgba8();
    let displacement = image::open(DISPLACEMENT_PATH).unwrap().to_luma16();
    let arm = utils::load_exr(ARM_PATH, ["R", "G", "B"]);
    let normal = utils::load_exr(NORMAL_PATH, ["R", "G", "B"]);
    let material = MaterialProperties {
        displacement_scale: 0.1,
        ..Default::default()
    };
    let lighting = Lighting {
        direction: Vec3::new(0.3, 0.4, 1.0),
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
        .write_buffer(uniform_buffer.as_mut(), 0, bytemuck::cast_slice(&uniforms))
        .unwrap();

    // Displace a unit sphere before scaling it, so all three sizes have the same relative relief.
    let mut sphere = utils::renderer::sphere_vertices(1.0, 0.0);
    utils::renderer::displace_vertices(&mut sphere, &displacement, &material);
    let vertex_count: usize = utils::renderer::sphere_vertex_count();
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
            vertices.len() * std::mem::size_of::<utils::renderer::SphereVertex>(),
            BufferUsage::Vertex,
        )
        .unwrap();
    renderer
        .write_buffer(vertex_buffer.as_mut(), 0, bytemuck::cast_slice(&vertices))
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
                handle: vertex_buffer.as_ref(),
                usage: BufferUsage::Vertex,
                offset: 0,
                size: vertex_buffer.size(),
            },
        },
        ResourceBinding {
            binding: 0,
            resource: ResourceBindingType::Texture {
                handle: texture.as_ref(),
            },
        },
        ResourceBinding {
            binding: 1,
            resource: ResourceBindingType::Sampler {
                handle: sampler.as_ref(),
            },
        },
        ResourceBinding {
            binding: 2,
            resource: ResourceBindingType::Texture {
                handle: arm_texture.as_ref(),
            },
        },
        ResourceBinding {
            binding: 3,
            resource: ResourceBindingType::Texture {
                handle: normal_texture.as_ref(),
            },
        },
        ResourceBinding {
            binding: 4,
            resource: ResourceBindingType::Buffer {
                handle: uniform_buffer.as_ref(),
                usage: BufferUsage::Uniform,
                offset: 0,
                size: uniform_buffer.size(),
            },
        },
    ];
    renderer
        .record_graphics_command(pipeline.as_mut(), &[&bindings], vertex_count as u32, 1)
        .unwrap();
    renderer.end_frame().unwrap();

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_sampler(sampler.as_mut()).unwrap();
    renderer.destroy_texture(texture.as_mut()).unwrap();
    renderer.destroy_texture(arm_texture.as_mut()).unwrap();
    renderer.destroy_texture(normal_texture.as_mut()).unwrap();
    renderer.destroy_buffer(uniform_buffer.as_mut()).unwrap();
    renderer.destroy_buffer(vertex_buffer.as_mut()).unwrap();
    renderer
        .destroy_graphics_pipeline(pipeline.as_mut())
        .unwrap();

    std::thread::sleep(std::time::Duration::from_secs(5));
    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Textures";
    const INIT_RENDERER: bool = true;
    utils::run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
