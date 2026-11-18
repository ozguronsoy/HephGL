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
        DepthBias, FeatureRequest, FrontFace, GraphicsPipelineOptions, Renderer, RendererBuffer,
        RendererTexture, ResourceBinding, SamplerAddressMode, SamplerOptions, TextureFormat,
        TextureOptions,
    },
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use renkrs::RGB;

mod utils;

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

/// Places the parts of a mesh in the courtyard.
struct Placement {
    mesh_dir: &'static str,
    region: Option<[f32; 4]>,
    origin: Vec3,
    /// Around the vertical axis in degrees.
    rotation: f32,
    scale: f32,
    position: Vec3,
}

/// Places a piece of the modular fort, picked by its bounds in the fort's file.
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

/// Places a whole prop mesh, moving the origin of its file to `position`.
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

// The bounds of the fort pieces in the fort's file.
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

/// Holds the GPU resources of a mesh part or the floor.
struct Draw {
    vertex_buffer: Box<dyn RendererBuffer>,
    index_buffer: Box<dyn RendererBuffer>,
    index_count: u32,
    /// The base-color, metallic-roughness, and normal texture paths.
    textures: [PathBuf; 3],
    material_buffer: Box<dyn RendererBuffer>,
    double_sided: bool,
    blended: bool,
    center: Vec3,
}

/// Creates a texture from a DDS file that contains a block-compressed image and its mip levels.
fn create_texture(
    renderer: &mut utils::renderer::ExampleRenderer,
    path: &Path,
    srgb: bool,
) -> Box<dyn RendererTexture> {
    let dds = Dds::read(File::open(path).unwrap()).unwrap();
    let format = match dds.get_dxgi_format().unwrap() {
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
    renderer
        .create_texture(
            &TextureOptions {
                width: dds.get_width(),
                height: dds.get_height(),
                format,
                mip_level_count: dds.get_num_mipmap_levels(),
            },
            dds.get_data(0).unwrap(),
        )
        .unwrap()
}

/// Uploads vertices in the [px, py, pz, nx, ny, nz, u, v] layout, the indices, and the material.
fn create_draw(
    renderer: &mut utils::renderer::ExampleRenderer,
    vertices: &[f32],
    indices: &[u32],
    textures: [PathBuf; 3],
    material: [[f32; 4]; 3],
    double_sided: bool,
    blended: bool,
) -> Draw {
    let mut vertex_buffer = renderer
        .create_buffer(std::mem::size_of_val(vertices), BufferUsage::Vertex)
        .unwrap();
    let mut index_buffer = renderer
        .create_buffer(std::mem::size_of_val(indices), BufferUsage::Index)
        .unwrap();
    let mut material_buffer = renderer
        .create_buffer(std::mem::size_of_val(&material), BufferUsage::Uniform)
        .unwrap();

    renderer
        .write_buffer(vertex_buffer.as_mut(), 0, bytemuck::cast_slice(vertices))
        .unwrap();
    renderer
        .write_buffer(index_buffer.as_mut(), 0, bytemuck::cast_slice(indices))
        .unwrap();
    renderer
        .write_buffer(material_buffer.as_mut(), 0, bytemuck::cast_slice(&material))
        .unwrap();

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
}

fn example(
    renderer: &mut utils::renderer::ExampleRenderer,
    _: RawWindowHandle,
    _: RawDisplayHandle,
) {
    let device = utils::renderer::get_best_device(renderer);
    renderer
        .set_device(
            Some(&device),
            &[
                FeatureRequest::new(BcTextures, false),
                FeatureRequest::new(AnisotropicFiltering, false),
                FeatureRequest::new(AsyncTransfer, false),
            ],
        )
        .unwrap();
    if !device.supported_features().contains(&BcTextures) {
        println!("The graphics device doesn't support BC textures, which the scene uses.");
        std::process::exit(0);
    }
    utils::renderer::set_best_msaa(renderer);

    let mut settings = *renderer.get_settings();
    settings.frames_in_flight = 2;
    settings.vsync = true;
    renderer.set_settings(settings).unwrap();

    let vert_shader =
        Shader::from_file(format!("{}/{}", utils::SHADERS_DIR, "camera_vert.spv")).unwrap();
    let depth_frag_shader = Shader::from_file(format!(
        "{}/{}",
        utils::SHADERS_DIR,
        "shadow_depth_frag.spv"
    ))
    .unwrap();
    let frag_shader = Shader::from_file(format!(
        "{}/{}",
        utils::SHADERS_DIR,
        "shadowed_mesh_frag.spv"
    ))
    .unwrap();
    // The shadow pass renders the scene from the sun into a depth-only canvas. Depth bias pushes
    // the stored depths away from the sun, so lit surfaces don't shadow themselves.
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
    let mut shadow_pipeline = renderer
        .create_graphics_pipeline(
            &[&vert_shader, &depth_frag_shader],
            &shadow_pipeline_options,
        )
        .unwrap();
    let mut pipeline = renderer
        .create_graphics_pipeline(&[&vert_shader, &frag_shader], &pipeline_options)
        .unwrap();
    // Double-sided parts are visible from behind, so their back faces aren't culled.
    let mut double_sided_shadow_pipeline = renderer
        .create_graphics_pipeline(
            &[&vert_shader, &depth_frag_shader],
            &GraphicsPipelineOptions {
                culling: CullingMode::Disabled,
                ..shadow_pipeline_options
            },
        )
        .unwrap();
    let mut double_sided_pipeline = renderer
        .create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions {
                culling: CullingMode::Disabled,
                ..pipeline_options
            },
        )
        .unwrap();
    // Blended parts are tested against the depths of the opaque parts but don't write their own,
    // so the blended parts behind them still show through. The scene's only blended parts, the
    // lantern's glass panes, are double-sided.
    let mut blended_pipeline = renderer
        .create_graphics_pipeline(
            &[&vert_shader, &frag_shader],
            &GraphicsPipelineOptions {
                blending: ColorBlending::Alpha,
                culling: CullingMode::Disabled,
                depth_write: false,
                ..pipeline_options
            },
        )
        .unwrap();
    drop(vert_shader);
    drop(depth_frag_shader);
    drop(frag_shader);

    let lighting = Lighting {
        direction: SUN_DIRECTION,
        color: RGB::new(1.0, 0.92, 0.8),
        intensity: 6.0,
        ambient: RGB::new(0.16, 0.18, 0.23),
    };
    let mut camera = Camera::new(Projection::Perspective {
        vertical_fov: std::f32::consts::FRAC_PI_3,
        aspect_ratio: 16.0 / 9.0,
        near: 0.1,
        far: 100.0,
    });
    camera.position = Vec3::new(-7.5, 1.6, -8.5);
    camera
        .look_at(&Vec3::new(-1.0, 2.6, -0.5), &Vec3::Y)
        .unwrap();

    let scene_directory = Path::new(SCENE_DIRECTORY);
    let mut meshes = HashMap::new();
    for placement in &PLACEMENTS {
        let _ = meshes.entry(placement.mesh_dir).or_insert_with(|| {
            Mesh::from_file(
                scene_directory
                    .join(placement.mesh_dir)
                    .join(format!("{}.gltf", placement.mesh_dir)),
            )
            .unwrap()
        });
    }

    let mut textures = HashMap::new();
    let mut draws = Vec::new();
    renderer.begin_frame().unwrap();
    renderer.clear(DISPLAY_CANVAS, RGB::default()).unwrap();
    let mut shadow_canvas = renderer
        .create_canvas(&CanvasOptions {
            width: SHADOW_MAP_SIZE,
            height: SHADOW_MAP_SIZE,
            depth_only: true,
        })
        .unwrap();

    // The floor is a single quad that repeats the cobblestone textures.
    let floor_textures = ["diff", "rough", "nor_gl"].map(|map| {
        scene_directory
            .join(FLOOR)
            .join("textures")
            .join(format!("cobblestone_large_01_{map}_1k.dds"))
    });
    for (path, srgb) in floor_textures.iter().zip([true, false, false]) {
        let _ = textures.insert(path.clone(), create_texture(renderer, path, srgb));
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
        renderer,
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
                MeshVertexAttribute::TextureCoordinates(part.base_texture_uv_set().unwrap()),
            ];
            let mut vertices = part.vertices(&vertex_attributes).unwrap();
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

            // The scene doesn't move, so the placement is applied to the vertices once.
            for vertex in vertices.chunks_exact_mut(component_count) {
                let position = rotation
                    * ((Vec3::from_slice(&vertex[..3]) - placement.origin) * placement.scale)
                    + placement.position;
                let normal = rotation * Vec3::from_slice(&vertex[3..6]);
                vertex[..3].copy_from_slice(&position.to_array());
                vertex[3..6].copy_from_slice(&normal.to_array());
            }

            // The glTF files still refer to the original JPG textures, which were converted to
            // DDS files with the same names.
            let part_textures = [
                (part.base_texture_image_uri().unwrap(), true),
                (part.arm_texture_image_uri().unwrap(), false),
                (part.normal_texture_image_uri().unwrap(), false),
            ]
            .map(|(uri, srgb)| {
                let path = mesh_directory.join(uri).with_extension("dds");
                let _ = textures
                    .entry(path.clone())
                    .or_insert_with(|| create_texture(renderer, &path, srgb));
                path
            });
            // Single-channel metallic-roughness textures store only the roughness.
            let roughness_only = textures[&part_textures[1]].format() == TextureFormat::Bc4Unorm;
            let (alpha_cutoff, blended) = match part.alpha_mode() {
                MeshAlphaMode::Opaque => (0.0, false),
                MeshAlphaMode::Mask { cutoff } => (cutoff, false),
                MeshAlphaMode::Blend => (0.0, true),
            };
            let material = part.material();
            draws.push(create_draw(
                renderer,
                &vertices,
                part.indices(),
                part_textures,
                [
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
                        f32::from(roughness_only),
                    ],
                    [alpha_cutoff, 0.0, 0.0, 0.0],
                ],
                part.is_double_sided(),
                blended,
            ));
        }
    }
    renderer.end_frame().unwrap();
    drop(meshes);

    // Blended parts are drawn last, from back to front, so each one blends with everything behind
    // it.
    let distance = |draw: &Draw| draw.center.distance(camera.position);
    draws.sort_by(|a, b| match (a.blended, b.blended) {
        (true, true) => distance(b).total_cmp(&distance(a)),
        (a_blended, b_blended) => a_blended.cmp(&b_blended),
    });

    // Every vertex is already in world space, so the model and normal matrices are identities.
    let camera_matrices = [
        Mat4::IDENTITY.to_cols_array(),
        camera.view_projection_matrix().unwrap().to_cols_array(),
        Mat4::IDENTITY.to_cols_array(),
    ];
    let mut camera_buffer = renderer
        .create_buffer(
            std::mem::size_of_val(&camera_matrices),
            BufferUsage::Uniform,
        )
        .unwrap();
    renderer
        .write_buffer(
            camera_buffer.as_mut(),
            0,
            bytemuck::cast_slice(&camera_matrices),
        )
        .unwrap();
    // The sun moves, so its matrices and the scene data change every frame.
    let mut frame_buffers = Vec::with_capacity(settings.frames_in_flight);
    for _ in 0..settings.frames_in_flight {
        frame_buffers.push((
            renderer
                .create_buffer(std::mem::size_of::<[[f32; 16]; 3]>(), BufferUsage::Uniform)
                .unwrap(),
            renderer
                .create_buffer(std::mem::size_of::<[[f32; 16]; 2]>(), BufferUsage::Uniform)
                .unwrap(),
        ));
    }

    let mut sampler = renderer
        .create_sampler(&SamplerOptions {
            address_mode_u: SamplerAddressMode::Repeat,
            address_mode_v: SamplerAddressMode::Repeat,
            max_anisotropy: match device.supported_features().contains(&AnisotropicFiltering) {
                true => Some(device.max_sampler_anisotropy()),
                false => None,
            },
            ..Default::default()
        })
        .unwrap();
    let mut shadow_sampler = renderer
        .create_sampler(&SamplerOptions {
            compare_op: Some(CompareOp::LessOrEqual),
            ..Default::default()
        })
        .unwrap();

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
    let sky_color = RGB::new(0.55, 0.68, 0.85);

    let mut frame_index = 0;
    let start = Instant::now();
    loop {
        renderer.begin_frame().unwrap();

        let progress = (start.elapsed().as_secs_f32() / ANIMATION_DURATION.as_secs_f32()).min(1.0);
        let sun_direction = Quat::from_rotation_y(((progress - 0.5) * SUN_SWEEP).to_radians())
            * lighting.direction.normalize();
        sun.position = SHADOW_CENTER - sun_direction * (2.0 * SHADOW_EXTENT);
        sun.look_at(&SHADOW_CENTER, &Vec3::Y).unwrap();
        let sun_view_projection = sun.view_projection_matrix().unwrap();

        let (sun_buffer, scene_buffer) = &mut frame_buffers[frame_index];
        let sun_matrices = [
            Mat4::IDENTITY.to_cols_array(),
            sun_view_projection.to_cols_array(),
            Mat4::IDENTITY.to_cols_array(),
        ];
        renderer
            .write_buffer(sun_buffer.as_mut(), 0, bytemuck::cast_slice(&sun_matrices))
            .unwrap();
        let scene_data = [
            sun_view_projection.to_cols_array(),
            [
                sun_direction.x,
                sun_direction.y,
                sun_direction.z,
                0.0,
                lighting.color.r,
                lighting.color.g,
                lighting.color.b,
                lighting.intensity,
                lighting.ambient.r,
                lighting.ambient.g,
                lighting.ambient.b,
                0.0,
                camera.position.x,
                camera.position.y,
                camera.position.z,
                1.0,
            ],
        ];
        renderer
            .write_buffer(scene_buffer.as_mut(), 0, bytemuck::cast_slice(&scene_data))
            .unwrap();

        renderer.clear(DISPLAY_CANVAS, sky_color).unwrap();

        // Render the depths seen from the sun into the shadow map. Blended parts let the light
        // through, so they don't cast shadows.
        renderer
            .clear(Some(shadow_canvas.as_mut()), RGB::default())
            .unwrap();
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
            renderer
                .record_graphics_command(
                    Some(shadow_canvas.as_mut()),
                    match draw.double_sided {
                        true => double_sided_shadow_pipeline.as_mut(),
                        false => shadow_pipeline.as_mut(),
                    },
                    Some(draw.index_buffer.as_ref()),
                    &vertex_bindings,
                    &[&resource_bindings],
                    draw.index_count,
                    1,
                )
                .unwrap();
        }

        // Render the scene from the camera, sampling the shadow map to find what the sun reaches.
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
                ResourceBinding::texture(7, shadow_canvas.texture()),
                ResourceBinding::sampler(8, shadow_sampler.as_ref()),
            ];
            renderer
                .record_graphics_command(
                    DISPLAY_CANVAS,
                    match (draw.blended, draw.double_sided) {
                        (true, _) => blended_pipeline.as_mut(),
                        (false, true) => double_sided_pipeline.as_mut(),
                        (false, false) => pipeline.as_mut(),
                    },
                    Some(draw.index_buffer.as_ref()),
                    &vertex_bindings,
                    &[&resource_bindings],
                    draw.index_count,
                    1,
                )
                .unwrap();
        }

        renderer.end_frame().unwrap();

        frame_index = (frame_index + 1) % frame_buffers.len();
        if progress == 1.0 {
            break;
        }
    }

    // Cleanup.
    renderer.wait_idle().unwrap();
    renderer.destroy_sampler(sampler.as_mut()).unwrap();
    renderer.destroy_sampler(shadow_sampler.as_mut()).unwrap();
    for texture in textures.values_mut() {
        renderer.destroy_texture(texture.as_mut()).unwrap();
    }
    for draw in &mut draws {
        renderer
            .destroy_buffer(draw.vertex_buffer.as_mut())
            .unwrap();
        renderer.destroy_buffer(draw.index_buffer.as_mut()).unwrap();
        renderer
            .destroy_buffer(draw.material_buffer.as_mut())
            .unwrap();
    }
    for (sun_buffer, scene_buffer) in &mut frame_buffers {
        renderer.destroy_buffer(sun_buffer.as_mut()).unwrap();
        renderer.destroy_buffer(scene_buffer.as_mut()).unwrap();
    }
    renderer.destroy_buffer(camera_buffer.as_mut()).unwrap();
    renderer.destroy_canvas(shadow_canvas.as_mut()).unwrap();
    renderer
        .destroy_graphics_pipeline(pipeline.as_mut())
        .unwrap();
    renderer
        .destroy_graphics_pipeline(shadow_pipeline.as_mut())
        .unwrap();
    renderer
        .destroy_graphics_pipeline(double_sided_pipeline.as_mut())
        .unwrap();
    renderer
        .destroy_graphics_pipeline(double_sided_shadow_pipeline.as_mut())
        .unwrap();
    renderer
        .destroy_graphics_pipeline(blended_pipeline.as_mut())
        .unwrap();

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Render Scene: Medieval Courtyard";
    const INIT_RENDERER: bool = true;
    utils::run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
