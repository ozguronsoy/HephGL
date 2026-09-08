use crate::renderers::{
    RendererHandle, RendererResult, RendererWorker, RendererWorkerFactory, vulkan::VulkanRenderer,
};

impl RendererWorkerFactory<VulkanRenderer> for RendererHandle<VulkanRenderer> {
    fn spawn_worker(&self) -> RendererResult<RendererWorker<VulkanRenderer>> {
        let renderer = unsafe { &mut *(self.p_renderer as *mut VulkanRenderer) };
        renderer.initialize_thread()?;
        Ok(RendererWorker::<VulkanRenderer> {
            p_renderer: self.p_renderer,
            uninitialize: |renderer| renderer.uninitialize_thread(),
            _marker: std::marker::PhantomData,
        })
    }
}
