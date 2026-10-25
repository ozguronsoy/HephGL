use std::sync::{Arc, Mutex};

use heph_gl::{
    GraphicsDeviceFeature::ComputeShaders,
    GraphicsDeviceType,
    GraphicsDeviceType::{Cpu, DiscreteGpu, IntegratedGpu, Other, VirtualGpu},
    Shader,
    renderers::{
        BufferUsage, FeatureRequest, Renderer, RendererHandle, RendererWorkerFactory,
        ResourceBinding, ResourceBindingType, Settings,
    },
};

use crate::{heph_expect_success, renderer::RendererTests, utils::SHADERS_DIR};

const DATA_COUNT: usize = 256;
const BYTE_SIZE: usize = DATA_COUNT * std::mem::size_of::<f32>();

impl<TestRenderer: Renderer> RendererTests<TestRenderer>
where
    RendererHandle<TestRenderer>: RendererWorkerFactory<TestRenderer>,
{
    pub(crate) fn test_single_threaded_compute_discrete_gpu(&self) {
        const TARGET_DEVICE_TYPE: GraphicsDeviceType = DiscreteGpu;
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    pub(crate) fn test_single_threaded_compute_integrated_gpu(&self) {
        const TARGET_DEVICE_TYPE: GraphicsDeviceType = IntegratedGpu;
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    pub(crate) fn test_single_threaded_compute_cpu(&self) {
        const TARGET_DEVICE_TYPE: GraphicsDeviceType = Cpu;
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    pub(crate) fn test_single_threaded_compute_virtual_gpu(&self) {
        const TARGET_DEVICE_TYPE: GraphicsDeviceType = VirtualGpu;
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    pub(crate) fn test_single_threaded_compute_other(&self) {
        const TARGET_DEVICE_TYPE: GraphicsDeviceType = Other;
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 1, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 2, 10);
        self.test_single_threaded_compute(TARGET_DEVICE_TYPE, 3, 10);
    }

    pub(crate) fn test_multi_threaded_compute_discrete_gpu(&self) {
        const TARGET_DEVICE_TYPE: GraphicsDeviceType = DiscreteGpu;
        self.test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    pub(crate) fn test_multi_threaded_compute_integrated_gpu(&self) {
        const TARGET_DEVICE_TYPE: GraphicsDeviceType = IntegratedGpu;
        self.test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    pub(crate) fn test_multi_threaded_compute_cpu(&self) {
        const TARGET_DEVICE_TYPE: GraphicsDeviceType = Cpu;
        self.test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    pub(crate) fn test_multi_threaded_compute_virtual_gpu(&self) {
        const TARGET_DEVICE_TYPE: GraphicsDeviceType = VirtualGpu;
        self.test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    pub(crate) fn test_multi_threaded_compute_other(&self) {
        const TARGET_DEVICE_TYPE: GraphicsDeviceType = GraphicsDeviceType::Other;
        self.test_multi_threaded_compute(TARGET_DEVICE_TYPE, 10);
    }

    fn test_single_threaded_compute(
        &self,
        target_device_type: GraphicsDeviceType,
        frames_in_flight: usize,
        n_frames: usize,
    ) {
        let create_renderer_result = self.create_renderer_with_target_device(
            target_device_type,
            &[FeatureRequest::new(ComputeShaders, true)],
            false,
        );
        assert!(
            create_renderer_result.is_some(),
            "The test should be skipped if device of the target type does not exist."
        );
        let mut renderer = heph_expect_success!(
            create_renderer_result.ok_or("No device of the target type found.")
        );

        let settings = Settings {
            frames_in_flight,
            ..Default::default()
        };
        heph_expect_success!(renderer.set_settings(settings));

        let shader = heph_expect_success!(Shader::from_file(format!(
            "{}/{}",
            SHADERS_DIR, "addition.spv"
        )));
        let mut pipeline = heph_expect_success!(renderer.create_compute_pipeline(&shader));
        drop(shader);

        let mut buffers = Vec::with_capacity(n_frames);

        for i in 0..n_frames {
            heph_expect_success!(renderer.begin_frame());

            let mut a_data = vec![0.0f32; DATA_COUNT];
            let mut b_data = vec![0.0f32; DATA_COUNT];
            for j in 0..DATA_COUNT {
                a_data[j] = (j * (i + 1)) as f32;
                b_data[j] = ((j * 2) * (i + 1)) as f32;
            }

            let mut buffer_a =
                heph_expect_success!(renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage));
            let mut buffer_b =
                heph_expect_success!(renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage));
            let buffer_c =
                heph_expect_success!(renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage));

            heph_expect_success!(renderer.write_buffer(
                buffer_a.as_mut(),
                0,
                bytemuck::cast_slice(&a_data)
            ));
            heph_expect_success!(renderer.write_buffer(
                buffer_b.as_mut(),
                0,
                bytemuck::cast_slice(&b_data)
            ));

            let bindings = [
                ResourceBinding {
                    binding: 0,
                    resource: ResourceBindingType::Buffer {
                        handle: buffer_a.as_ref(),
                        usage: BufferUsage::Storage,
                        offset: 0,
                        size: buffer_a.size(),
                    },
                },
                ResourceBinding {
                    binding: 1,
                    resource: ResourceBindingType::Buffer {
                        handle: buffer_b.as_ref(),
                        usage: BufferUsage::Storage,
                        offset: 0,
                        size: buffer_b.size(),
                    },
                },
                ResourceBinding {
                    binding: 2,
                    resource: ResourceBindingType::Buffer {
                        handle: buffer_c.as_ref(),
                        usage: BufferUsage::Storage,
                        offset: 0,
                        size: buffer_c.size(),
                    },
                },
            ];

            heph_expect_success!(renderer.record_compute_command(
                pipeline.as_mut(),
                &[&bindings],
                (1, 1, 1)
            ));

            heph_expect_success!(renderer.end_frame());

            buffers.push((buffer_a, buffer_b, buffer_c));
        }

        // Wait for all frames to finish.
        heph_expect_success!(renderer.wait_idle());

        for mut buffer in buffers {
            let mut data_a = vec![0.0f32; DATA_COUNT];
            let mut data_b = vec![0.0f32; DATA_COUNT];
            let mut data_c = vec![0.0f32; DATA_COUNT];

            heph_expect_success!(renderer.read_buffer(
                buffer.0.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut data_a)
            ));
            heph_expect_success!(renderer.read_buffer(
                buffer.1.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut data_b)
            ));
            heph_expect_success!(renderer.read_buffer(
                buffer.2.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut data_c)
            ));

            for i in 0..DATA_COUNT {
                assert_eq!(data_a[i] + data_b[i], data_c[i]);
            }

            heph_expect_success!(renderer.destroy_buffer(buffer.0.as_mut()));
            heph_expect_success!(renderer.destroy_buffer(buffer.1.as_mut()));
            heph_expect_success!(renderer.destroy_buffer(buffer.2.as_mut()));
        }

        heph_expect_success!(renderer.destroy_compute_pipeline(pipeline.as_mut()));
    }

    fn test_multi_threaded_compute(
        &self,
        target_device_type: GraphicsDeviceType,
        n_threads: usize,
    ) {
        let create_renderer_result = self.create_renderer_with_target_device(
            target_device_type,
            &[FeatureRequest::new(ComputeShaders, true)],
            false,
        );
        assert!(
            create_renderer_result.is_some(),
            "The test should be skipped if device of the target type does not exist."
        );
        let mut renderer = heph_expect_success!(
            create_renderer_result.ok_or("No device of the target type found.")
        );

        let shader_path = SHADERS_DIR.to_owned() + "/addition.spv";
        let shader = heph_expect_success!(Shader::from_file(shader_path));
        let pipeline = Arc::new(Mutex::new(heph_expect_success!(
            renderer.create_compute_pipeline(&shader)
        )));
        drop(shader);

        std::thread::scope(|s| {
            let (tx, rx) = std::sync::mpsc::channel();
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(n_threads + 1));
            let renderer_handle = RendererHandle::from(&mut renderer);

            for thread_id in 0..n_threads {
                let tx = tx.clone();
                let barrier = barrier.clone();
                let pipeline = pipeline.clone();

                let _ = s.spawn(move || {
                    let mut renderer_worker = heph_expect_success!(renderer_handle.spawn_worker());
                    heph_expect_success!(renderer_worker.begin_frame());

                    let i = thread_id + 1;

                    let mut a_data = vec![0.0f32; DATA_COUNT];
                    let mut b_data = vec![0.0f32; DATA_COUNT];
                    for j in 0..DATA_COUNT {
                        a_data[j] = (j * i) as f32;
                        b_data[j] = ((j * 2) * i) as f32;
                    }

                    let mut buffer_a = heph_expect_success!(
                        renderer_worker.create_buffer(BYTE_SIZE, BufferUsage::Storage)
                    );
                    let mut buffer_b = heph_expect_success!(
                        renderer_worker.create_buffer(BYTE_SIZE, BufferUsage::Storage)
                    );
                    let buffer_c = heph_expect_success!(
                        renderer_worker.create_buffer(BYTE_SIZE, BufferUsage::Storage)
                    );

                    heph_expect_success!(renderer_worker.write_buffer(
                        buffer_a.as_mut(),
                        0,
                        bytemuck::cast_slice(&a_data)
                    ));
                    heph_expect_success!(renderer_worker.write_buffer(
                        buffer_b.as_mut(),
                        0,
                        bytemuck::cast_slice(&b_data)
                    ));

                    let bindings = [
                        ResourceBinding {
                            binding: 0,
                            resource: ResourceBindingType::Buffer {
                                handle: buffer_a.as_ref(),
                                usage: BufferUsage::Storage,
                                offset: 0,
                                size: buffer_a.size(),
                            },
                        },
                        ResourceBinding {
                            binding: 1,
                            resource: ResourceBindingType::Buffer {
                                handle: buffer_b.as_ref(),
                                usage: BufferUsage::Storage,
                                offset: 0,
                                size: buffer_b.size(),
                            },
                        },
                        ResourceBinding {
                            binding: 2,
                            resource: ResourceBindingType::Buffer {
                                handle: buffer_c.as_ref(),
                                usage: BufferUsage::Storage,
                                offset: 0,
                                size: buffer_c.size(),
                            },
                        },
                    ];
                    heph_expect_success!(renderer_worker.record_compute_command(
                        heph_expect_success!(pipeline.lock()).as_mut(),
                        &[&bindings],
                        (1, 1, 1)
                    ));

                    heph_expect_success!(renderer_worker.end_frame());

                    heph_expect_success!(tx.send((buffer_a, buffer_b, buffer_c, a_data, b_data,)));
                    let _ = barrier.wait();
                });
            }

            heph_expect_success!(renderer.begin_frame());

            let mut cleanup_data = Vec::with_capacity(n_threads);
            for _ in 0..n_threads {
                let (buffer_a, buffer_b, buffer_c, a_data, b_data) =
                    heph_expect_success!(rx.recv());
                cleanup_data.push((buffer_a, buffer_b, buffer_c, a_data, b_data));
            }

            heph_expect_success!(renderer.end_frame());
            heph_expect_success!(renderer.wait_idle());

            for (mut buffer_a, mut buffer_b, mut buffer_c, a_data, b_data) in cleanup_data {
                let mut c_data = vec![0.0f32; DATA_COUNT];
                heph_expect_success!(renderer.read_buffer(
                    buffer_c.as_ref(),
                    0,
                    bytemuck::cast_slice_mut(&mut c_data)
                ));

                for j in 0..DATA_COUNT {
                    assert_eq!(a_data[j] + b_data[j], c_data[j]);
                }

                heph_expect_success!(renderer.destroy_buffer(buffer_a.as_mut()));
                heph_expect_success!(renderer.destroy_buffer(buffer_b.as_mut()));
                heph_expect_success!(renderer.destroy_buffer(buffer_c.as_mut()));
            }
            let _ = barrier.wait();
        });

        heph_expect_success!(
            renderer.destroy_compute_pipeline(heph_expect_success!(pipeline.lock()).as_mut())
        );
    }
}
