use std::time::{Duration, Instant};

use heph_gl::{
    camera::{Camera, Projection},
    graphics_device::Feature::AnisotropicFiltering,
    light::Lighting,
    material::MaterialProperties,
    math::{Mat4, Quat, Vec3},
    mesh::{Mesh, MeshVertexAttribute},
    renderers::{
        Renderer,
        concurrency::{RendererHandle, RendererWorkerFactory},
        resources::{BufferUsage, ResourceBinding, ResourceBindingType, TextureFormat},
        settings::{
            ColorBlending, CullingMode, FeatureRequest, FrontFace, GraphicsPipelineOptions,
            PrimitiveTopology, SamplerAddressMode, SamplerOptions, Settings, TextureOptions,
        },
    },
    shader::Shader,
    text::{
        Font, TextVertex,
        options::{FontOptions, TextType},
    },
    transform::Transform,
};
use renkrs::RGB;

use crate::{heph_expect_success, renderer::RendererTests, utils::SHADERS_DIR};

type SphereVertex = [f32; 8];
type HeightMap = image::ImageBuffer<image::Luma<u16>, Vec<u16>>;

const ANIMATION_DURATION: Duration = Duration::from_secs(5);
const BROKEN_BRICK_WALL_IMAGE_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_diff_1k.jpg";
const BROKEN_BRICK_WALL_DISPLACEMENT_PATH: &str =
    "assets/broken_brick_wall/broken_brick_wall_disp_1k.png";
const BROKEN_BRICK_WALL_ARM_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_arm_1k.exr";
const BROKEN_BRICK_WALL_NORMAL_PATH: &str =
    "assets/broken_brick_wall/broken_brick_wall_nor_gl_1k.exr";
const BROKEN_BRICK_WALL_LATITUDE_SEGMENTS: usize = 512;
const BROKEN_BRICK_WALL_LONGITUDE_SEGMENTS: usize = 1024;
const BROKEN_BRICK_WALL_TEXTURE_REPEATS_U: f32 = 2.0;

impl<TestRenderer: Renderer> RendererTests<TestRenderer>
where
    RendererHandle<TestRenderer>: RendererWorkerFactory<TestRenderer>,
{
    pub(crate) fn test_render_static_shapes(&self) {
        let mut renderer = self.create_renderer_with_any_device(&[]);

        {
            let msaa = heph_expect_success!(renderer.supported_msaa_list()).last();
            assert!(msaa.is_some());
            heph_expect_success!(renderer.set_settings(Settings {
                msaa: *heph_expect_success!(
                    msaa.ok_or("Failed to get a supported MSAA sample count.")
                ),
                ..Default::default()
            }));
        }

        let shader_infos = [
            ("dot", "shape", PrimitiveTopology::PointList, 1),
            ("line", "shape", PrimitiveTopology::LineList, 2),
            ("ellipse", "shape", PrimitiveTopology::LineStrip, 257),
            ("pentagon", "shape", PrimitiveTopology::TriangleStrip, 5),
            (
                "basic_triangle",
                "basic_triangle",
                PrimitiveTopology::TriangleList,
                3,
            ),
        ];
        let mut pipelines = Vec::with_capacity(shader_infos.len());
        {
            for (vertex_shader_name, frag_shader_name, topology, draw_count) in shader_infos {
                let vertex_shader = heph_expect_success!(Shader::from_file(format!(
                    "{}/shapes/{}_vert.spv",
                    SHADERS_DIR, vertex_shader_name
                )));
                let frag_shader = heph_expect_success!(Shader::from_file(format!(
                    "{}/shapes/{}_frag.spv",
                    SHADERS_DIR, frag_shader_name
                )));
                pipelines.push((
                    heph_expect_success!(renderer.create_graphics_pipeline(
                        &[&vertex_shader, &frag_shader],
                        &GraphicsPipelineOptions {
                            topology,
                            ..Default::default()
                        },
                    )),
                    draw_count,
                ));
            }
        }

        heph_expect_success!(renderer.begin_frame());
        heph_expect_success!(renderer.clear(RGB::default()));
        for (pipeline, draw_count) in pipelines.iter_mut() {
            heph_expect_success!(renderer.record_graphics_command(
                pipeline.as_mut(),
                &[],
                *draw_count,
                1
            ));
        }
        heph_expect_success!(renderer.end_frame());

        heph_expect_success!(renderer.wait_idle());
        for (ref mut pipeline, _) in pipelines {
            heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
        }
    }

    pub(crate) fn test_render_dynamic_cubes(&self) {
        type Vertex = [f32; 10];
        const POSITIONS: [[f32; 3]; 8] = [
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
        ];
        const INDICES: [u32; 36] = [
            0, 1, 2, 2, 3, 0, 4, 6, 5, 6, 4, 7, 0, 3, 7, 7, 4, 0, 1, 5, 6, 6, 2, 1, 0, 4, 5, 5, 1,
            0, 3, 2, 6, 6, 7, 3,
        ];

        let mut renderer = self.create_renderer_with_any_device(&[]);
        let vert_shader = heph_expect_success!(Shader::from_file(format!(
            "{}/shapes/{}",
            SHADERS_DIR, "cube_vert.spv"
        )));
        let frag_shader = heph_expect_success!(Shader::from_file(format!(
            "{}/shapes/{}",
            SHADERS_DIR, "cube_frag.spv"
        )));
        let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions {
                blending: ColorBlending::Alpha,
                culling: CullingMode::Back,
                front_face: FrontFace::Clockwise,
                ..Default::default()
            }
        ));
        drop(vert_shader);
        drop(frag_shader);

        let vertex_buffer_size = std::mem::size_of::<[Vertex; 8]>();
        let index_buffer_size = std::mem::size_of_val(&INDICES);
        let mut cube_a_buffer =
            heph_expect_success!(renderer.create_buffer(vertex_buffer_size, BufferUsage::Vertex));
        let mut cube_b_buffer =
            heph_expect_success!(renderer.create_buffer(vertex_buffer_size, BufferUsage::Vertex));
        let mut index_buffer =
            heph_expect_success!(renderer.create_buffer(index_buffer_size, BufferUsage::Index));

        heph_expect_success!(renderer.write_buffer(
            index_buffer.as_mut(),
            0,
            bytemuck::cast_slice(&INDICES)
        ));

        let cube_vertices = |offset: [f32; 3], color: [f32; 4]| -> [Vertex; 8] {
            std::array::from_fn(|i| {
                let position = POSITIONS[i];

                [
                    position[0],
                    position[1],
                    position[2],
                    color[0],
                    color[1],
                    color[2],
                    color[3],
                    offset[0],
                    offset[1],
                    offset[2],
                ]
            })
        };

        heph_expect_success!(renderer.begin_frame());
        heph_expect_success!(renderer.clear(RGB::default()));

        let cube_a = cube_vertices([-0.5, -0.125, 0.5], [1.0, 0.0, 0.0, 1.0]);
        let cube_b = cube_vertices([0.5, 0.125, 0.2], [0.0, 0.0, 1.0, 0.5]);
        heph_expect_success!(renderer.write_buffer(
            cube_a_buffer.as_mut(),
            0,
            bytemuck::cast_slice(&cube_a)
        ));
        heph_expect_success!(renderer.write_buffer(
            cube_b_buffer.as_mut(),
            0,
            bytemuck::cast_slice(&cube_b)
        ));

        let cube_a_bindings = [
            ResourceBinding {
                binding: 0,
                resource: ResourceBindingType::Buffer {
                    handle: cube_a_buffer.as_ref(),
                    usage: BufferUsage::Vertex,
                    offset: 0,
                    size: cube_a_buffer.size(),
                },
            },
            ResourceBinding {
                binding: 0,
                resource: ResourceBindingType::Buffer {
                    handle: index_buffer.as_ref(),
                    usage: BufferUsage::Index,
                    offset: 0,
                    size: index_buffer.size(),
                },
            },
        ];
        let cube_b_bindings = [
            ResourceBinding {
                binding: 0,
                resource: ResourceBindingType::Buffer {
                    handle: cube_b_buffer.as_ref(),
                    usage: BufferUsage::Vertex,
                    offset: 0,
                    size: cube_b_buffer.size(),
                },
            },
            ResourceBinding {
                binding: 0,
                resource: ResourceBindingType::Buffer {
                    handle: index_buffer.as_ref(),
                    usage: BufferUsage::Index,
                    offset: 0,
                    size: index_buffer.size(),
                },
            },
        ];
        heph_expect_success!(renderer.record_graphics_command(
            pipeline.as_mut(),
            &[&cube_a_bindings],
            INDICES.len() as u32,
            1
        ));
        heph_expect_success!(renderer.record_graphics_command(
            pipeline.as_mut(),
            &[&cube_b_bindings],
            INDICES.len() as u32,
            1
        ));

        heph_expect_success!(renderer.end_frame());

        heph_expect_success!(renderer.wait_idle());
        heph_expect_success!(renderer.destroy_buffer(cube_a_buffer.as_mut()));
        heph_expect_success!(renderer.destroy_buffer(cube_b_buffer.as_mut()));
        heph_expect_success!(renderer.destroy_buffer(index_buffer.as_mut()));
        heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
    }

    pub(crate) fn test_render_textures(&self) {
        let mut renderer = self
            .create_renderer_with_any_device(&[FeatureRequest::new(AnisotropicFiltering, false)]);
        let max_anisotropy = if let Some(device) = renderer.get_device() {
            match device.supported_features.contains(&AnisotropicFiltering) {
                true => Some(device.max_sampler_anisotropy),
                false => None,
            }
        } else {
            None
        };

        let vert_shader = heph_expect_success!(Shader::from_file(format!(
            "{}/{}",
            SHADERS_DIR, "broken_brick_wall_vert.spv"
        )));
        let frag_shader = heph_expect_success!(Shader::from_file(format!(
            "{}/{}",
            SHADERS_DIR, "broken_brick_wall_frag.spv"
        )));
        let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions::default(),
        ));
        drop(vert_shader);
        drop(frag_shader);

        let image = heph_expect_success!(image::open(BROKEN_BRICK_WALL_IMAGE_PATH)).to_rgba8();
        let displacement =
            heph_expect_success!(image::open(BROKEN_BRICK_WALL_DISPLACEMENT_PATH)).to_luma16();
        let arm = self.load_exr(BROKEN_BRICK_WALL_ARM_PATH, ["R", "G", "B"]);
        let normal = self.load_exr(BROKEN_BRICK_WALL_NORMAL_PATH, ["R", "G", "B"]);
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
        let mut uniform_buffer = heph_expect_success!(
            renderer.create_buffer(std::mem::size_of_val(&uniforms), BufferUsage::Uniform)
        );
        heph_expect_success!(renderer.write_buffer(
            uniform_buffer.as_mut(),
            0,
            bytemuck::cast_slice(&uniforms)
        ));

        let mut sphere = self.sphere_vertices(1.0, 0.0);
        self.displace_vertices(&mut sphere, &displacement, &material);
        let vertex_count: usize =
            BROKEN_BRICK_WALL_LATITUDE_SEGMENTS * BROKEN_BRICK_WALL_LONGITUDE_SEGMENTS * 6 * 3;
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
        let mut vertex_buffer = heph_expect_success!(renderer.create_buffer(
            vertices.len() * std::mem::size_of::<SphereVertex>(),
            BufferUsage::Vertex,
        ));
        heph_expect_success!(renderer.write_buffer(
            vertex_buffer.as_mut(),
            0,
            bytemuck::cast_slice(&vertices)
        ));
        drop(vertices);

        let mut sampler = heph_expect_success!(renderer.create_sampler(&SamplerOptions {
            address_mode_u: SamplerAddressMode::Repeat,
            max_anisotropy,
            ..Default::default()
        }));

        heph_expect_success!(renderer.begin_frame());
        heph_expect_success!(renderer.clear(RGB::default()));
        let mut texture = heph_expect_success!(renderer.create_texture(
            &TextureOptions {
                width: image.width(),
                height: image.height(),
                format: TextureFormat::Rgba8Srgb,
                mip_level_count: TextureOptions::max_mip_level_count(image.width(), image.height()),
            },
            image.as_raw(),
        ));
        let mut arm_texture = heph_expect_success!(renderer.create_texture(
            &TextureOptions {
                width: arm.width(),
                height: arm.height(),
                format: TextureFormat::Rgba8Unorm,
                mip_level_count: TextureOptions::max_mip_level_count(arm.width(), arm.height()),
            },
            arm.as_raw(),
        ));
        let mut normal_texture = heph_expect_success!(renderer.create_texture(
            &TextureOptions {
                width: image.width(),
                height: image.height(),
                format: TextureFormat::Rgba8Unorm,
                mip_level_count: TextureOptions::max_mip_level_count(image.width(), image.height()),
            },
            normal.as_raw(),
        ));
        heph_expect_success!(renderer.end_frame());

        heph_expect_success!(renderer.begin_frame());
        heph_expect_success!(renderer.clear(RGB::default()));
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
        heph_expect_success!(renderer.record_graphics_command(
            pipeline.as_mut(),
            &[&bindings],
            vertex_count as u32,
            1,
        ));
        heph_expect_success!(renderer.end_frame());

        heph_expect_success!(renderer.wait_idle());
        heph_expect_success!(renderer.destroy_sampler(sampler.as_mut()));
        heph_expect_success!(renderer.destroy_texture(texture.as_mut()));
        heph_expect_success!(renderer.destroy_texture(arm_texture.as_mut()));
        heph_expect_success!(renderer.destroy_texture(normal_texture.as_mut()));
        heph_expect_success!(renderer.destroy_buffer(uniform_buffer.as_mut()));
        heph_expect_success!(renderer.destroy_buffer(vertex_buffer.as_mut()));
        heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
    }

    pub(crate) fn test_render_text(&self) {
        const FONT_PATH: &str = "assets/liberation_sans/LiberationSans-Regular.ttf";
        const FONT_SIZE: f32 = 72.0;

        let create_pipeline = |renderer: &mut TestRenderer,
                               vertex_shader: &str,
                               fragment_shader: &str,
                               options: &GraphicsPipelineOptions| {
            let vert_shader = heph_expect_success!(Shader::from_file(format!(
                "{}/{}",
                SHADERS_DIR, vertex_shader
            )));
            let frag_shader = heph_expect_success!(Shader::from_file(format!(
                "{}/{}",
                SHADERS_DIR, fragment_shader
            )));
            heph_expect_success!(
                renderer.create_graphics_pipeline(&[&vert_shader, &frag_shader], options)
            )
        };
        let create_vertex_buffer = |renderer: &mut TestRenderer, vertices: &[TextVertex]| {
            let mut buffer = heph_expect_success!(
                renderer.create_buffer(std::mem::size_of_val(vertices), BufferUsage::Vertex)
            );
            heph_expect_success!(renderer.write_buffer(
                buffer.as_mut(),
                0,
                bytemuck::cast_slice(vertices)
            ));
            buffer
        };
        let center_vertices = |vertices: &mut [TextVertex]| {
            let mut min = Vec3::from_slice(&vertices[0][..3]);
            let mut max = min;
            for vertex in vertices.iter() {
                let position = Vec3::from_slice(&vertex[..3]);
                min = min.min(position);
                max = max.max(position);
            }

            let center = (min + max) * 0.5;
            for vertex in vertices {
                vertex[0] -= center.x;
                vertex[1] -= center.y;
                vertex[2] -= center.z;
            }
        };

        let mut renderer = self.create_renderer_with_any_device(&[]);
        let Some(&msaa) = heph_expect_success!(renderer.supported_msaa_list()).last() else {
            panic!("Failed to get a supported MSAA sample count.");
        };
        let settings = Settings {
            frames_in_flight: 3,
            vsync: true,
            msaa,
            ..Default::default()
        };
        heph_expect_success!(renderer.set_settings(settings));

        let mut screen_pipeline = create_pipeline(
            &mut renderer,
            "text_vert.spv",
            "text_frag.spv",
            &GraphicsPipelineOptions {
                blending: ColorBlending::Alpha,
                depth_test: false,
                depth_write: false,
                ..Default::default()
            },
        );
        let mut world_pipeline = create_pipeline(
            &mut renderer,
            "text_world_vert.spv",
            "text_frag.spv",
            &GraphicsPipelineOptions {
                blending: ColorBlending::Alpha,
                ..Default::default()
            },
        );
        let mut extruded_pipeline = create_pipeline(
            &mut renderer,
            "text_world_vert.spv",
            "text_extruded_frag.spv",
            &GraphicsPipelineOptions::default(),
        );

        let mut font = heph_expect_success!(Font::from_file(
            FONT_PATH,
            &FontOptions {
                size: FONT_SIZE,
                enable_screen_text: true,
                enable_world_text: true,
                enable_extruded_text: true,
                ..Default::default()
            }
        ));
        let screen_vertices = heph_expect_success!(font.vertices(
            "Screen Text",
            &Vec3::new(460.0, 36.0, 0.0),
            [1.0, 1.0, 1.0, 1.0],
            TextType::Screen {
                viewport_size: [1280.0, 720.0],
            },
        ));
        let mut world_vertices = heph_expect_success!(font.vertices(
            "World Text",
            &Vec3::ZERO,
            [0.3, 0.8, 1.0, 1.0],
            TextType::World { scale: 0.018 },
        ));
        let mut extruded_vertices = heph_expect_success!(font.vertices(
            "Extruded Text",
            &Vec3::ZERO,
            [1.0, 0.65, 0.2, 1.0],
            TextType::Extruded {
                scale: 0.018,
                depth: 0.35,
                curve_tolerance_divider: 512.0,
            },
        ));

        center_vertices(&mut world_vertices);
        center_vertices(&mut extruded_vertices);
        let mut screen_vertex_buffer = create_vertex_buffer(&mut renderer, &screen_vertices);
        let mut world_vertex_buffer = create_vertex_buffer(&mut renderer, &world_vertices);
        let mut extruded_vertex_buffer = create_vertex_buffer(&mut renderer, &extruded_vertices);

        let mut uniform_buffers = Vec::with_capacity(settings.frames_in_flight);
        for _ in 0..settings.frames_in_flight {
            uniform_buffers.push([
                heph_expect_success!(
                    renderer
                        .create_buffer(std::mem::size_of::<[[f32; 16]; 3]>(), BufferUsage::Uniform)
                ),
                heph_expect_success!(
                    renderer
                        .create_buffer(std::mem::size_of::<[[f32; 16]; 3]>(), BufferUsage::Uniform)
                ),
            ]);
        }

        let mut sampler = heph_expect_success!(renderer.create_sampler(&SamplerOptions::default()));

        heph_expect_success!(renderer.begin_frame());
        heph_expect_success!(renderer.clear(RGB::default()));
        let mut texture = heph_expect_success!(renderer.create_texture(
            &TextureOptions {
                width: font.atlas_width(),
                height: font.atlas_height(),
                format: TextureFormat::R8Unorm,
                mip_level_count: 1,
            },
            font.atlas_data(),
        ));
        heph_expect_success!(renderer.end_frame());

        let camera = Camera::new(Projection::Perspective {
            vertical_fov: std::f32::consts::FRAC_PI_3,
            aspect_ratio: 16.0 / 9.0,
            near: 0.1,
            far: 100.0,
        });
        let mut transforms = [
            Transform::new(Vec3::new(0.0, 1.3, 8.0), Quat::IDENTITY, Vec3::ONE),
            Transform::new(Vec3::new(0.0, -1.3, 8.0), Quat::IDENTITY, Vec3::ONE),
        ];

        let mut frame_index = 0;
        let start = Instant::now();
        while start.elapsed() < ANIMATION_DURATION {
            heph_expect_success!(renderer.begin_frame());
            heph_expect_success!(renderer.clear(RGB::default()));

            let progress =
                (start.elapsed().as_secs_f32() / ANIMATION_DURATION.as_secs_f32()).min(1.0);
            let angle = progress * std::f32::consts::TAU;
            for (index, transform) in transforms.iter_mut().enumerate() {
                transform.rotation = Quat::from_rotation_y(angle);
                let matrices = [
                    heph_expect_success!(transform.matrix()).to_cols_array(),
                    heph_expect_success!(camera.view_projection_matrix()).to_cols_array(),
                    Mat4::from_mat3(heph_expect_success!(transform.normal_matrix()))
                        .to_cols_array(),
                ];
                heph_expect_success!(renderer.write_buffer(
                    uniform_buffers[frame_index][index].as_mut(),
                    0,
                    bytemuck::cast_slice(&matrices),
                ));
            }

            let screen_bindings = [
                ResourceBinding {
                    binding: 0,
                    resource: ResourceBindingType::Buffer {
                        handle: screen_vertex_buffer.as_ref(),
                        usage: BufferUsage::Vertex,
                        offset: 0,
                        size: screen_vertex_buffer.size(),
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
            ];
            let world_bindings = [
                ResourceBinding {
                    binding: 0,
                    resource: ResourceBindingType::Buffer {
                        handle: world_vertex_buffer.as_ref(),
                        usage: BufferUsage::Vertex,
                        offset: 0,
                        size: world_vertex_buffer.size(),
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
                    resource: ResourceBindingType::Buffer {
                        handle: uniform_buffers[frame_index][0].as_ref(),
                        usage: BufferUsage::Uniform,
                        offset: 0,
                        size: uniform_buffers[frame_index][0].size(),
                    },
                },
            ];
            let extruded_bindings = [
                ResourceBinding {
                    binding: 0,
                    resource: ResourceBindingType::Buffer {
                        handle: extruded_vertex_buffer.as_ref(),
                        usage: BufferUsage::Vertex,
                        offset: 0,
                        size: extruded_vertex_buffer.size(),
                    },
                },
                ResourceBinding {
                    binding: 2,
                    resource: ResourceBindingType::Buffer {
                        handle: uniform_buffers[frame_index][1].as_ref(),
                        usage: BufferUsage::Uniform,
                        offset: 0,
                        size: uniform_buffers[frame_index][1].size(),
                    },
                },
            ];

            heph_expect_success!(renderer.record_graphics_command(
                screen_pipeline.as_mut(),
                &[&screen_bindings],
                screen_vertices.len() as u32,
                1,
            ));
            heph_expect_success!(renderer.record_graphics_command(
                world_pipeline.as_mut(),
                &[&world_bindings],
                world_vertices.len() as u32,
                1,
            ));
            heph_expect_success!(renderer.record_graphics_command(
                extruded_pipeline.as_mut(),
                &[&extruded_bindings],
                extruded_vertices.len() as u32,
                1,
            ));

            heph_expect_success!(renderer.end_frame());
            frame_index = (frame_index + 1) % uniform_buffers.len();
        }

        heph_expect_success!(renderer.wait_idle());
        heph_expect_success!(renderer.destroy_sampler(sampler.as_mut()));
        heph_expect_success!(renderer.destroy_texture(texture.as_mut()));
        heph_expect_success!(renderer.destroy_buffer(screen_vertex_buffer.as_mut()));
        heph_expect_success!(renderer.destroy_buffer(world_vertex_buffer.as_mut()));
        heph_expect_success!(renderer.destroy_buffer(extruded_vertex_buffer.as_mut()));
        for frame_buffers in &mut uniform_buffers {
            for buffer in frame_buffers {
                heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
            }
        }
        heph_expect_success!(renderer.destroy_graphics_pipeline(screen_pipeline.as_mut()));
        heph_expect_success!(renderer.destroy_graphics_pipeline(world_pipeline.as_mut()));
        heph_expect_success!(renderer.destroy_graphics_pipeline(extruded_pipeline.as_mut()));
    }

    pub(crate) fn test_render_meshes(&self) {
        use std::{collections::HashMap, path::Path};

        const MESH_PATH: &str = "assets/ship/ship_pinnace_4k.gltf";
        const ANIMATION_DURATION: Duration = Duration::from_secs(5);

        let mut renderer = self
            .create_renderer_with_any_device(&[FeatureRequest::new(AnisotropicFiltering, false)]);
        let max_anisotropy = if let Some(device) = renderer.get_device() {
            match device.supported_features.contains(&AnisotropicFiltering) {
                true => Some(device.max_sampler_anisotropy),
                false => None,
            }
        } else {
            None
        };
        let Some(&msaa) = heph_expect_success!(renderer.supported_msaa_list()).last() else {
            panic!("Failed to get a supported MSAA sample count.");
        };
        let settings = Settings {
            frames_in_flight: 3,
            vsync: true,
            msaa,
            ..Default::default()
        };
        heph_expect_success!(renderer.set_settings(settings));

        let vert_shader = heph_expect_success!(Shader::from_file(format!(
            "{}/{}",
            SHADERS_DIR, "camera_vert.spv"
        )));
        let frag_shader = heph_expect_success!(Shader::from_file(format!(
            "{}/{}",
            SHADERS_DIR, "mesh_frag.spv"
        )));
        let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions {
                front_face: FrontFace::Clockwise,
                ..Default::default()
            },
        ));
        drop(vert_shader);
        drop(frag_shader);

        let mesh = heph_expect_success!(Mesh::from_file(MESH_PATH));
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for part in mesh.parts() {
            let positions = heph_expect_success!(part.vertices(&[MeshVertexAttribute::Position]));
            for position in positions.as_chunks::<3>().0 {
                let position = Vec3::from_slice(position);
                min = min.min(position);
                max = max.max(position);
            }
        }
        let center = (min + max) * 0.5;
        let mut transform = Transform::new(
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::splat(3.0 / (max - min).max_element()),
        );

        let lighting = Lighting {
            direction: Vec3::new(0.5, -0.8, 1.0),
            intensity: 3.0,
            ambient: RGB::new(0.15, 0.15, 0.15),
            ..Default::default()
        };
        let mut camera = Camera::new(Projection::Perspective {
            vertical_fov: std::f32::consts::FRAC_PI_3,
            aspect_ratio: 16.0 / 9.0,
            near: 0.1,
            far: 100.0,
        });
        camera.position = Vec3::new(1.6, 1.0, -3.0);
        heph_expect_success!(camera.look_at(&Vec3::ZERO, &Vec3::Y));

        let mesh_directory = heph_expect_success!(
            Path::new(MESH_PATH)
                .parent()
                .ok_or("Missing mesh directory.")
        );
        let mut textures = HashMap::new();
        let mut part_buffers = Vec::with_capacity(mesh.part_count());
        heph_expect_success!(renderer.begin_frame());
        heph_expect_success!(renderer.clear(RGB::default()));
        for part in mesh.parts() {
            let vertex_attributes = [
                MeshVertexAttribute::Position,
                MeshVertexAttribute::Normal,
                MeshVertexAttribute::TextureCoordinates(heph_expect_success!(
                    part.base_texture_uv_set()
                        .ok_or("Missing base-color texture UV set.")
                )),
            ];
            let mut vertices = heph_expect_success!(part.vertices(&vertex_attributes));
            let component_count = part.vertex_component_count(&vertex_attributes);
            for vertex in vertices.chunks_exact_mut(component_count) {
                vertex[0] -= center.x;
                vertex[1] -= center.y;
                vertex[2] -= center.z;
            }
            let vertex_count = vertices.len() / component_count;
            let mut vertex_buffer = heph_expect_success!(renderer.create_buffer(
                vertex_count * part.vertex_size_bytes(&vertex_attributes),
                BufferUsage::Vertex,
            ));
            heph_expect_success!(renderer.write_buffer(
                vertex_buffer.as_mut(),
                0,
                bytemuck::cast_slice(&vertices)
            ));

            let indices = part.indices();
            let mut index_buffer = heph_expect_success!(
                renderer.create_buffer(std::mem::size_of_val(indices), BufferUsage::Index,)
            );
            heph_expect_success!(renderer.write_buffer(
                index_buffer.as_mut(),
                0,
                bytemuck::cast_slice(indices)
            ));

            let part_textures = [
                (
                    heph_expect_success!(
                        part.base_texture_image_uri()
                            .ok_or("Missing base-color texture URI.")
                    ),
                    TextureFormat::Rgba8Srgb,
                ),
                (
                    heph_expect_success!(
                        part.arm_texture_image_uri()
                            .ok_or("Missing metallic-roughness texture URI.")
                    ),
                    TextureFormat::Rgba8Unorm,
                ),
                (
                    heph_expect_success!(
                        part.normal_texture_image_uri()
                            .ok_or("Missing normal texture URI.")
                    ),
                    TextureFormat::Rgba8Unorm,
                ),
            ]
            .map(|(uri, format)| {
                textures.entry(uri.to_owned()).or_insert_with(|| {
                    let image =
                        heph_expect_success!(image::open(mesh_directory.join(uri))).to_rgba8();
                    heph_expect_success!(renderer.create_texture(
                        &TextureOptions {
                            width: image.width(),
                            height: image.height(),
                            format,
                            mip_level_count: TextureOptions::max_mip_level_count(
                                image.width(),
                                image.height()
                            ),
                        },
                        image.as_raw(),
                    ))
                });
                uri.to_owned()
            });
            let material = part.material();
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
                [camera.position.x, camera.position.y, camera.position.z, 1.0],
            ];
            let mut material_buffer = heph_expect_success!(
                renderer.create_buffer(std::mem::size_of_val(&uniforms), BufferUsage::Uniform)
            );
            heph_expect_success!(renderer.write_buffer(
                material_buffer.as_mut(),
                0,
                bytemuck::cast_slice(&uniforms)
            ));

            part_buffers.push((
                vertex_buffer,
                index_buffer,
                indices.len() as u32,
                part_textures,
                material_buffer,
            ));
        }

        let part_textures: Vec<_> = part_buffers
            .iter()
            .map(|(_, _, _, uris, _)| uris.each_ref().map(|uri| textures[uri].as_ref()))
            .collect();

        heph_expect_success!(renderer.end_frame());

        let mut uniform_buffers = Vec::with_capacity(settings.frames_in_flight);
        for _ in 0..settings.frames_in_flight {
            uniform_buffers.push(heph_expect_success!(
                renderer.create_buffer(std::mem::size_of::<[[f32; 16]; 3]>(), BufferUsage::Uniform)
            ));
        }
        let mut sampler = heph_expect_success!(renderer.create_sampler(&SamplerOptions {
            address_mode_u: SamplerAddressMode::Repeat,
            address_mode_v: SamplerAddressMode::Repeat,
            max_anisotropy,
            ..Default::default()
        }));

        let mut frame_index = 0;
        let start = Instant::now();
        loop {
            heph_expect_success!(renderer.begin_frame());
            heph_expect_success!(renderer.clear(RGB::default()));

            let progress =
                (start.elapsed().as_secs_f32() / ANIMATION_DURATION.as_secs_f32()).min(1.0);
            transform.rotation = Quat::from_rotation_y(progress * std::f32::consts::TAU);
            let matrices = [
                heph_expect_success!(transform.matrix()).to_cols_array(),
                heph_expect_success!(camera.view_projection_matrix()).to_cols_array(),
                Mat4::from_mat3(heph_expect_success!(transform.normal_matrix())).to_cols_array(),
            ];

            let uniform_buffer = &mut uniform_buffers[frame_index];
            heph_expect_success!(renderer.write_buffer(
                uniform_buffer.as_mut(),
                0,
                bytemuck::cast_slice(&matrices)
            ));

            for (
                (vertex_buffer, index_buffer, index_count, _, material_buffer),
                [texture, arm_texture, normal_texture],
            ) in part_buffers.iter().zip(&part_textures)
            {
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
                        resource: ResourceBindingType::Buffer {
                            handle: index_buffer.as_ref(),
                            usage: BufferUsage::Index,
                            offset: 0,
                            size: index_buffer.size(),
                        },
                    },
                    ResourceBinding {
                        binding: 0,
                        resource: ResourceBindingType::Texture { handle: *texture },
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
                            handle: *arm_texture,
                        },
                    },
                    ResourceBinding {
                        binding: 3,
                        resource: ResourceBindingType::Texture {
                            handle: *normal_texture,
                        },
                    },
                    ResourceBinding {
                        binding: 4,
                        resource: ResourceBindingType::Buffer {
                            handle: material_buffer.as_ref(),
                            usage: BufferUsage::Uniform,
                            offset: 0,
                            size: material_buffer.size(),
                        },
                    },
                    ResourceBinding {
                        binding: 5,
                        resource: ResourceBindingType::Buffer {
                            handle: uniform_buffer.as_ref(),
                            usage: BufferUsage::Uniform,
                            offset: 0,
                            size: uniform_buffer.size(),
                        },
                    },
                ];
                heph_expect_success!(renderer.record_graphics_command(
                    pipeline.as_mut(),
                    &[&bindings],
                    *index_count,
                    1
                ));
            }
            heph_expect_success!(renderer.end_frame());

            frame_index = (frame_index + 1) % uniform_buffers.len();
            if progress == 1.0 {
                break;
            }
        }

        heph_expect_success!(renderer.wait_idle());
        heph_expect_success!(renderer.destroy_sampler(sampler.as_mut()));
        for texture in textures.values_mut() {
            heph_expect_success!(renderer.destroy_texture(texture.as_mut()));
        }

        for (vertex_buffer, index_buffer, _, _, material_buffer) in &mut part_buffers {
            heph_expect_success!(renderer.destroy_buffer(material_buffer.as_mut()));
            heph_expect_success!(renderer.destroy_buffer(vertex_buffer.as_mut()));
            heph_expect_success!(renderer.destroy_buffer(index_buffer.as_mut()));
        }
        for uniform_buffer in &mut uniform_buffers {
            heph_expect_success!(renderer.destroy_buffer(uniform_buffer.as_mut()));
        }
        heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
    }

    pub(crate) fn test_render_animated_meshes(&self) {
        const MESH_PATH: &str = "assets/dragon/model.glb";
        const ANIMATION_DURATION: Duration = Duration::from_secs(5);

        let mut renderer = self
            .create_renderer_with_any_device(&[FeatureRequest::new(AnisotropicFiltering, false)]);
        let max_anisotropy = if let Some(device) = renderer.get_device() {
            match device.supported_features.contains(&AnisotropicFiltering) {
                true => Some(device.max_sampler_anisotropy),
                false => None,
            }
        } else {
            None
        };
        let Some(&msaa) = heph_expect_success!(renderer.supported_msaa_list()).last() else {
            panic!("Failed to get a supported MSAA sample count.");
        };
        let settings = Settings {
            frames_in_flight: 2,
            vsync: true,
            msaa,
            ..Default::default()
        };
        heph_expect_success!(renderer.set_settings(settings));

        let vert_shader = heph_expect_success!(Shader::from_file(format!(
            "{}/{}",
            SHADERS_DIR, "animation_vert.spv"
        )));
        let frag_shader = heph_expect_success!(Shader::from_file(format!(
            "{}/{}",
            SHADERS_DIR, "mesh_frag.spv"
        )));
        let mut pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions {
                front_face: FrontFace::Clockwise,
                ..Default::default()
            },
        ));
        drop(vert_shader);
        drop(frag_shader);

        let mesh = heph_expect_success!(Mesh::from_file(MESH_PATH));

        let part = heph_expect_success!(mesh.parts().next().ok_or("Missing mesh part."));
        let skeleton = heph_expect_success!(mesh.skeleton().ok_or("Missing skeleton."));
        let default_transforms = skeleton.transforms();
        let mut transforms = default_transforms.clone();
        let joint_matrices = heph_expect_success!(
            part.joint_matrices(&heph_expect_success!(skeleton.matrices(&transforms)))
        );
        let vertex_attributes = [
            MeshVertexAttribute::Position,
            MeshVertexAttribute::Normal,
            MeshVertexAttribute::TextureCoordinates(heph_expect_success!(
                part.base_texture_uv_set()
                    .ok_or("Missing base-color texture UV set.")
            )),
            MeshVertexAttribute::JointIndices(0),
            MeshVertexAttribute::JointWeights(0),
        ];
        let vertices = heph_expect_success!(part.vertices(&vertex_attributes));
        let component_count = part.vertex_component_count(&vertex_attributes);

        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in vertices.chunks_exact(component_count) {
            let position = Vec3::from_slice(vertex);
            let mut skinned_position = Vec3::ZERO;
            for influence in 0..4 {
                skinned_position += joint_matrices[vertex[8 + influence] as usize]
                    .transform_point3(position)
                    * vertex[12 + influence];
            }
            min = min.min(skinned_position);
            max = max.max(skinned_position);
        }
        let model = Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0))
            * Mat4::from_scale(Vec3::splat(5.0 / (max - min).max_element()))
            * Mat4::from_translation(-(min + max) * 0.5);

        let lighting = Lighting {
            direction: Vec3::new(0.5, -0.8, 1.0),
            intensity: 3.0,
            ambient: RGB::new(0.15, 0.15, 0.15),
            ..Default::default()
        };
        let mut camera = Camera::new(Projection::Perspective {
            vertical_fov: std::f32::consts::FRAC_PI_3,
            aspect_ratio: 16.0 / 9.0,
            near: 0.1,
            far: 100.0,
        });
        camera.position = Vec3::new(1.6, 1.0, -3.0);
        heph_expect_success!(camera.look_at(&Vec3::ZERO, &Vec3::Y));

        heph_expect_success!(renderer.begin_frame());
        heph_expect_success!(renderer.clear(RGB::default()));
        let vertex_count = vertices.len() / component_count;
        let mut vertex_buffer = heph_expect_success!(renderer.create_buffer(
            vertex_count * part.vertex_size_bytes(&vertex_attributes),
            BufferUsage::Vertex,
        ));
        heph_expect_success!(renderer.write_buffer(
            vertex_buffer.as_mut(),
            0,
            bytemuck::cast_slice(&vertices)
        ));

        let indices = part.indices();
        let mut index_buffer = heph_expect_success!(
            renderer.create_buffer(std::mem::size_of_val(indices), BufferUsage::Index)
        );
        heph_expect_success!(renderer.write_buffer(
            index_buffer.as_mut(),
            0,
            bytemuck::cast_slice(indices)
        ));

        let image = heph_expect_success!(image::load_from_memory(heph_expect_success!(
            part.base_texture_image_data()
                .ok_or("Missing embedded base-color texture.")
        )))
        .to_rgba8();
        let (min_filter, mag_filter, mipmap_filter) = heph_expect_success!(
            part.base_texture_sampler_filters()
                .ok_or("Missing base-color sampler filters.")
        );
        let (address_mode_u, address_mode_v) = heph_expect_success!(
            part.base_texture_sampler_address_modes()
                .ok_or("Missing base-color sampler addressing modes.")
        );

        let mut texture = heph_expect_success!(renderer.create_texture(
            &TextureOptions {
                width: image.width(),
                height: image.height(),
                format: TextureFormat::Rgba8Srgb,
                mip_level_count: match mipmap_filter.is_some() {
                    true => TextureOptions::max_mip_level_count(image.width(), image.height()),
                    false => 1,
                },
            },
            image.as_raw(),
        ));
        let mut arm_texture = heph_expect_success!(renderer.create_texture(
            &TextureOptions {
                width: 1,
                height: 1,
                format: TextureFormat::Rgba8Unorm,
                mip_level_count: 1,
            },
            &[255, 255, 255, 255],
        ));
        let mut normal_texture = heph_expect_success!(renderer.create_texture(
            &TextureOptions {
                width: 1,
                height: 1,
                format: TextureFormat::Rgba8Unorm,
                mip_level_count: 1,
            },
            &[128, 128, 255, 255],
        ));

        let material = part.material();
        let uniforms = [
            [
                material.base_color.r,
                material.base_color.g,
                material.base_color.b,
                material.base_color.a,
            ],
            [material.roughness, material.metallicness, 0.0, 0.0],
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
            [camera.position.x, camera.position.y, camera.position.z, 1.0],
        ];
        let mut material_buffer = heph_expect_success!(
            renderer.create_buffer(std::mem::size_of_val(&uniforms), BufferUsage::Uniform)
        );
        heph_expect_success!(renderer.write_buffer(
            material_buffer.as_mut(),
            0,
            bytemuck::cast_slice(&uniforms)
        ));

        heph_expect_success!(renderer.end_frame());

        let matrices = [
            model.to_cols_array(),
            heph_expect_success!(camera.view_projection_matrix()).to_cols_array(),
            model.inverse().transpose().to_cols_array(),
        ];
        let mut uniform_buffer = heph_expect_success!(
            renderer.create_buffer(std::mem::size_of_val(&matrices), BufferUsage::Uniform)
        );
        heph_expect_success!(renderer.write_buffer(
            uniform_buffer.as_mut(),
            0,
            bytemuck::cast_slice(&matrices)
        ));

        let mut joint_buffers = Vec::with_capacity(settings.frames_in_flight);
        for _ in 0..settings.frames_in_flight {
            joint_buffers.push(heph_expect_success!(renderer.create_buffer(
                joint_matrices.len() * std::mem::size_of::<[f32; 16]>(),
                BufferUsage::Storage,
            )));
        }
        let mut sampler = heph_expect_success!(renderer.create_sampler(&SamplerOptions {
            min_filter,
            mag_filter,
            mipmap_filter: mipmap_filter.unwrap_or_default(),
            address_mode_u,
            address_mode_v,
            max_anisotropy,
            ..Default::default()
        }));

        let mut frame_index = 0;
        let start = Instant::now();
        loop {
            heph_expect_success!(renderer.begin_frame());
            heph_expect_success!(renderer.clear(RGB::default()));

            let elapsed = start.elapsed().as_secs_f32();
            let clip_index = (elapsed / ANIMATION_DURATION.as_secs_f32()) as usize;
            if clip_index >= mesh.animations().len() {
                heph_expect_success!(renderer.end_frame());
                break;
            }

            let animation = &mesh.animations()[clip_index];
            let clip_time = elapsed % ANIMATION_DURATION.as_secs_f32();
            let time = match animation.duration() > 0.0 {
                true => clip_time % animation.duration(),
                false => 0.0,
            };

            transforms.clone_from_slice(&default_transforms);
            heph_expect_success!(animation.sample(time, &mut transforms));

            let matrices = heph_expect_success!(skeleton.matrices(&transforms));
            let joints: Vec<[f32; 16]> = heph_expect_success!(part.joint_matrices(&matrices))
                .iter()
                .map(Mat4::to_cols_array)
                .collect();
            let joint_buffer = &mut joint_buffers[frame_index];
            heph_expect_success!(renderer.write_buffer(
                joint_buffer.as_mut(),
                0,
                bytemuck::cast_slice(&joints)
            ));

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
                    resource: ResourceBindingType::Buffer {
                        handle: index_buffer.as_ref(),
                        usage: BufferUsage::Index,
                        offset: 0,
                        size: index_buffer.size(),
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
                        handle: material_buffer.as_ref(),
                        usage: BufferUsage::Uniform,
                        offset: 0,
                        size: material_buffer.size(),
                    },
                },
                ResourceBinding {
                    binding: 5,
                    resource: ResourceBindingType::Buffer {
                        handle: uniform_buffer.as_ref(),
                        usage: BufferUsage::Uniform,
                        offset: 0,
                        size: uniform_buffer.size(),
                    },
                },
                ResourceBinding {
                    binding: 6,
                    resource: ResourceBindingType::Buffer {
                        handle: joint_buffer.as_ref(),
                        usage: BufferUsage::Storage,
                        offset: 0,
                        size: joint_buffer.size(),
                    },
                },
            ];
            heph_expect_success!(renderer.record_graphics_command(
                pipeline.as_mut(),
                &[&bindings],
                indices.len() as u32,
                1
            ));

            heph_expect_success!(renderer.end_frame());

            frame_index = (frame_index + 1) % joint_buffers.len();
        }

        heph_expect_success!(renderer.wait_idle());
        heph_expect_success!(renderer.destroy_sampler(sampler.as_mut()));
        heph_expect_success!(renderer.destroy_texture(texture.as_mut()));
        heph_expect_success!(renderer.destroy_texture(arm_texture.as_mut()));
        heph_expect_success!(renderer.destroy_texture(normal_texture.as_mut()));
        heph_expect_success!(renderer.destroy_buffer(material_buffer.as_mut()));
        heph_expect_success!(renderer.destroy_buffer(vertex_buffer.as_mut()));
        heph_expect_success!(renderer.destroy_buffer(index_buffer.as_mut()));
        heph_expect_success!(renderer.destroy_buffer(uniform_buffer.as_mut()));
        for joint_buffer in &mut joint_buffers {
            heph_expect_success!(renderer.destroy_buffer(joint_buffer.as_mut()));
        }
        heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
    }

    fn load_exr(&self, path: &str, channel_names: [&str; 3]) -> image::RgbaImage {
        let layer =
            heph_expect_success!(exr::prelude::read_first_flat_layer_from_file(path)).layer_data;
        let mut channels = channel_names.map(|name| {
            heph_expect_success!(
                layer
                    .channel_data
                    .list
                    .iter()
                    .find(|channel| channel.name == *name)
                    .ok_or("Missing EXR channel.")
            )
            .sample_data
            .values_as_f32()
        });
        image::RgbaImage::from_fn(
            layer.size.width() as u32,
            layer.size.height() as u32,
            |_, _| {
                let [r, g, b] = std::array::from_fn(|i| {
                    (heph_expect_success!(channels[i].next().ok_or("Missing EXR sample."))
                        .clamp(0.0, 1.0)
                        * 255.0)
                        .round() as u8
                });
                image::Rgba([r, g, b, 255])
            },
        )
    }

    fn displace_vertices(
        &self,
        vertices: &mut [SphereVertex],
        displacement: &HeightMap,
        material: &MaterialProperties,
    ) {
        for vertex in vertices {
            let u = vertex[6];
            let v = vertex[7];
            let u = if v == 0.0 || v == 1.0 { 0.0 } else { u };
            let x = u.rem_euclid(1.0) * displacement.width() as f32 - 0.5;
            let y = (v * displacement.height() as f32 - 0.5)
                .clamp(0.0, (displacement.height() - 1) as f32);
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

    fn sphere_position(&self, radius: f32, x_offset: f32, u: f32, v: f32) -> [f32; 3] {
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

    fn sphere_vertices(&self, radius: f32, x_offset: f32) -> Vec<SphereVertex> {
        let mut vertices = Vec::with_capacity(
            BROKEN_BRICK_WALL_LATITUDE_SEGMENTS * BROKEN_BRICK_WALL_LONGITUDE_SEGMENTS * 6,
        );

        for y in 0..BROKEN_BRICK_WALL_LATITUDE_SEGMENTS {
            let v0 = y as f32 / BROKEN_BRICK_WALL_LATITUDE_SEGMENTS as f32;
            let v1 = (y + 1) as f32 / BROKEN_BRICK_WALL_LATITUDE_SEGMENTS as f32;

            for x in 0..BROKEN_BRICK_WALL_LONGITUDE_SEGMENTS {
                let u0 = x as f32 / BROKEN_BRICK_WALL_LONGITUDE_SEGMENTS as f32;
                let u1 = (x + 1) as f32 / BROKEN_BRICK_WALL_LONGITUDE_SEGMENTS as f32;

                let p00 = self.sphere_position(radius, x_offset, u0, v0);
                let p10 = self.sphere_position(radius, x_offset, u1, v0);
                let p01 = self.sphere_position(radius, x_offset, u0, v1);
                let p11 = self.sphere_position(radius, x_offset, u1, v1);

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
                let texture_u0 = u0 * BROKEN_BRICK_WALL_TEXTURE_REPEATS_U;
                let texture_u1 = u1 * BROKEN_BRICK_WALL_TEXTURE_REPEATS_U;

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
}
