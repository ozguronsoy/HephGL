use std::fs::File;

use ddsfile::{Dds, DxgiFormat};
use heph_gl::{
    Error,
    GraphicsDeviceFeature::{AnisotropicFiltering, BcTextures},
    renderers::{
        BufferUsage, CompareOp, FeatureRequest, Renderer, RendererBuffer, RendererHandle,
        RendererResource, RendererSampler, RendererTexture, RendererWorkerFactory, SamplerOptions,
        TextureFormat, TextureOptions,
    },
};

use crate::{heph_expect_err, heph_expect_success, renderer::RendererTests};

const BUFFER_SIZE: usize = 1024;

struct ForeignBuffer;
impl RendererResource for ForeignBuffer {
    fn is_allocated(&self) -> bool {
        false
    }
}
impl RendererBuffer for ForeignBuffer {
    fn size(&self) -> usize {
        4
    }
    fn usage(&self) -> BufferUsage {
        BufferUsage::Storage
    }
}

struct ForeignTexture;
impl RendererResource for ForeignTexture {
    fn is_allocated(&self) -> bool {
        false
    }
}
impl RendererTexture for ForeignTexture {
    fn width(&self) -> u32 {
        1
    }
    fn height(&self) -> u32 {
        1
    }
    fn format(&self) -> TextureFormat {
        TextureFormat::Rgba8Unorm
    }
    fn mip_level_count(&self) -> u32 {
        1
    }
}

struct ForeignSampler;
impl RendererResource for ForeignSampler {
    fn is_allocated(&self) -> bool {
        false
    }
}
impl RendererSampler for ForeignSampler {}

impl<TestRenderer: Renderer> RendererTests<TestRenderer>
where
    RendererHandle<TestRenderer>: RendererWorkerFactory<TestRenderer>,
{
    pub(crate) fn test_buffer_io(&self) {
        let renderer = self.create_renderer_with_any_device(&[]);

        let data = [1u32, 2, 3, 4, 5];
        let buffer_size = data.len() * size_of::<u32>();
        let create_buffer = || {
            let mut buffer =
                heph_expect_success!(renderer.create_buffer(buffer_size, BufferUsage::Storage));
            heph_expect_success!(renderer.write_buffer(
                buffer.as_mut(),
                0,
                bytemuck::cast_slice(&data)
            ));
            buffer
        };

        {
            let mut result = [0u32; 5];
            let mut buffer = create_buffer();
            heph_expect_success!(renderer.read_buffer(
                buffer.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut result)
            ));
            assert_eq!(data, result);
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        }

        {
            let mut result = [0u32; 2];
            let mut buffer = create_buffer();
            heph_expect_success!(renderer.read_buffer(
                buffer.as_ref(),
                2 * size_of::<u32>(),
                bytemuck::cast_slice_mut(&mut result)
            ));
            assert_eq!(result, [3, 4]);
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        }

        {
            let new_data = [5u32, 4, 3, 2, 1];
            let mut result = [0u32; 5];
            let mut buffer = create_buffer();
            heph_expect_success!(renderer.write_buffer(
                buffer.as_mut(),
                0,
                bytemuck::cast_slice(&new_data)
            ));
            heph_expect_success!(renderer.read_buffer(
                buffer.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut result)
            ));
            assert_eq!(result, new_data);
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        }

        {
            let new_data = [32u32, 86];
            let mut result = [0u32; 5];
            let mut buffer = create_buffer();
            heph_expect_success!(renderer.write_buffer(
                buffer.as_mut(),
                3 * size_of::<u32>(),
                bytemuck::cast_slice(&new_data)
            ));
            heph_expect_success!(renderer.read_buffer(
                buffer.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut result)
            ));
            assert_eq!(result, [1u32, 2, 3, 32, 86]);
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        }

        {
            let mut result = [0u32; 10];
            let mut buffer = create_buffer();
            heph_expect_err!(
                renderer.read_buffer(buffer.as_ref(), 0, bytemuck::cast_slice_mut(&mut result)),
                Error::fail("")
            );
            heph_expect_err!(
                renderer.read_buffer(
                    buffer.as_ref(),
                    2 * size_of::<u32>(),
                    bytemuck::cast_slice_mut(&mut result[..5])
                ),
                Error::fail("")
            );
            heph_expect_err!(
                renderer.read_buffer(
                    buffer.as_ref(),
                    5 * size_of::<u32>(),
                    bytemuck::cast_slice_mut(&mut result[..5])
                ),
                Error::invalid_argument("")
            );
            heph_expect_err!(
                renderer.read_buffer(
                    buffer.as_ref(),
                    6 * size_of::<u32>(),
                    bytemuck::cast_slice_mut(&mut result[..5])
                ),
                Error::invalid_argument("")
            );
            heph_expect_err!(
                renderer.read_buffer(
                    buffer.as_ref(),
                    2 * size_of::<u32>(),
                    bytemuck::cast_slice_mut(&mut result[..0])
                ),
                Error::invalid_argument("")
            );
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        }

        {
            let new_data = [0u32; 10];
            let mut buffer = create_buffer();
            heph_expect_err!(
                renderer.write_buffer(buffer.as_mut(), 0, bytemuck::cast_slice(&new_data)),
                Error::fail("")
            );
            heph_expect_err!(
                renderer.write_buffer(
                    buffer.as_mut(),
                    2 * size_of::<u32>(),
                    bytemuck::cast_slice(&new_data[..5])
                ),
                Error::fail("")
            );
            heph_expect_err!(
                renderer.write_buffer(
                    buffer.as_mut(),
                    5 * size_of::<u32>(),
                    bytemuck::cast_slice(&new_data[..1])
                ),
                Error::invalid_argument("")
            );
            heph_expect_err!(
                renderer.write_buffer(
                    buffer.as_mut(),
                    6 * size_of::<u32>(),
                    bytemuck::cast_slice(&new_data[..1])
                ),
                Error::invalid_argument("")
            );
            heph_expect_err!(
                renderer.write_buffer(
                    buffer.as_mut(),
                    2 * size_of::<u32>(),
                    bytemuck::cast_slice(&new_data[..0])
                ),
                Error::invalid_argument("")
            );
            heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        }
    }

    pub(crate) fn test_uniform_buffer(&self) {
        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }
        self.test_buffer(&data, BufferUsage::Uniform);
    }

    pub(crate) fn test_storage_buffer(&self) {
        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }
        self.test_buffer(&data, BufferUsage::Storage);
    }

    pub(crate) fn test_index_buffer(&self) {
        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }
        self.test_buffer(&data, BufferUsage::Index);
    }

    pub(crate) fn test_vertex_buffer(&self) {
        let mut data = Vec::with_capacity(BUFFER_SIZE);
        for i in 0..BUFFER_SIZE {
            data.push(i + 1);
        }
        self.test_buffer(&data, BufferUsage::Vertex);
    }

    pub(crate) fn test_impossible_buffer_size(&self) {
        let renderer = self.create_renderer_with_any_device(&[]);
        let buffer_size = 1024 * 1024 * 1024 * 1024; // 1 TB
        heph_expect_err!(renderer.create_buffer(buffer_size, BufferUsage::Storage));
    }

    pub(crate) fn test_texture(&self) {
        const RGBA_IMAGE_PATH: &str = "assets/broken_brick_wall/broken_brick_wall_diff_1k.jpg";
        const BC_IMAGE_PATH: &str = "assets/medieval_courtyard_scene/cobblestone_floor_04_1k/textures/cobblestone_floor_04_diff_1k.dds";

        let rgba_image = heph_expect_success!(image::open(RGBA_IMAGE_PATH)).into_rgba8();
        let rgba_image_max_mip_level_count =
            TextureOptions::max_mip_level_count(rgba_image.width(), rgba_image.height());

        let bc_image =
            heph_expect_success!(Dds::read(heph_expect_success!(File::open(BC_IMAGE_PATH))));
        let bc_image_format = match heph_expect_success!(
            bc_image
                .get_dxgi_format()
                .ok_or("Missing DDS pixel format.")
        ) {
            DxgiFormat::BC1_UNorm_sRGB => TextureFormat::Bc1Srgb,
            DxgiFormat::BC2_UNorm_sRGB => TextureFormat::Bc2Srgb,
            DxgiFormat::BC3_UNorm_sRGB => TextureFormat::Bc3Srgb,
            DxgiFormat::BC4_UNorm => TextureFormat::Bc4Unorm,
            DxgiFormat::BC5_UNorm => TextureFormat::Bc5Unorm,
            DxgiFormat::BC6H_UF16 => TextureFormat::Bc6hUfloat,
            DxgiFormat::BC7_UNorm => TextureFormat::Bc7Srgb,
            format => panic!("Unsupported DDS pixel format: {format:?}"),
        };

        {
            let mut renderer = self.create_renderer_with_any_device(&[]);

            for mip_level_count in 1..=rgba_image_max_mip_level_count {
                let texture_options = TextureOptions {
                    width: rgba_image.width(),
                    height: rgba_image.height(),
                    format: TextureFormat::Rgba8Srgb,
                    mip_level_count,
                };

                heph_expect_success!(renderer.begin_frame());
                let mut texture =
                    heph_expect_success!(renderer.create_texture(&texture_options, &rgba_image));
                heph_expect_success!(renderer.end_frame());
                heph_expect_success!(renderer.wait_idle());

                assert!(texture.is_allocated());
                assert_eq!(texture.as_ref().width(), texture_options.width);
                assert_eq!(texture.as_ref().height(), texture_options.height);
                assert_eq!(texture.as_ref().format(), texture_options.format);
                assert_eq!(
                    texture.as_ref().mip_level_count(),
                    texture_options.mip_level_count
                );

                heph_expect_success!(renderer.destroy_texture(texture.as_mut()));
                assert!(!texture.is_allocated());
            }
        }

        {
            let mut renderer = self.create_renderer_with_any_device(&[]);
            heph_expect_success!(renderer.begin_frame());
            heph_expect_err!(
                renderer.create_texture(
                    &TextureOptions {
                        width: 0,
                        height: rgba_image.height(),
                        format: TextureFormat::Rgba8Srgb,
                        mip_level_count: TextureOptions::max_mip_level_count(
                            rgba_image.width(),
                            rgba_image.height()
                        ),
                    },
                    &rgba_image
                ),
                Error::invalid_argument("")
            );
            heph_expect_err!(
                renderer.create_texture(
                    &TextureOptions {
                        width: rgba_image.width(),
                        height: 0,
                        format: TextureFormat::Rgba8Srgb,
                        mip_level_count: TextureOptions::max_mip_level_count(
                            rgba_image.width(),
                            rgba_image.height()
                        ),
                    },
                    &rgba_image
                ),
                Error::invalid_argument("")
            );
            heph_expect_err!(
                renderer.create_texture(
                    &TextureOptions {
                        width: rgba_image.width(),
                        height: rgba_image.height(),
                        format: TextureFormat::Rgba8Srgb,
                        mip_level_count: 0,
                    },
                    &rgba_image
                ),
                Error::invalid_argument("")
            );
            heph_expect_err!(
                renderer.create_texture(
                    &TextureOptions {
                        width: rgba_image.width(),
                        height: rgba_image.height(),
                        format: TextureFormat::Rgba8Srgb,
                        mip_level_count: TextureOptions::max_mip_level_count(
                            rgba_image.width(),
                            rgba_image.height()
                        ) + 1,
                    },
                    &rgba_image
                ),
                Error::invalid_argument("")
            );
            heph_expect_success!(renderer.end_frame());
        }

        {
            let mut renderer =
                self.create_renderer_with_any_device(&[FeatureRequest::new(BcTextures, false)]);
            if renderer
                .get_device()
                .is_some_and(|d| d.supported_features().contains(&BcTextures))
            {
                let texture_options = TextureOptions {
                    width: bc_image.get_width(),
                    height: bc_image.get_height(),
                    format: bc_image_format,
                    mip_level_count: bc_image.get_num_mipmap_levels(),
                };

                heph_expect_success!(renderer.begin_frame());
                let mut texture =
                    heph_expect_success!(renderer.create_texture(&texture_options, &bc_image.data));
                heph_expect_success!(renderer.end_frame());
                heph_expect_success!(renderer.wait_idle());

                assert!(texture.is_allocated());
                assert_eq!(texture.as_ref().width(), texture_options.width);
                assert_eq!(texture.as_ref().height(), texture_options.height);
                assert_eq!(texture.as_ref().format(), texture_options.format);
                assert_eq!(
                    texture.as_ref().mip_level_count(),
                    texture_options.mip_level_count
                );

                heph_expect_success!(renderer.destroy_texture(texture.as_mut()));
                assert!(!texture.is_allocated());
            }
        }

        {
            let mut renderer = self.create_renderer_with_any_device(&[]);
            heph_expect_success!(renderer.begin_frame());
            heph_expect_err!(
                renderer.create_texture(
                    &TextureOptions {
                        width: bc_image.get_width(),
                        height: bc_image.get_height(),
                        format: bc_image_format,
                        mip_level_count: bc_image.get_num_mipmap_levels(),
                    },
                    &bc_image.data
                ),
                Error::invalid_operation("")
            );
            heph_expect_success!(renderer.end_frame());
        }

        {
            let mut renderer =
                self.create_renderer_with_any_device(&[FeatureRequest::new(BcTextures, false)]);
            if renderer
                .get_device()
                .is_some_and(|d| d.supported_features().contains(&BcTextures))
            {
                heph_expect_success!(renderer.begin_frame());
                heph_expect_err!(
                    renderer.create_texture(
                        &TextureOptions {
                            width: bc_image.get_width(),
                            height: bc_image.get_height(),
                            format: bc_image_format,
                            mip_level_count: bc_image.get_num_mipmap_levels() - 1,
                        },
                        &bc_image.data
                    ),
                    Error::invalid_operation("")
                );
                heph_expect_err!(
                    renderer.create_texture(
                        &TextureOptions {
                            width: bc_image.get_width(),
                            height: bc_image.get_height(),
                            format: bc_image_format,
                            mip_level_count: bc_image.get_num_mipmap_levels() + 1,
                        },
                        &bc_image.data
                    ),
                    Error::invalid_argument("")
                );
                heph_expect_success!(renderer.end_frame());
            }
        }
    }

    pub(crate) fn test_sampler(&self) {
        {
            let renderer = self.create_renderer_with_any_device(&[]);
            let mut sampler =
                heph_expect_success!(renderer.create_sampler(&SamplerOptions::default()));
            assert!(sampler.as_ref().is_allocated());
            heph_expect_success!(renderer.destroy_sampler(sampler.as_mut()));
            assert!(!sampler.as_ref().is_allocated());
        }

        {
            let renderer = self.create_renderer_with_any_device(&[FeatureRequest::new(
                AnisotropicFiltering,
                false,
            )]);
            if renderer
                .get_device()
                .is_some_and(|d| d.supported_features().contains(&AnisotropicFiltering))
            {
                let mut sampler = heph_expect_success!(renderer.create_sampler(&SamplerOptions {
                    max_anisotropy: Some(1.0),
                    ..Default::default()
                }));
                assert!(sampler.as_ref().is_allocated());
                heph_expect_success!(renderer.destroy_sampler(sampler.as_mut()));
            }
        }

        {
            let renderer = self.create_renderer_with_any_device(&[FeatureRequest::new(
                AnisotropicFiltering,
                false,
            )]);
            heph_expect_err!(
                renderer.create_sampler(&SamplerOptions {
                    max_anisotropy: Some(0.5),
                    ..Default::default()
                }),
                Error::invalid_argument("")
            );
        }

        {
            let renderer = self.create_renderer_with_any_device(&[]);
            heph_expect_err!(
                renderer.create_sampler(&SamplerOptions {
                    max_anisotropy: Some(1.0),
                    ..Default::default()
                }),
                Error::invalid_operation("")
            );
        }

        {
            let renderer = self.create_renderer_with_any_device(&[]);
            let mut sampler = heph_expect_success!(renderer.create_sampler(&SamplerOptions {
                compare_op: Some(CompareOp::Less),
                ..Default::default()
            }));
            assert!(sampler.as_ref().is_allocated());
            heph_expect_success!(renderer.destroy_sampler(sampler.as_mut()));
        }
    }

    pub(crate) fn test_reject_foreign_resources(&self) {
        let renderer = self.create_renderer_with_any_device(&[]);
        let mut buffer: Box<dyn RendererBuffer> = Box::new(ForeignBuffer);
        let mut texture: Box<dyn RendererTexture> = Box::new(ForeignTexture);
        let mut sampler: Box<dyn RendererSampler> = Box::new(ForeignSampler);

        heph_expect_err!(
            renderer.write_buffer(buffer.as_mut(), 0, &[0; 4]),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.read_buffer(buffer.as_ref(), 0, &mut [0; 4]),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.destroy_buffer(buffer.as_mut()),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.destroy_texture(texture.as_mut()),
            Error::invalid_argument("")
        );
        heph_expect_err!(
            renderer.destroy_sampler(sampler.as_mut()),
            Error::invalid_argument("")
        );
    }

    fn test_buffer<T>(&self, data: &Vec<T>, usage: BufferUsage)
    where
        T: bytemuck::Pod + PartialEq + Default + std::fmt::Debug,
    {
        let renderer = self.create_renderer_with_any_device(&[]);

        let buffer_size = data.len() * size_of::<T>();
        let mut buffer = heph_expect_success!(renderer.create_buffer(buffer_size, usage));
        assert!(buffer.is_allocated());
        assert_eq!(buffer.size(), buffer_size);
        assert_eq!(buffer.usage(), usage);

        heph_expect_success!(renderer.write_buffer(buffer.as_mut(), 0, bytemuck::cast_slice(data)));

        let mut output = vec![T::default(); data.len()];
        heph_expect_success!(renderer.read_buffer(
            buffer.as_ref(),
            0,
            bytemuck::cast_slice_mut(&mut output)
        ));

        assert_eq!(*data, output);

        heph_expect_success!(renderer.destroy_buffer(buffer.as_mut()));
        assert_eq!(buffer.size(), 0);
        assert!(!buffer.is_allocated());
    }
}
