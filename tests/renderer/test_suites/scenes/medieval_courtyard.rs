use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use ddsfile::{Dds, DxgiFormat};
use heph_gl::{
    Camera,
    GraphicsDeviceFeature::{AnisotropicFiltering, AsyncTransfer, BcTextures},
    Lighting, Mesh, MeshAlphaMode, MeshVertexAttribute, Projection, Shader,
    math::{Mat4, Quat, Vec3},
    renderers::{
        BufferUsage, CanvasOptions, ColorBlending, CompareOp, CullingMode, DISPLAY_CANVAS,
        DepthBias, FeatureRequest, FrontFace, GraphicsCommandArgs, GraphicsPipelineOptions,
        Renderer, RendererBuffer, ResourceBinding, SamplerAddressMode, SamplerOptions, Settings,
        TextureFormat, TextureOptions,
    },
};

use crate::{heph_expect_success, renderer::RendererTests, utils::SHADERS_DIR};

const SCENE_DIRECTORY: &str = "assets/medieval_courtyard_scene";
const FORT: &str = "modular_fort_01_1k";
const FLOOR: &str = "cobblestone_large_01_1k";
const FLOOR_TILE_SIZE: f32 = 4.0;
const SHADOW_MAP_SIZE: u32 = 2048;
const SHADOW_CENTER: Vec3 = Vec3::new(0.0, 4.0, -5.0);
const SHADOW_EXTENT: f32 = 24.0;
const SUN_DIRECTION: Vec3 = Vec3::new(0.65, -0.95, 0.6);
const SUN_SWEEP: f32 = 60.0;
const ANIMATION_DURATION: Duration = Duration::from_secs(5);

struct Placement {
    mesh_dir: &'static str,
    region: Option<[f32; 4]>,
    origin: Vec3,
    rotation: f32,
    scale: f32,
    position: Vec3,
}

const fn fort_piece(bounds: [f32; 4], rotation: f32, position: Vec3) -> Placement {
    Placement {
        mesh_dir: FORT,
        region: Some(bounds),
        origin: Vec3::new(bounds[0], 0.0, bounds[1]),
        rotation,
        scale: 1.0,
        position,
    }
}

const fn prop(mesh_dir: &'static str, rotation: f32, scale: f32, position: Vec3) -> Placement {
    Placement {
        mesh_dir,
        region: None,
        origin: Vec3::ZERO,
        rotation,
        scale,
        position,
    }
}

const THICK_STRAIGHT_WALL_01: [f32; 4] = [10.0, 19.44, 14.17, 34.0];
const THICK_STRAIGHT_WALL_02: [f32; 4] = [26.0, -0.56, 30.23, 14.0];
const THIN_GATE_WALL: [f32; 4] = [10.07, -14.61, 12.74, -7.2];
const THIN_STRAIGHT_WALL_04: [f32; 4] = [31.54, -14.61, 34.09, -7.2];
const STAIRS_WALL: [f32; 4] = [45.0, -0.82, 48.54, 14.0];

const PLACEMENTS: [Placement; 15] = [
    fort_piece(THICK_STRAIGHT_WALL_02, -90.0, Vec3::new(2.56, 0.0, 0.0)),
    fort_piece(THIN_GATE_WALL, 0.0, Vec3::new(0.0, 0.0, -7.41)),
    fort_piece(THIN_STRAIGHT_WALL_04, 0.0, Vec3::new(0.0, 0.0, -14.82)),
    fort_piece(THICK_STRAIGHT_WALL_01, 90.0, Vec3::new(-16.17, 0.0, -14.0)),
    fort_piece(THICK_STRAIGHT_WALL_01, 0.0, Vec3::new(-16.17, 0.0, -14.0)),
    fort_piece(STAIRS_WALL, 0.0, Vec3::new(12.0, 0.0, -10.0)),
    fort_piece(THICK_STRAIGHT_WALL_01, 0.0, Vec3::new(15.54, 0.0, -10.0)),
    prop("wooden_crate_01_1k", 0.0, 1.4, Vec3::new(-6.5, 0.0, -0.9)),
    prop("wooden_crate_02_1k", 154.0, 1.3, Vec3::new(-2.8, 0.0, -6.2)),
    prop(
        "wooden_lantern_01_1k",
        0.0,
        1.5,
        Vec3::new(-2.8, 0.58, -6.2),
    ),
    prop("wooden_barrels_01_1k", 0.0, 1.0, Vec3::new(-3.0, 0.0, -2.4)),
    prop("potted_plant_01_1k", 0.0, 1.2, Vec3::new(-0.9, 0.0, -6.5)),
    prop("potted_plant_02_1k", 0.0, 1.5, Vec3::new(-4.5, 0.0, -0.7)),
    prop(
        "rock_moss_set_02_1k",
        30.0,
        0.5,
        Vec3::new(-8.0, 0.0, -12.0),
    ),
    prop(
        "rock_moss_set_02_1k",
        200.0,
        0.4,
        Vec3::new(-1.5, 0.0, -11.0),
    ),
];

struct Draw {
    vertex_buffer: Box<dyn RendererBuffer>,
    index_buffer: Box<dyn RendererBuffer>,
    index_count: u32,
    textures: [PathBuf; 3],
    material_buffer: Box<dyn RendererBuffer>,
    double_sided: bool,
    blended: bool,
    center: Vec3,
}

impl<TestRenderer: Renderer> RendererTests<TestRenderer> {
    pub(crate) fn test_render_scene_medieval_courtyard(&self) {
        let create_texture = |renderer: &mut TestRenderer, path: &Path, srgb: bool| {
            let dds = heph_expect_success!(Dds::read(heph_expect_success!(File::open(path))));
            let format = match heph_expect_success!(
                dds.get_dxgi_format().ok_or("Missing DDS pixel format.")
            ) {
                DxgiFormat::BC1_UNorm_sRGB => TextureFormat::Bc1Srgb,
                DxgiFormat::BC2_UNorm_sRGB => TextureFormat::Bc2Srgb,
                DxgiFormat::BC3_UNorm_sRGB => TextureFormat::Bc3Srgb,
                DxgiFormat::BC4_UNorm => TextureFormat::Bc4Unorm,
                DxgiFormat::BC5_UNorm => TextureFormat::Bc5Unorm,
                DxgiFormat::BC7_UNorm => match srgb {
                    true => TextureFormat::Bc7Srgb,
                    false => TextureFormat::Bc7Unorm,
                },
                format => panic!("Unsupported DDS pixel format: {format:?}"),
            };
            heph_expect_success!(renderer.create_texture(
                &TextureOptions {
                    width: dds.get_width(),
                    height: dds.get_height(),
                    format,
                    mip_level_count: dds.get_num_mipmap_levels(),
                },
                heph_expect_success!(dds.get_data(0)),
            ))
        };
        let create_draw = |renderer: &mut TestRenderer,
                           vertices: &[f32],
                           indices: &[u32],
                           textures: [PathBuf; 3],
                           material: [[f32; 4]; 3],
                           double_sided: bool,
                           blended: bool| {
            let mut vertex_buffer = heph_expect_success!(
                renderer.create_buffer(std::mem::size_of_val(vertices), BufferUsage::Vertex)
            );
            let mut index_buffer = heph_expect_success!(
                renderer.create_buffer(std::mem::size_of_val(indices), BufferUsage::Index)
            );
            let mut material_buffer = heph_expect_success!(
                renderer.create_buffer(std::mem::size_of_val(&material), BufferUsage::Uniform)
            );

            heph_expect_success!(renderer.write_buffer(
                vertex_buffer.as_mut(),
                0,
                bytemuck::cast_slice(vertices)
            ));
            heph_expect_success!(renderer.write_buffer(
                index_buffer.as_mut(),
                0,
                bytemuck::cast_slice(indices)
            ));
            heph_expect_success!(renderer.write_buffer(
                material_buffer.as_mut(),
                0,
                bytemuck::cast_slice(&material)
            ));

            Draw {
                vertex_buffer,
                index_buffer,
                index_count: indices.len() as u32,
                textures,
                material_buffer,
                double_sided,
                blended,
                center: vertices
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|vertex| Vec3::from_slice(&vertex[..3]))
                    .sum::<Vec3>()
                    / (vertices.len() / 8) as f32,
            }
        };

        let mut renderer = self.create_renderer_with_any_device(&[
            FeatureRequest::new(BcTextures, false),
            FeatureRequest::new(AnisotropicFiltering, false),
            FeatureRequest::new(AsyncTransfer, false),
        ]);
        if renderer
            .get_device()
            .map(|d| !d.supported_features().contains(&BcTextures))
            .unwrap_or(true)
        {
            return;
        }
        let max_anisotropy = if let Some(device) = renderer.get_device() {
            match device.supported_features().contains(&AnisotropicFiltering) {
                true => Some(device.max_sampler_anisotropy()),
                false => None,
            }
        } else {
            None
        };
        let Some(&msaa) = renderer
            .get_device()
            .and_then(|device| device.supported_msaa_levels().last())
        else {
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
            SHADERS_DIR, "camera_vert.spv"
        )));
        let depth_frag_shader = heph_expect_success!(Shader::from_file(format!(
            "{}/{}",
            SHADERS_DIR, "shadow_depth_frag.spv"
        )));
        let frag_shader = heph_expect_success!(Shader::from_file(format!(
            "{}/{}",
            SHADERS_DIR, "shadowed_mesh_frag.spv"
        )));
        let shadow_pipeline_options = GraphicsPipelineOptions {
            front_face: FrontFace::Clockwise,
            culling: CullingMode::Back,
            depth_only: true,
            depth_bias: Some(DepthBias {
                constant: 2.0,
                slope: 2.5,
                clamp: None,
            }),
            ..Default::default()
        };
        let pipeline_options = GraphicsPipelineOptions {
            front_face: FrontFace::Clockwise,
            culling: CullingMode::Back,
            ..Default::default()
        };
        let mut shadow_pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
            &[&vert_shader, &depth_frag_shader],
            &shadow_pipeline_options,
        ));
        let mut pipeline = heph_expect_success!(
            renderer.create_graphics_pipeline(&[&vert_shader, &frag_shader], &pipeline_options)
        );
        let mut double_sided_shadow_pipeline =
            heph_expect_success!(renderer.create_graphics_pipeline(
                &[&vert_shader, &depth_frag_shader],
                &GraphicsPipelineOptions {
                    culling: CullingMode::Disabled,
                    ..shadow_pipeline_options
                },
            ));
        let mut double_sided_pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions {
                culling: CullingMode::Disabled,
                ..pipeline_options.clone()
            },
        ));
        let mut blended_pipeline = heph_expect_success!(renderer.create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions {
                blending: ColorBlending::Alpha,
                culling: CullingMode::Disabled,
                depth_write: false,
                ..pipeline_options
            },
        ));
        drop(vert_shader);
        drop(depth_frag_shader);
        drop(frag_shader);

        let lighting = Lighting {
            direction: SUN_DIRECTION,
            color: [1.0, 0.92, 0.8],
            intensity: 6.0,
            ambient: [0.16, 0.18, 0.23],
        };
        let mut camera = Camera::new(Projection::Perspective {
            vertical_fov: std::f32::consts::FRAC_PI_3,
            aspect_ratio: 16.0 / 9.0,
            near: 0.1,
            far: 100.0,
        });
        camera.position = Vec3::new(-7.5, 1.6, -8.5);
        heph_expect_success!(camera.look_at(&Vec3::new(-1.0, 2.6, -0.5), &Vec3::Y));

        let scene_directory = Path::new(SCENE_DIRECTORY);
        let mut meshes = HashMap::new();
        for placement in &PLACEMENTS {
            let _ = meshes.entry(placement.mesh_dir).or_insert_with(|| {
                heph_expect_success!(Mesh::from_file(
                    scene_directory
                        .join(placement.mesh_dir)
                        .join(format!("{}.gltf", placement.mesh_dir)),
                ))
            });
        }

        let mut textures = HashMap::new();
        let mut draws = Vec::new();
        heph_expect_success!(renderer.begin_frame());
        heph_expect_success!(renderer.clear(DISPLAY_CANVAS, [0.0, 0.0, 0.0]));
        let mut shadow_canvas = heph_expect_success!(renderer.create_canvas(&CanvasOptions {
            width: SHADOW_MAP_SIZE,
            height: SHADOW_MAP_SIZE,
            depth_only: true,
            ..Default::default()
        }));

        let floor_textures = ["diff", "rough", "nor_gl"].map(|map| {
            scene_directory
                .join(FLOOR)
                .join("textures")
                .join(format!("cobblestone_large_01_{map}_1k.dds"))
        });
        for (path, srgb) in floor_textures.iter().zip([true, false, false]) {
            let _ = textures.insert(path.clone(), create_texture(&mut renderer, path, srgb));
        }
        let floor_corners = [(-30.0, -35.0), (30.0, -35.0), (30.0, 25.0), (-30.0, 25.0)];
        let floor_vertices: Vec<f32> = floor_corners
            .iter()
            .flat_map(|&(x, z)| {
                [
                    x,
                    0.0,
                    z,
                    0.0,
                    1.0,
                    0.0,
                    x / FLOOR_TILE_SIZE,
                    z / FLOOR_TILE_SIZE,
                ]
            })
            .collect();
        draws.push(create_draw(
            &mut renderer,
            &floor_vertices,
            &[0, 2, 1, 0, 3, 2],
            floor_textures,
            [
                [1.0, 1.0, 1.0, 1.0],
                [1.0, 0.0, 1.0, 1.0],
                [0.0, 0.0, 0.0, 0.0],
            ],
            true,
            false,
        ));

        for placement in &PLACEMENTS {
            let rotation = Quat::from_rotation_y(placement.rotation.to_radians());
            let mesh_directory = scene_directory.join(placement.mesh_dir);
            for part in meshes[placement.mesh_dir].parts() {
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

                if let Some([min_x, min_z, max_x, max_z]) = placement.region {
                    let mut min = Vec3::splat(f32::INFINITY);
                    let mut max = Vec3::splat(f32::NEG_INFINITY);
                    for vertex in vertices.chunks_exact(component_count) {
                        let position = Vec3::from_slice(&vertex[..3]);
                        min = min.min(position);
                        max = max.max(position);
                    }
                    let center = (min + max) * 0.5;
                    if !(center.x >= min_x
                        && center.x <= max_x
                        && center.z >= min_z
                        && center.z <= max_z)
                    {
                        continue;
                    }
                }

                for vertex in vertices.chunks_exact_mut(component_count) {
                    let position = rotation
                        * ((Vec3::from_slice(&vertex[..3]) - placement.origin) * placement.scale)
                        + placement.position;
                    let normal = rotation * Vec3::from_slice(&vertex[3..6]);
                    vertex[..3].copy_from_slice(&position.to_array());
                    vertex[3..6].copy_from_slice(&normal.to_array());
                }

                let part_textures = [
                    (
                        heph_expect_success!(
                            part.base_texture_image_uri()
                                .ok_or("Missing base-color texture URI.")
                        ),
                        true,
                    ),
                    (
                        heph_expect_success!(
                            part.arm_texture_image_uri()
                                .ok_or("Missing metallic-roughness texture URI.")
                        ),
                        false,
                    ),
                    (
                        heph_expect_success!(
                            part.normal_texture_image_uri()
                                .ok_or("Missing normal texture URI.")
                        ),
                        false,
                    ),
                ]
                .map(|(uri, srgb)| {
                    let path = mesh_directory.join(uri).with_extension("dds");
                    let _ = textures
                        .entry(path.clone())
                        .or_insert_with(|| create_texture(&mut renderer, &path, srgb));
                    path
                });
                let roughness_only =
                    textures[&part_textures[1]].format() == TextureFormat::Bc4Unorm;
                let (alpha_cutoff, blended) = match part.alpha_mode() {
                    MeshAlphaMode::Opaque => (0.0, false),
                    MeshAlphaMode::Mask { cutoff } => (cutoff, false),
                    MeshAlphaMode::Blend => (0.0, true),
                };
                let material = part.material();
                draws.push(create_draw(
                    &mut renderer,
                    &vertices,
                    part.indices(),
                    part_textures,
                    [
                        [
                            material.base_color[0],
                            material.base_color[1],
                            material.base_color[2],
                            material.base_color[3],
                        ],
                        [
                            material.roughness,
                            material.metallicness,
                            material.normal_scale,
                            f32::from(roughness_only),
                        ],
                        [alpha_cutoff, 0.0, 0.0, 0.0],
                    ],
                    part.is_double_sided(),
                    blended,
                ));
            }
        }
        heph_expect_success!(renderer.end_frame());
        drop(meshes);

        let distance = |draw: &Draw| draw.center.distance(camera.position);
        draws.sort_by(|a, b| match (a.blended, b.blended) {
            (true, true) => distance(b).total_cmp(&distance(a)),
            (a_blended, b_blended) => a_blended.cmp(&b_blended),
        });

        let camera_matrices = [
            Mat4::IDENTITY.to_cols_array(),
            heph_expect_success!(camera.view_projection_matrix()).to_cols_array(),
            Mat4::IDENTITY.to_cols_array(),
        ];
        let mut camera_buffer = heph_expect_success!(renderer.create_buffer(
            std::mem::size_of_val(&camera_matrices),
            BufferUsage::Uniform,
        ));
        heph_expect_success!(renderer.write_buffer(
            camera_buffer.as_mut(),
            0,
            bytemuck::cast_slice(&camera_matrices),
        ));
        let mut frame_buffers = Vec::with_capacity(settings.frames_in_flight);
        for _ in 0..settings.frames_in_flight {
            frame_buffers.push((
                heph_expect_success!(
                    renderer
                        .create_buffer(std::mem::size_of::<[[f32; 16]; 3]>(), BufferUsage::Uniform)
                ),
                heph_expect_success!(
                    renderer
                        .create_buffer(std::mem::size_of::<[[f32; 16]; 2]>(), BufferUsage::Uniform)
                ),
            ));
        }

        let mut sampler = heph_expect_success!(renderer.create_sampler(&SamplerOptions {
            address_mode_u: SamplerAddressMode::Repeat,
            address_mode_v: SamplerAddressMode::Repeat,
            max_anisotropy,
            ..Default::default()
        }));
        let mut shadow_sampler = heph_expect_success!(renderer.create_sampler(&SamplerOptions {
            compare_op: Some(CompareOp::LessOrEqual),
            ..Default::default()
        }));

        let draw_textures = draws
            .iter()
            .map(|draw| draw.textures.each_ref().map(|path| textures[path].as_ref()))
            .collect::<Vec<_>>();
        let mut sun = Camera::new(Projection::Orthographic {
            left: -SHADOW_EXTENT,
            right: SHADOW_EXTENT,
            bottom: -SHADOW_EXTENT,
            top: SHADOW_EXTENT,
            near: 0.0,
            far: 4.0 * SHADOW_EXTENT,
        });
        let sky_color = [0.55, 0.68, 0.85];

        let mut frame_index = 0;
        let start = Instant::now();
        loop {
            heph_expect_success!(renderer.begin_frame());

            let progress =
                (start.elapsed().as_secs_f32() / ANIMATION_DURATION.as_secs_f32()).min(1.0);
            let sun_direction = Quat::from_rotation_y(((progress - 0.5) * SUN_SWEEP).to_radians())
                * lighting.direction.normalize();
            sun.position = SHADOW_CENTER - sun_direction * (2.0 * SHADOW_EXTENT);
            heph_expect_success!(sun.look_at(&SHADOW_CENTER, &Vec3::Y));
            let sun_view_projection = heph_expect_success!(sun.view_projection_matrix());

            let (sun_buffer, scene_buffer) = &mut frame_buffers[frame_index];
            let sun_matrices = [
                Mat4::IDENTITY.to_cols_array(),
                sun_view_projection.to_cols_array(),
                Mat4::IDENTITY.to_cols_array(),
            ];
            heph_expect_success!(renderer.write_buffer(
                sun_buffer.as_mut(),
                0,
                bytemuck::cast_slice(&sun_matrices)
            ));
            let scene_data = [
                sun_view_projection.to_cols_array(),
                [
                    sun_direction.x,
                    sun_direction.y,
                    sun_direction.z,
                    0.0,
                    lighting.color[0],
                    lighting.color[1],
                    lighting.color[2],
                    lighting.intensity,
                    lighting.ambient[0],
                    lighting.ambient[1],
                    lighting.ambient[2],
                    0.0,
                    camera.position.x,
                    camera.position.y,
                    camera.position.z,
                    1.0,
                ],
            ];
            heph_expect_success!(renderer.write_buffer(
                scene_buffer.as_mut(),
                0,
                bytemuck::cast_slice(&scene_data)
            ));

            heph_expect_success!(renderer.clear(DISPLAY_CANVAS, sky_color));

            heph_expect_success!(renderer.clear(Some(shadow_canvas.as_mut()), [0.0, 0.0, 0.0]));
            for (draw, [texture, _, _]) in draws
                .iter()
                .zip(&draw_textures)
                .filter(|(draw, _)| !draw.blended)
            {
                let vertex_bindings = [ResourceBinding::buffer(
                    0,
                    draw.vertex_buffer.as_ref(),
                    0,
                    draw.vertex_buffer.size(),
                )];
                let resource_bindings = [
                    ResourceBinding::texture(0, *texture),
                    ResourceBinding::sampler(1, sampler.as_ref()),
                    ResourceBinding::buffer(
                        4,
                        draw.material_buffer.as_ref(),
                        0,
                        draw.material_buffer.size(),
                    ),
                    ResourceBinding::buffer(5, sun_buffer.as_ref(), 0, sun_buffer.size()),
                ];
                heph_expect_success!(renderer.record_graphics_command(
                    Some(shadow_canvas.as_mut()),
                    match draw.double_sided {
                        true => double_sided_shadow_pipeline.as_mut(),
                        false => shadow_pipeline.as_mut(),
                    },
                    &[&resource_bindings],
                    &GraphicsCommandArgs::VertexArgs {
                        index_buffer: Some(draw.index_buffer.as_ref()),
                        vertex_bindings: &vertex_bindings,
                        draw_count: draw.index_count,
                        instance_count: 1
                    }
                ));
            }

            for (draw, [texture, arm_texture, normal_texture]) in draws.iter().zip(&draw_textures) {
                let vertex_bindings = [ResourceBinding::buffer(
                    0,
                    draw.vertex_buffer.as_ref(),
                    0,
                    draw.vertex_buffer.size(),
                )];
                let resource_bindings = [
                    ResourceBinding::texture(0, *texture),
                    ResourceBinding::sampler(1, sampler.as_ref()),
                    ResourceBinding::texture(2, *arm_texture),
                    ResourceBinding::texture(3, *normal_texture),
                    ResourceBinding::buffer(
                        4,
                        draw.material_buffer.as_ref(),
                        0,
                        draw.material_buffer.size(),
                    ),
                    ResourceBinding::buffer(5, camera_buffer.as_ref(), 0, camera_buffer.size()),
                    ResourceBinding::buffer(6, scene_buffer.as_ref(), 0, scene_buffer.size()),
                    ResourceBinding::texture(7, shadow_canvas.textures().next().unwrap()),
                    ResourceBinding::sampler(8, shadow_sampler.as_ref()),
                ];
                heph_expect_success!(renderer.record_graphics_command(
                    DISPLAY_CANVAS,
                    match (draw.blended, draw.double_sided) {
                        (true, _) => blended_pipeline.as_mut(),
                        (false, true) => double_sided_pipeline.as_mut(),
                        (false, false) => pipeline.as_mut(),
                    },
                    &[&resource_bindings],
                    &GraphicsCommandArgs::VertexArgs {
                        index_buffer: Some(draw.index_buffer.as_ref()),
                        vertex_bindings: &vertex_bindings,
                        draw_count: draw.index_count,
                        instance_count: 1
                    }
                ));
            }

            heph_expect_success!(renderer.end_frame());

            frame_index = (frame_index + 1) % frame_buffers.len();
            if progress == 1.0 {
                break;
            }
        }

        heph_expect_success!(renderer.wait_idle());
        heph_expect_success!(renderer.destroy_sampler(sampler.as_mut()));
        heph_expect_success!(renderer.destroy_sampler(shadow_sampler.as_mut()));
        for texture in textures.values_mut() {
            heph_expect_success!(renderer.destroy_texture(texture.as_mut()));
        }
        for draw in &mut draws {
            heph_expect_success!(renderer.destroy_buffer(draw.vertex_buffer.as_mut()));
            heph_expect_success!(renderer.destroy_buffer(draw.index_buffer.as_mut()));
            heph_expect_success!(renderer.destroy_buffer(draw.material_buffer.as_mut()));
        }
        for (sun_buffer, scene_buffer) in &mut frame_buffers {
            heph_expect_success!(renderer.destroy_buffer(sun_buffer.as_mut()));
            heph_expect_success!(renderer.destroy_buffer(scene_buffer.as_mut()));
        }
        heph_expect_success!(renderer.destroy_buffer(camera_buffer.as_mut()));
        heph_expect_success!(renderer.destroy_canvas(shadow_canvas.as_mut()));
        heph_expect_success!(renderer.destroy_graphics_pipeline(pipeline.as_mut()));
        heph_expect_success!(renderer.destroy_graphics_pipeline(shadow_pipeline.as_mut()));
        heph_expect_success!(renderer.destroy_graphics_pipeline(double_sided_pipeline.as_mut()));
        heph_expect_success!(
            renderer.destroy_graphics_pipeline(double_sided_shadow_pipeline.as_mut())
        );
        heph_expect_success!(renderer.destroy_graphics_pipeline(blended_pipeline.as_mut()));
    }
}
