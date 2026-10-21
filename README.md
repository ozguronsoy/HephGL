# HephGL

- [Introduction](#introduction)
- [Setup](#setup)
    - [Install](#install)
    - [Dependencies](#dependencies)
- [Usage](#usage)
    - [Environment Variables](#environment-variables)
    - [Running Examples](#running-examples)
    - [Running Tests](#running-tests)
    - [Renderer](#renderer)
    - [Graphics](#graphics)
- [Examples](examples/)
- [Documentation](https://docs.rs/heph-gl/0.1.0/heph_gl/)
- [Contributing](#contributing)
- [License](LICENSE)


## Introduction

HephGL is an experimental cross-platform graphics library created for the sole purpose of me
learning Rust and graphics programming.

Goals:

- Multiple graphics backends (e.g., Vulkan, DirectX) with runtime backend switching.
- Clean, simple, low boilerplate, and easy to use API that is agnostic to the used backend.
- Thread-safe design.
- High performance and low memory consumption.
- Seamless integration with any windowing library.

## Setup

### Install

If you want to use HephGL in your project, run the following command
```bash
cargo add heph-gl
```

or add `heph-gl = <version>` to your `Cargo.toml`.

If you want to work on HephGL, simply clone this repository.

### Dependencies

clippy
```bash
rustup component add clippy
```

`nightly` is used for formatting
```bash
rustup toolchain install nightly --component rustfmt
```

`nextest` is used for integration tests since running them with standard `cargo test` results in
failure due to window creation in worker threads.
```bash
cargo install cargo-nextest --locked
```

`llvm-cov` is used for generating code coverage reports
```bash
cargo install cargo-llvm-cov --locked
```

All other dependencies are stated in the `Cargo.toml` file, and will be fetched automatically
during build.

## Usage

### Environment Variables

HephGL supports compile-time configuration through optional environment variables:
- `HEPHGL_RENDERER_MAX_CONCURRENT_THREADS=<usize>`: The maximum number of threads a renderer can concurrently run.

### Running Examples

Running an example using the default backend (Vulkan)
```bash
cargo run --example <example_name>
```

Running an example using a specific backend
```bash
cargo run --example <example_name> --no-default-features --features <backend_name>
```

Backends:
- vulkan
- directx
- metal
- opengl

> [!NOTE]
> Currently only Vulkan is supported.

### Running Tests

Running the unit tests
```bash
cargo test
```

Running the integration tests
```bash
cargo nextest run
```

Generating code coverage reports
```bash
cargo llvm-cov nextest
```

### Renderer

The `Renderer` trait provides a clean and simple API that is agnostic to the backend it uses.  This
enables us to switch between graphics APIs without changing the code. Renderers must be initialized
before we can start utilizing them.

After initialization and device selection, rendering and compute work follow the same basic
lifecycle: load shaders, create a pipeline, and record commands between `begin_frame` and
`end_frame`. Ending the frame submits the recorded work. Reuse the pipeline across frames, then
wait for the GPU before releasing its resources.

```mermaid
flowchart TD
    A[Create and initialize renderer] --> B[Select graphics device]
    B --> C[Load shaders and create pipeline]
    C --> D[Begin frame]
    D --> E[Prepare resources and record commands]
    E --> F[End frame]
    F --> G{More frames?}
    G -- Yes --> D
    G -- No --> H[Wait for GPU completion and release resources]
```

The following example renders a basic triangle using the Vulkan backend. Replace the placeholders
with the corresponding raw handles from your chosen windowing library. Shader paths are relative to
the repository root, these shaders generate the triangle's vertices without vertex buffers.

```rust
use heph_gl::{
    renderers::{
        Renderer,
        settings::{GraphicsPipelineOptions, InitializeOptions},
        vulkan::VulkanRenderer,
    },
    shader::Shader,
};
use renkrs::RGB;

let mut renderer = VulkanRenderer::new();
renderer.initialize(&InitializeOptions {
        app_name: "Basic Triangle",
        window_handle: <raw_window_handle>,
        display_handle: <raw_display_handle>,
        api_version: VulkanRenderer::LATEST_API_VERSION,
    }).unwrap();
renderer.set_device(VulkanRenderer::DEFAULT_DEVICE, &[]).unwrap();

let vert_shader = Shader::from_file("shaders/shapes/basic_triangle_vert.spv").unwrap();
let frag_shader = Shader::from_file("shaders/shapes/basic_triangle_frag.spv").unwrap();
let pipeline = renderer.create_graphics_pipeline(
        &[&vert_shader, &frag_shader],
        &GraphicsPipelineOptions::default(),
    ).unwrap();
drop(vert_shader);
drop(frag_shader);

renderer.begin_frame().unwrap();
renderer.clear(RGB::default()).unwrap();
// Three vertices, one instance.
renderer.record_graphics_command(&pipeline, &[], 3, 1).unwrap();
renderer.end_frame().unwrap();

renderer.wait_idle().unwrap();
renderer.destroy_graphics_pipeline(&pipeline).unwrap();
```

Check out the [examples directory](examples/) for more examples and flowcharts.

### Graphics

While the `Renderer` trait successfully abstracts the underlying backend, using it directly still
requires a fair amount of boilerplate for pipeline creation, resource management, command
recording, and multithreaded synchronization. The `Graphics` struct aims to provide a higher-level,
simplified API that allows you to easily render sprites or execute computations without writing all
of this boilerplate code.

You might wonder why the `Renderer` is still exposed directly if the `Graphics` struct is easier to
use. The `Graphics` struct is ideal for rapid development and applications that are not strictly
performance-critical. However, by retaining direct access to the `Renderer`, HephGL ensures that
developers can bypass the higher-level abstractions to fine-tune and optimize the rendering
pipeline specifically for their use cases.

> [!NOTE]
> The `Graphics` struct is not implemented yet.

## Contributing

All types of contributions are welcome. Feel free to open a PR, issue, or discussion.

HephGL uses `prek` for code formatting and linting. Follow the steps below to setup `prek`.

First install `prek`
```bash
cargo install prek --locked
```

To automatically run the required checks before every commit, install the pre-commit hook. Run this
command at the root of the repository
```bash
prek install
```

You can also run `prek` manually if you don't want to install the hook
```bash
prek
```

## License

Licensed under the BSD-3-Clause License.
