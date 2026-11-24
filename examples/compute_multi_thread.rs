use std::sync::{Arc, Barrier};

use heph_gl::{
    Error,
    GraphicsDeviceFeature::ComputeShaders,
    Result, Shader,
    renderers::{
        BufferUsage, FeatureRequest, Renderer, RendererBuffer, RendererComputePipeline,
        RendererTaskParam, ResourceBinding,
    },
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

mod utils;

const N_THREADS: usize = 10;
const BUFFER_LEN: usize = 256;
const BUFFER_SIZE: usize = BUFFER_LEN * std::mem::size_of::<f32>();

fn create_data(thread_index: usize) -> (Vec<f32>, Vec<f32>) {
    let mut data_a = vec![0.0f32; BUFFER_LEN];
    let mut data_b = vec![0.0f32; BUFFER_LEN];
    for i in 0..BUFFER_LEN {
        data_a[i] = ((i + 10) * (thread_index + 1)) as f32;
        data_b[i] = ((i + 20) * (thread_index + 1)) as f32;
    }
    (data_a, data_b)
}

// The initial param of each worker. Every worker gets its own clone of the pipeline, and its index
// is used to create distinct data per worker. The frame barrier makes sure every worker has begun
// its frame before the main thread ends its own.
struct WorkerIndex {
    pipeline: Box<dyn RendererComputePipeline>,
    index: usize,
    frame_barrier: Arc<Barrier>,
}
impl RendererTaskParam for WorkerIndex {}

// Replaces `WorkerIndex` once the resources are created. It carries the pipeline and buffers to the
// following tasks, and is returned to the main thread when the worker finishes so the results can
// be read and the buffers freed.
struct AdditionResources {
    pipeline: Box<dyn RendererComputePipeline>,
    buffer_a: Box<dyn RendererBuffer>,
    buffer_b: Box<dyn RendererBuffer>,
    buffer_c: Box<dyn RendererBuffer>,
}
impl RendererTaskParam for AdditionResources {}

// Every worker begins its own frame. The commands it records are submitted to the GPU by the main
// thread when it ends its frame. Returning `None` passes the current param to the next task.
fn begin_task(
    renderer: &dyn Renderer,
    param: Option<&mut dyn RendererTaskParam>,
) -> Result<Option<Box<dyn RendererTaskParam>>> {
    let param = param
        .ok_or(Error::invalid_argument("Missing the worker index."))?
        .downcast_ref::<WorkerIndex>()?;

    renderer.begin_frame()?;
    let _ = param.frame_barrier.wait();
    Ok(None)
}

// Creates the buffers in the GPU and fills the inputs. Returns the resources as the new param, so
// the following tasks can use them.
fn create_resources_task(
    renderer: &dyn Renderer,
    param: Option<&mut dyn RendererTaskParam>,
) -> Result<Option<Box<dyn RendererTaskParam>>> {
    let param = param
        .ok_or(Error::invalid_argument("Missing the worker index."))?
        .downcast_ref::<WorkerIndex>()?;

    let mut buffer_a = renderer.create_buffer(BUFFER_SIZE, BufferUsage::Storage)?;
    let mut buffer_b = renderer.create_buffer(BUFFER_SIZE, BufferUsage::Storage)?;
    let buffer_c = renderer.create_buffer(BUFFER_SIZE, BufferUsage::Storage)?;

    let (a_data, b_data) = create_data(param.index);

    renderer.write_buffer(buffer_a.as_mut(), 0, bytemuck::cast_slice(&a_data))?;
    renderer.write_buffer(buffer_b.as_mut(), 0, bytemuck::cast_slice(&b_data))?;

    Ok(Some(Box::new(AdditionResources {
        pipeline: param.pipeline.clone(),
        buffer_a,
        buffer_b,
        buffer_c,
    })))
}

// Binds the buffers to the shader and records the addition. The shader takes one set of resources
// with 3 bindings (2 input buffers, and an output buffer).
fn addition_task(
    renderer: &dyn Renderer,
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

// Ends the worker's frame, so the main thread can submit its commands.
fn end_task(
    renderer: &dyn Renderer,
    _: Option<&mut dyn RendererTaskParam>,
) -> Result<Option<Box<dyn RendererTaskParam>>> {
    renderer.end_frame()?;
    Ok(None)
}

// This example processes multiple additions in a single frame.
fn example(
    renderer: &mut utils::renderer::ExampleRenderer,
    _: RawWindowHandle,
    _: RawDisplayHandle,
) {
    let device = utils::renderer::get_best_device(renderer);
    let features = [FeatureRequest::new(ComputeShaders, true)];
    renderer.set_device(Some(&device), &features).unwrap();

    let shader = Shader::from_file(format!("{}/{}", utils::SHADERS_DIR, "/addition.spv")).unwrap();
    let mut pipeline = renderer.create_compute_pipeline(&shader).unwrap();
    drop(shader);

    // Spawn the workers and start them. Each worker runs its tasks in order on its own thread, with
    // its own clone of the pipeline and index as the initial param. Since we only need a single
    // addition per worker, the tasks are executed once. The workers keep their thread resources
    // alive until they are finished, so the main thread can still submit their commands.
    let frame_barrier = Arc::new(Barrier::new(N_THREADS + 1));
    let mut workers = Vec::with_capacity(N_THREADS);
    for i in 0..N_THREADS {
        const WORK_ONCE: bool = true;
        let mut worker = renderer.spawn_worker();
        if let Err((e, _)) = worker.work(
            &[begin_task, create_resources_task, addition_task, end_task],
            Some(Box::new(WorkerIndex {
                pipeline: pipeline.clone(),
                index: i,
                frame_barrier: frame_barrier.clone(),
            })),
            WORK_ONCE,
        ) {
            panic!("{e}");
        }
        workers.push(worker);
    }

    // Workers wait for the main thread to begin its frame before they begin their own. Wait until
    // all of them have begun, so their commands are submitted when the main thread ends its frame.
    renderer.begin_frame().unwrap();
    let _ = frame_barrier.wait();
    renderer.end_frame().unwrap();

    // Wait until the graphics device finishes processing.
    renderer.wait_idle().unwrap();

    // Finish the workers now that their commands are processed. Finishing a worker releases its
    // thread-local resources and returns its last param, which holds the resources it created in
    // this case.
    let mut results = Vec::with_capacity(N_THREADS);
    for worker in &mut workers {
        results.push(worker.finish().unwrap().unwrap());
    }
    drop(workers);

    // Print the results.
    for (i, result) in results.iter().enumerate() {
        let param = result.downcast_ref::<AdditionResources>().unwrap();

        let mut a_data = vec![0.0f32; BUFFER_LEN];
        let mut b_data = vec![0.0f32; BUFFER_LEN];
        let mut c_data = vec![0.0f32; BUFFER_LEN];
        renderer
            .read_buffer(
                param.buffer_a.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut a_data),
            )
            .unwrap();
        renderer
            .read_buffer(
                param.buffer_b.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut b_data),
            )
            .unwrap();
        renderer
            .read_buffer(
                param.buffer_c.as_ref(),
                0,
                bytemuck::cast_slice_mut(&mut c_data),
            )
            .unwrap();

        let index = 0;
        println!(
            "(thread:{}) {} + {} = {}",
            i + 1,
            a_data[index],
            b_data[index],
            c_data[index]
        );
    }

    // Cleanup
    for result in &mut results {
        let param = result.downcast_mut::<AdditionResources>().unwrap();

        renderer.destroy_buffer(param.buffer_a.as_mut()).unwrap();
        renderer.destroy_buffer(param.buffer_b.as_mut()).unwrap();
        renderer.destroy_buffer(param.buffer_c.as_mut()).unwrap();
    }
    renderer
        .destroy_compute_pipeline(pipeline.as_mut())
        .unwrap();

    std::process::exit(0);
}

fn main() {
    const EXAMPLE_NAME: &str = "Compute (Multi Thread)";
    const INIT_RENDERER: bool = true;
    utils::run_example(EXAMPLE_NAME, INIT_RENDERER, example);
}
