use heph_gl::{
    Error,
    GraphicsDeviceFeature::ComputeShaders,
    GraphicsDeviceType,
    GraphicsDeviceType::{Cpu, DiscreteGpu, IntegratedGpu, Other, VirtualGpu},
    Result, Shader,
    renderers::{
        BufferUsage, FeatureRequest, Renderer, RendererBuffer, RendererComputePipeline,
        RendererTaskParam, ResourceBinding, Settings,
    },
};

use crate::{heph_expect_success, renderer::RendererTests, utils::SHADERS_DIR};

const DATA_COUNT: usize = 256;
const BYTE_SIZE: usize = DATA_COUNT * std::mem::size_of::<f32>();

impl<TestRenderer: Renderer> RendererTests<TestRenderer> {
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
                ResourceBinding::buffer(0, buffer_a.as_ref(), 0, buffer_a.size()),
                ResourceBinding::buffer(1, buffer_b.as_ref(), 0, buffer_b.size()),
                ResourceBinding::buffer(2, buffer_c.as_ref(), 0, buffer_c.size()),
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

        struct WorkerIndex {
            pipeline: Box<dyn RendererComputePipeline>,
            index: usize,
        }
        impl RendererTaskParam for WorkerIndex {}

        struct AdditionResources {
            pipeline: Box<dyn RendererComputePipeline>,
            buffer_a: Box<dyn RendererBuffer>,
            buffer_b: Box<dyn RendererBuffer>,
            buffer_c: Box<dyn RendererBuffer>,
            a_data: Vec<f32>,
            b_data: Vec<f32>,
        }
        impl RendererTaskParam for AdditionResources {}

        fn begin_task(
            renderer: &mut dyn Renderer,
            _: Option<&mut dyn RendererTaskParam>,
        ) -> Result<Option<Box<dyn RendererTaskParam>>> {
            renderer.begin_frame()?;
            Ok(None)
        }

        fn create_resources_task(
            renderer: &mut dyn Renderer,
            param: Option<&mut dyn RendererTaskParam>,
        ) -> Result<Option<Box<dyn RendererTaskParam>>> {
            let param = param
                .ok_or(Error::invalid_argument("Missing the worker index."))?
                .downcast_ref::<WorkerIndex>()?;

            let i = param.index + 1;
            let mut a_data = vec![0.0f32; DATA_COUNT];
            let mut b_data = vec![0.0f32; DATA_COUNT];
            for j in 0..DATA_COUNT {
                a_data[j] = (j * i) as f32;
                b_data[j] = ((j * 2) * i) as f32;
            }

            let mut buffer_a = renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage)?;
            let mut buffer_b = renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage)?;
            let buffer_c = renderer.create_buffer(BYTE_SIZE, BufferUsage::Storage)?;
            renderer.write_buffer(buffer_a.as_mut(), 0, bytemuck::cast_slice(&a_data))?;
            renderer.write_buffer(buffer_b.as_mut(), 0, bytemuck::cast_slice(&b_data))?;

            Ok(Some(Box::new(AdditionResources {
                pipeline: param.pipeline.clone(),
                buffer_a,
                buffer_b,
                buffer_c,
                a_data,
                b_data,
            })))
        }

        fn addition_task(
            renderer: &mut dyn Renderer,
            param: Option<&mut dyn RendererTaskParam>,
        ) -> Result<Option<Box<dyn RendererTaskParam>>> {
            let param = param
                .ok_or(Error::invalid_argument("Missing the addition resources."))?
                .downcast_mut::<AdditionResources>()?;

            let bindings = [
                ResourceBinding::buffer(0, param.buffer_a.as_ref(), 0, param.buffer_a.size()),
                ResourceBinding::buffer(1, param.buffer_b.as_ref(), 0, param.buffer_b.size()),
                ResourceBinding::buffer(2, param.buffer_c.as_ref(), 0, param.buffer_c.size()),
            ];
            renderer.record_compute_command(param.pipeline.as_mut(), &[&bindings], (1, 1, 1))?;

            Ok(None)
        }

        fn end_task(
            renderer: &mut dyn Renderer,
            _: Option<&mut dyn RendererTaskParam>,
        ) -> Result<Option<Box<dyn RendererTaskParam>>> {
            renderer.end_frame()?;
            Ok(None)
        }

        let shader_path = SHADERS_DIR.to_owned() + "/addition.spv";
        let shader = heph_expect_success!(Shader::from_file(shader_path));
        let mut pipeline = heph_expect_success!(renderer.create_compute_pipeline(&shader));
        drop(shader);

        let mut workers = Vec::with_capacity(n_threads);
        for index in 0..n_threads {
            let mut worker = renderer.spawn_worker();
            heph_expect_success!(
                worker
                    .work(
                        &[begin_task, create_resources_task, addition_task, end_task],
                        Some(Box::new(WorkerIndex {
                            pipeline: pipeline.clone(),
                            index,
                        })),
                        true,
                    )
                    .map_err(|(e, _)| e)
            );
            workers.push(worker);
        }

        heph_expect_success!(renderer.begin_frame());
        std::thread::sleep(std::time::Duration::from_millis(100));
        heph_expect_success!(renderer.end_frame());
        heph_expect_success!(renderer.wait_idle());

        let mut results = workers
            .iter_mut()
            .map(|worker| {
                heph_expect_success!(
                    heph_expect_success!(worker.finish()).ok_or("The worker returned no param.")
                )
            })
            .collect::<Vec<_>>();
        drop(workers);

        for result in &mut results {
            let resources = heph_expect_success!(result.downcast_mut::<AdditionResources>());

            let mut c_data = vec![0.0f32; DATA_COUNT];
            heph_expect_success!(renderer.read_buffer(
                resources.buffer_c.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut c_data)
            ));

            for j in 0..DATA_COUNT {
                assert_eq!(resources.a_data[j] + resources.b_data[j], c_data[j]);
            }

            heph_expect_success!(renderer.destroy_buffer(resources.buffer_a.as_mut()));
            heph_expect_success!(renderer.destroy_buffer(resources.buffer_b.as_mut()));
            heph_expect_success!(renderer.destroy_buffer(resources.buffer_c.as_mut()));
        }

        heph_expect_success!(renderer.destroy_compute_pipeline(pipeline.as_mut()));
    }
}
