# Examples

Start with renderer setup and device enumeration, then move on to compute and rendering. Within
each category, examples progress from basic usage to more advanced resource management and
animation.

Run an example from the repository root using its filename without `.rs`:

```bash
cargo run --example render_basic_triangle
```

The shared [utilities](utils/mod.rs) create the window and renderer, and initialize the renderer
for every example except `creating_renderer`, which demonstrates initialization explicitly. The
flowcharts describe reusable library workflows. Compute and rendering workflows assume an
initialized renderer; device selection, feature requests, and renderer settings are grouped as
renderer configuration.

## Creation, initialization, and device enumeration

### [creating_renderer](creating_renderer.rs)

Creates and initializes a renderer, then selects a graphics device and the features your
application needs. Initialization connects the renderer to your windowing system.

```mermaid
flowchart LR
    A[Create renderer] --> B[Configure initialization options and window handles]
    B --> C[Initialize renderer]
    C --> D[Select graphics device and request features]
    D --> E[Renderer ready to use]
```

### [enumerate_devices](enumerate_devices.rs)

Discovers available graphics devices and prints their capabilities.

```mermaid
flowchart LR
    A[Initialized renderer] --> B[Enumerate graphics devices]
    B --> C[Print device information]
```

## Compute

### [compute_single_thread_single_frame](compute_single_thread_single_frame.rs)

Executes a GPU computation in a single frame. Creates a compute pipeline, binds IO resources,
records work, and waits for completion before reading results on the CPU.

```mermaid
flowchart LR
    A[Configure renderer] --> B[Load compute shader and create pipeline]
    B --> C[Begin frame]
    C --> D[Create IO buffers and upload inputs]
    D --> E[Bind resources and record compute commands]
    E --> F[End frame to submit work]
    F --> G[Wait for GPU completion and read results]
    G --> H[Release resources]
```

### [compute_single_thread_multi_frame](compute_single_thread_multi_frame.rs)

Keeps multiple frames in flight using a reusable set of resources for each frame slot. Reads
completed results before reusing a slot, then collects pending results when processing finishes.

```mermaid
flowchart TD
    A[Configure renderer] --> B[Create compute pipeline]
    B --> C[Allocate resources for each frame in flight]
    C --> D[Begin frame and acquire reusable frame resources]
    D --> E[Read previous results if available]
    E --> F[Update inputs, bind resources, and record compute commands]
    F --> G[End frame to submit work]
    G --> H{More work?}
    H -- Yes --> D
    H -- No --> I[Collect remaining results as frames complete]
    I --> J[Release resources]
```

### [compute_multi_thread](compute_multi_thread.rs)

Records GPU computations concurrently using renderer workers. Shares a renderer handle, prepares
resources on worker threads, and coordinates frame completion and resource lifetimes with the main
thread before consuming results.

```mermaid
flowchart TD
    A[Configure renderer and create compute pipeline] --> B[Share renderer handle and spawn workers]
    B --> C[Each worker begins a frame]
    C --> D[Prepare resources and record compute commands]
    D --> E[End worker frame and hand off resource handles]
    E --> F[Keep worker resources alive until GPU completion]
    B --> G[Main thread begins a frame]
    E --> H[Synchronize with workers]
    G --> H
    H --> I[End main frame and wait for GPU]
    I --> J[Allow workers to finish]
    F --> J
    J --> K[Read results and release resources]
```

## Rendering

### [render_basic_triangle](render_basic_triangle.rs)

Demonstrates the basic rendering lifecycle: creates a graphics pipeline from shaders, begins a
frame, clears the render target, records a draw, and ends the frame. Shader-generated geometry
keeps resource setup minimal.

```mermaid
flowchart LR
    A[Configure renderer] --> B[Load shaders and create pipeline]
    B --> C[Begin frame and clear]
    C --> D[Record draw command]
    D --> E[End frame]
    E --> F[Wait for GPU completion and release resources]
```

### [render_shapes](render_shapes.rs)

Uses primitive topology to control how vertices form points, lines, and triangles. Creates
pipelines with the appropriate topology and combines their draw commands in one frame.

```mermaid
flowchart LR
    A[Configure renderer] --> B[Choose shaders and primitive topologies]
    B --> P[Create graphics pipelines]
    P --> C[Begin frame and clear]
    C --> D[Record a draw for each shape]
    D --> E[End frame]
    E --> F[Wait for GPU completion and release resources]
```

### [render_moving_cubes](render_moving_cubes.rs)

Renders animated geometry using vertex and index buffers. Updates vertex data each frame, binds the
buffers for indexed drawing, and configures pipeline options such as culling and blending.

```mermaid
flowchart TD
    A[Configure renderer and graphics pipeline] --> B[Create vertex and index buffers]
    B --> D[Begin frame and clear]
    D --> E[Update geometry]
    E --> F[Bind buffers and record indexed draws]
    F --> G[End frame]
    G --> C{Animation finished?}
    C -- Yes --> H[Wait for GPU completion and release resources]
    C -- No --> D
```

### [render_textures](render_textures.rs)

Applies textures and material properties to geometry. Uploads image data to the GPU, configures
sampling, and binds textures alongside geometry and material uniforms when recording draws.

```mermaid
flowchart TD
    A[Configure renderer and graphics pipeline] --> B[Read images and prepare geometry/material data]
    B --> D[Create buffers and sampler]
    D --> E[Upload textures to the GPU within a frame]
    E --> F[Begin render frame and clear]
    F --> G[Bind buffers, textures, and sampler and record draw]
    G --> H[End frame]
    H --> I[Wait for GPU completion and release resources]
```

### [render_text](render_text.rs)

Generates renderable text from a font for screen-space, world-space, or extruded rendering.
Prepares text geometry and a font atlas where needed, then uses suitable pipelines and transforms
to draw text within a scene.

```mermaid
flowchart TD
    A[Configure renderer] --> B[Create pipelines for the chosen text types]
    B --> C[Load font and generate text vertex buffers]
    C --> D[Prepare text resources and upload font atlas where needed]
    D --> E[Configure camera and text transforms]
    E --> G[Begin frame, clear, and update transforms]
    G --> H[Bind text resources and record draws]
    H --> I[End frame]
    I --> F{Animation finished?}
    F -- No --> G
    F -- Yes --> J[Wait for GPU completion and release resources]
```

### [render_meshes](render_meshes.rs)

Loads a mesh and renders its parts with their geometry, textures, and materials. Uses a camera and
object transforms to place the mesh in a scene, and stores transform updates in per-frame
resources.

```mermaid
flowchart TD
    A[Configure renderer and graphics pipeline] --> B[Load mesh]
    B --> C[Configure object transforms, camera, and lighting]
    C --> D[Upload mesh geometry, textures, and materials]
    D --> E[Prepare samplers and per-frame transform resources]
    E --> G[Begin frame, clear, and update transforms]
    G --> H[Bind each part's resources and record indexed draws]
    H --> I[End frame]
    I --> F{Animation finished?}
    F -- No --> G
    F -- Yes --> J[Wait for GPU completion and release resources]
```

### [render_animated_meshes](render_animated_meshes.rs)

Animates a skinned mesh using a skeleton and animation clips. Samples an animation at the current
playback time, computes joint matrices, and updates GPU resources so the vertex shader can deform
the mesh.

```mermaid
flowchart TD
    A[Configure renderer and skinning pipeline] --> B[Load mesh, skeleton, and animations]
    B --> C[Configure object transforms, camera, and lighting]
    C --> D[Prepare mesh resources and per-frame joint buffers]
    D --> E[Begin frame and clear]
    E --> G[Sample animation at the current playback time]
    G --> H[Compute joint matrices and update joint buffer]
    H --> I[Bind resources and record skinned indexed draw]
    I --> J[End frame]
    J --> F{Animation finished?}
    F -- No --> E
    F -- Yes --> K[Wait for GPU completion and release resources]
```
