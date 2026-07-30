use heph_gl::renderers::vulkan_renderer::VulkanRenderer;
use heph_gl::renderers::{
    BufferUsage, FeatureRequest, InitializeOptions, PipelineHandle, Renderer, ResourceBinding,
    ResourceBindingType,
};
use heph_gl::shader::ShaderSource;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use renkrs::RGB;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

#[derive(Default)]
pub struct App {
    window: Option<Window>,
    renderer: Option<VulkanRenderer>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_title("HephGL - Window Sandbox")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

        self.window = Some(event_loop.create_window(window_attributes).unwrap());
        println!("OS Window created successfully.");
        self.renderer = Some(VulkanRenderer::new());
        println!("Renderer created successfully.");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Close signal received. Shutting down HephGL.");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && let Some(ref mut active_renderer) = self.renderer
                {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::KeyI) => {
                            let window = self.window.as_ref().unwrap();
                            let initialize_options = InitializeOptions {
                                app_name: "Sandbox",
                                window_handle: window.window_handle().unwrap().as_raw(),
                                display_handle: window.display_handle().unwrap().as_raw(),
                            };
                            active_renderer.initialize(&initialize_options).unwrap();
                            println!("Renderer initialized.");
                        }
                        PhysicalKey::Code(KeyCode::KeyE) => {
                            let graphics_devices = active_renderer.enumerate_devices().unwrap();
                            println!("Graphics device count: {}", graphics_devices.len());
                            for dev in &graphics_devices {
                                println!("{}", dev);
                            }
                            active_renderer
                                .set_settings(heph_gl::renderers::Settings {
                                    frames_in_flight: 1,
                                })
                                .unwrap();
                            active_renderer
                                .set_device(
                                    &graphics_devices[0],
                                    &[FeatureRequest {
                                        feature: heph_gl::graphics_device::Feature::ComputeShaders,
                                        required: true,
                                    }],
                                )
                                .unwrap();
                        }
                        PhysicalKey::Code(KeyCode::KeyR) => {
                            active_renderer
                                .clear(RGB::<f32> {
                                    r: 1.0,
                                    g: 0.0,
                                    b: 0.0,
                                })
                                .unwrap();
                        }
                        PhysicalKey::Code(KeyCode::KeyG) => {
                            active_renderer
                                .clear(RGB::<f32> {
                                    r: 0.0,
                                    g: 1.0,
                                    b: 0.0,
                                })
                                .unwrap();
                        }
                        PhysicalKey::Code(KeyCode::KeyB) => {
                            active_renderer
                                .clear(RGB::<f32> {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 1.0,
                                })
                                .unwrap();
                        }
                        PhysicalKey::Code(KeyCode::KeyC) => {
                            active_renderer.initialize_frames().unwrap();

                            let shader_path = "C:\\Users\\ozgur\\OneDrive\\Desktop\\Projects\\HephGL\\examples\\shaders\\addition.spv";
                            let shader_module = active_renderer
                                .create_shader(&ShaderSource::from_file(shader_path).unwrap())
                                .unwrap();
                            let pipeline = active_renderer
                                .create_compute_pipeline(&shader_module)
                                .unwrap();

                            for i in 1..11 {
                                active_renderer.begin_frame().unwrap();

                                let data_count = 256;
                                let byte_size = (data_count * std::mem::size_of::<f32>()) as u64;

                                let mut a_data = vec![0.0f32; data_count];
                                let mut b_data = vec![0.0f32; data_count];
                                for j in 0..data_count {
                                    a_data[j] = (j * i) as f32;
                                    b_data[j] = ((j * 2) * i) as f32;
                                }

                                let mut buffer_a = active_renderer
                                    .create_buffer(byte_size, BufferUsage::Storage)
                                    .unwrap();
                                let mut buffer_b = active_renderer
                                    .create_buffer(byte_size, BufferUsage::Storage)
                                    .unwrap();
                                let mut buffer_c = active_renderer
                                    .create_buffer(byte_size, BufferUsage::Storage)
                                    .unwrap();

                                active_renderer
                                    .write_buffer(&buffer_a, bytemuck::cast_slice(&a_data))
                                    .unwrap();
                                active_renderer
                                    .write_buffer(&buffer_b, bytemuck::cast_slice(&b_data))
                                    .unwrap();

                                let bindings = [
                                    ResourceBinding {
                                        binding: 0,
                                        resource: ResourceBindingType::Buffer {
                                            handle: buffer_a,
                                            usage: BufferUsage::Storage,
                                            offset: 0,
                                            size: buffer_a.size(),
                                        },
                                    },
                                    ResourceBinding {
                                        binding: 1,
                                        resource: ResourceBindingType::Buffer {
                                            handle: buffer_b,
                                            usage: BufferUsage::Storage,
                                            offset: 0,
                                            size: buffer_b.size(),
                                        },
                                    },
                                    ResourceBinding {
                                        binding: 2,
                                        resource: ResourceBindingType::Buffer {
                                            handle: buffer_c,
                                            usage: BufferUsage::Storage,
                                            offset: 0,
                                            size: buffer_c.size(),
                                        },
                                    },
                                ];

                                let resource_set = active_renderer
                                    .create_resource_set(
                                        &PipelineHandle::Compute(pipeline),
                                        &bindings,
                                    )
                                    .unwrap();

                                let recorded_command = active_renderer
                                    .record_compute_pass(&pipeline, &[&resource_set], (1, 1, 1))
                                    .unwrap();
                                active_renderer
                                    .submit_commands(&[recorded_command])
                                    .unwrap();

                                active_renderer.wait_idle().unwrap();

                                let mut c_data = vec![0.0f32; data_count];
                                active_renderer
                                    .read_buffer(&buffer_c, bytemuck::cast_slice_mut(&mut c_data))
                                    .unwrap();

                                {
                                    let j = 10;
                                    println!(
                                        "Result[{}] {}: {} + {} = {}",
                                        i, j, a_data[j], b_data[j], c_data[j]
                                    );
                                }

                                active_renderer.destroy_buffer(&mut buffer_a).unwrap();
                                active_renderer.destroy_buffer(&mut buffer_b).unwrap();
                                active_renderer.destroy_buffer(&mut buffer_c).unwrap();

                                active_renderer.end_frame().unwrap();
                            }
                            active_renderer.destroy_compute_pipeline(&pipeline).unwrap();
                            active_renderer.destroy_shader(&shader_module).unwrap();

                            active_renderer.uninitialize_frames().unwrap();
                        }
                        PhysicalKey::Code(KeyCode::KeyM) => {
                            active_renderer.initialize_frames().unwrap();

                            let shader_path = "C:\\Users\\ozgur\\OneDrive\\Desktop\\Projects\\HephGL\\examples\\shaders\\addition.spv";
                            let shader_module = active_renderer
                                .create_shader(&ShaderSource::from_file(shader_path).unwrap())
                                .unwrap();
                            let pipeline = active_renderer
                                .create_compute_pipeline(&shader_module)
                                .unwrap();

                            let thread_count = 5;
                            let data_count = 256;
                            let byte_size = (data_count * std::mem::size_of::<f32>()) as u64;

                            let renderer_ptr = active_renderer as *mut VulkanRenderer as usize;

                            std::thread::scope(|s| {
                                let (tx, rx) = std::sync::mpsc::channel();
                                let barrier =
                                    std::sync::Arc::new(std::sync::Barrier::new(thread_count + 1));

                                for thread_id in 0..thread_count {
                                    let tx = tx.clone();
                                    let b = barrier.clone();

                                    s.spawn(move || {
                                        let renderer =
                                            unsafe { &mut *(renderer_ptr as *mut VulkanRenderer) };

                                        renderer.initialize_frames().unwrap();

                                        for i in 1..11 {
                                            b.wait();

                                            let mut a_data = vec![0.0f32; data_count];
                                            let mut b_data = vec![0.0f32; data_count];
                                            for j in 0..data_count {
                                                a_data[j] = (j * i) as f32 + thread_id as f32;
                                                b_data[j] = ((j * 2) * i) as f32;
                                            }

                                            let buffer_a = renderer
                                                .create_buffer(byte_size, BufferUsage::Storage)
                                                .unwrap();
                                            let buffer_b = renderer
                                                .create_buffer(byte_size, BufferUsage::Storage)
                                                .unwrap();
                                            let buffer_c = renderer
                                                .create_buffer(byte_size, BufferUsage::Storage)
                                                .unwrap();

                                            renderer
                                                .write_buffer(
                                                    &buffer_a,
                                                    bytemuck::cast_slice(&a_data),
                                                )
                                                .unwrap();
                                            renderer
                                                .write_buffer(
                                                    &buffer_b,
                                                    bytemuck::cast_slice(&b_data),
                                                )
                                                .unwrap();

                                            let bindings = [
                                                ResourceBinding {
                                                    binding: 0,
                                                    resource: ResourceBindingType::Buffer {
                                                        handle: buffer_a,
                                                        usage: BufferUsage::Storage,
                                                        offset: 0,
                                                        size: buffer_a.size(),
                                                    },
                                                },
                                                ResourceBinding {
                                                    binding: 1,
                                                    resource: ResourceBindingType::Buffer {
                                                        handle: buffer_b,
                                                        usage: BufferUsage::Storage,
                                                        offset: 0,
                                                        size: buffer_b.size(),
                                                    },
                                                },
                                                ResourceBinding {
                                                    binding: 2,
                                                    resource: ResourceBindingType::Buffer {
                                                        handle: buffer_c,
                                                        usage: BufferUsage::Storage,
                                                        offset: 0,
                                                        size: buffer_c.size(),
                                                    },
                                                },
                                            ];

                                            let resource_set = renderer
                                                .create_resource_set(
                                                    &PipelineHandle::Compute(pipeline),
                                                    &bindings,
                                                )
                                                .unwrap();

                                            let recorded_command = renderer
                                                .record_compute_pass(
                                                    &pipeline,
                                                    &[&resource_set],
                                                    (1, 1, 1),
                                                )
                                                .unwrap();

                                            tx.send((
                                                thread_id,
                                                recorded_command,
                                                buffer_a,
                                                buffer_b,
                                                buffer_c,
                                                a_data,
                                                b_data,
                                                i,
                                            ))
                                            .unwrap();

                                            b.wait();
                                        }

                                        renderer.uninitialize_frames().unwrap();
                                    });
                                }

                                for _ in 1..11 {
                                    active_renderer.begin_frame().unwrap();

                                    barrier.wait();

                                    let mut commands = Vec::new();
                                    let mut cleanup_data = Vec::new();

                                    for _ in 0..thread_count {
                                        let (
                                            t_id,
                                            cmd,
                                            buf_a,
                                            buf_b,
                                            buf_c,
                                            a_data,
                                            b_data,
                                            frame_idx,
                                        ) = rx.recv().unwrap();
                                        commands.push(cmd);
                                        cleanup_data.push((
                                            t_id, buf_a, buf_b, buf_c, a_data, b_data, frame_idx,
                                        ));
                                    }

                                    active_renderer.submit_commands(&commands).unwrap();
                                    active_renderer.wait_idle().unwrap();

                                    for (
                                        t_id,
                                        mut buf_a,
                                        mut buf_b,
                                        mut buf_c,
                                        a_data,
                                        b_data,
                                        frame_idx,
                                    ) in cleanup_data
                                    {
                                        let mut c_data = vec![0.0f32; data_count];
                                        active_renderer
                                            .read_buffer(
                                                &buf_c,
                                                bytemuck::cast_slice_mut(&mut c_data),
                                            )
                                            .unwrap();

                                        let j = 10;
                                        println!(
                                            "Thread {} Result[{}] {}: {} + {} = {}",
                                            t_id, frame_idx, j, a_data[j], b_data[j], c_data[j]
                                        );

                                        active_renderer.destroy_buffer(&mut buf_a).unwrap();
                                        active_renderer.destroy_buffer(&mut buf_b).unwrap();
                                        active_renderer.destroy_buffer(&mut buf_c).unwrap();
                                    }

                                    barrier.wait();
                                    active_renderer.end_frame().unwrap();
                                }
                            });

                            active_renderer.destroy_compute_pipeline(&pipeline).unwrap();
                            active_renderer.destroy_shader(&shader_module).unwrap();

                            active_renderer.uninitialize_frames().unwrap();
                        }
                        PhysicalKey::Code(KeyCode::KeyU) => {
                            active_renderer.uninitialize().unwrap();
                            println!("Renderer uninitialized.");
                        }
                        PhysicalKey::Code(KeyCode::KeyQ) => {
                            event_loop.exit();
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    let _ = event_loop.run_app(&mut app);
}
