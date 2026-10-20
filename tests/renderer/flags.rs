macro_rules! define_renderer_test_flags {
    (
        $($tfn:ident),* $(,)?
    ) => {
        paste::paste! {
            #[derive(Default, Clone, Copy)]
            pub struct RendererTestFlags {
                /// Skips all tests.
                pub skip_all_tests: bool,

                $(
                    /// Skips testing this feature.
                    pub [<skip_ $tfn>]: bool,
                    /// Indicates whether the feature is not yet implemented, and tests should fail
                    /// with a `todo!()`.
                    pub [<todo_ $tfn>]: bool,
                    /// Indicates whether the feature is not supported by this renderer, and tests
                    /// should fail with an `unimplemented!()`.
                    pub [<unimplemented_ $tfn>]: bool,
                )*
            }
        }
    };
}

define_renderer_test_flags!(
    test_invalid_app_name,
    test_initialize_renderer,
    test_enumerate_devices,
    test_set_device,
    test_set_settings,
    test_uniform_buffer,
    test_storage_buffer,
    test_index_buffer,
    test_vertex_buffer,
    test_impossible_buffer_size,
    test_begin_twice,
    test_end_without_begin,
    test_resize_window,
    test_calling_main_thread_only_fn_from_worker_thread,
    test_multiple_workers_per_thread,
    test_workers_begin_before_main,
    test_excess_threads,
    test_single_threaded_compute_discrete_gpu,
    test_single_threaded_compute_integrated_gpu,
    test_single_threaded_compute_cpu,
    test_single_threaded_compute_virtual_gpu,
    test_single_threaded_compute_other,
    test_multi_threaded_compute_discrete_gpu,
    test_multi_threaded_compute_integrated_gpu,
    test_multi_threaded_compute_cpu,
    test_multi_threaded_compute_virtual_gpu,
    test_multi_threaded_compute_other,
    test_render_static_shapes,
    test_render_dynamic_cubes,
    test_render_textures,
    test_render_text,
    test_render_meshes,
    test_render_animated_meshes
);
