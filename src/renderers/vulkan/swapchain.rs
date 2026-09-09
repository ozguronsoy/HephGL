/// Represents the Vulkan swapchain and its associated resources required for presentation.
pub struct SwapchainContext {
    /// The swapchain extension loader.
    pub loader: ash::khr::swapchain::Device,
    /// The Vulkan swapchain instance.
    pub swapchain: ash::vk::SwapchainKHR,
    /// The pixel format of the surface images.
    pub format: ash::vk::Format,
    /// The width and height of the swapchain images.
    pub extent: ash::vk::Extent2D,
    /// The Vulkan images.
    pub images: Vec<ash::vk::Image>,
    /// The views required to draw into the Vulkan images as color attachments.
    pub image_views: Vec<ash::vk::ImageView>,
    /// The semaphore used for GPU-GPU synchronization.
    pub semaphores: Vec<ash::vk::Semaphore>,
    /// The depth buffer.
    pub depth_image: ash::vk::Image,
    /// The allocation of depth buffer.
    pub depth_image_allocation: Option<vk_mem::Allocation>,
    /// The depth buffer view
    pub depth_image_view: ash::vk::ImageView,
}
