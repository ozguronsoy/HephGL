#![allow(dead_code)]

pub mod renderer;

use heph_gl::renderers::{InitializeOptions, LATEST_API_VERSION, Msaa, Renderer, Settings};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::utils::renderer::ExampleRenderer;

pub type ExampleFn = fn(&mut ExampleRenderer, RawWindowHandle, RawDisplayHandle) -> ();

pub const SHADERS_DIR: &str = "shaders";

struct App {
    name: String,
    example: ExampleFn,
    init_renderer: bool,
    window: Option<Window>,
    renderer: Option<ExampleRenderer>,
}

impl App {
    fn new(name: &str, init_renderer: bool, example: ExampleFn) -> Self {
        Self {
            name: name.to_owned(),
            example,
            init_renderer,
            window: None,
            renderer: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_size = winit::dpi::LogicalSize::new(1280.0, 720.0);
        let mut window_attributes = Window::default_attributes()
            .with_title(format!("HephGL - {}", self.name))
            .with_inner_size(window_size);

        if let Some(monitor) = event_loop.primary_monitor() {
            let monitor_position = monitor.position();
            let monitor_size = monitor.size();
            let scale_factor = monitor.scale_factor();
            let window_width = window_size.width * scale_factor;
            let window_height = window_size.height * scale_factor;
            let x = monitor_position.x
                + ((f64::from(monitor_size.width) - window_width) / 2.0).round() as i32;
            let y = monitor_position.y
                + ((f64::from(monitor_size.height) - window_height) / 2.0).round() as i32;
            window_attributes =
                window_attributes.with_position(winit::dpi::PhysicalPosition::new(x, y));
        }

        let window = event_loop.create_window(window_attributes).unwrap();
        let window_handle = window.window_handle().unwrap().as_raw();
        let display_handle = window.display_handle().unwrap().as_raw();

        let mut renderer = ExampleRenderer::new();

        if self.init_renderer {
            let init_options = InitializeOptions {
                app_name: &self.name,
                window_handle,
                display_handle,
                api_version: LATEST_API_VERSION,
            };
            renderer.initialize(&init_options).unwrap();
            renderer
                .set_settings(Settings {
                    frames_in_flight: 1,
                    vsync: true,
                    stereoscopic_3d_rendering: false,
                    default_size: (1920, 1080),
                    msaa: Msaa::X1,
                })
                .unwrap();
        }

        (self.example)(&mut renderer, window_handle, display_handle);

        self.window = Some(window);
        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if event.physical_key == PhysicalKey::Code(KeyCode::KeyQ) {
                    event_loop.exit();
                }
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => (),
        }
    }
}

pub fn load_exr(path: &str, channel_names: [&str; 3]) -> image::RgbaImage {
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

pub fn run_example(name: &str, init_renderer: bool, example: ExampleFn) {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new(name, init_renderer, example);
    let _ = event_loop.run_app(&mut app);
}
