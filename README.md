# HephGL

- [Introduction](#introduction)
- [Setup](#setup)
    - [Install](#install)
    - [Dependencies](#dependencies)
- [Usage](#usage)
    - [Running Examples](#running-examples)
    - [Running Tests](#running-tests)
- [Examples](examples/)
- [Documentation](https://docs.rs/heph-gl/0.1.0/heph_gl/)
- [Contributing](#contributing)
- [License](#license)


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

HephGL uses `nightly` for formatting
```bash
rustup toolchain install nightly --component rustfmt
```

HephGL uses `nextest` for integration tests since running them with standard `cargo test` results in
failure due to window creation in worker threads.
```bash
cargo install cargo-nextest --locked
```

All other dependencies are stated in the `Cargo.toml` file, and will be fetched automatically during
build.

## Usage

### Running Examples

Running an example using Vulkan
```bash
cargo run --example <example_name>
```

Running an example using a different backend
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

Running the integration tests
```bash
cargo nextest run
```

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
