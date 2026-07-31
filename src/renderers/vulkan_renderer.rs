use core::panic;
use std::cell::UnsafeCell;
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicU64, Ordering};

use ash::vk::{
    ApplicationInfo, Buffer, BufferCreateInfo, BufferUsageFlags, CommandBuffer,
    CommandBufferAllocateInfo, CommandBufferBeginInfo, CommandBufferLevel, CommandBufferUsageFlags,
    CommandPool, CommandPoolCreateFlags, CommandPoolCreateInfo, ComputePipelineCreateInfo,
    DescriptorPool, DescriptorPoolCreateInfo, DescriptorPoolResetFlags, DescriptorPoolSize,
    DescriptorSet, DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo,
    DescriptorType, DeviceCreateInfo, DeviceQueueCreateInfo, Fence, Handle, InstanceCreateInfo,
    MemoryHeapFlags, PhysicalDeviceFeatures2, PhysicalDeviceMemoryProperties2,
    PhysicalDeviceProperties2, PhysicalDeviceType, Pipeline, PipelineBindPoint, PipelineCache,
    PipelineLayout, PipelineLayoutCreateInfo, PipelineShaderStageCreateInfo, Queue,
    QueueFamilyProperties2, QueueFlags, ShaderModule, ShaderModuleCreateInfo, ShaderStageFlags,
    StructureType, SubmitInfo, SurfaceKHR,
};
use ash::{Entry, Instance};
use renkrs::RGB;
use vk_mem::Alloc;

use crate::graphics_device::{Feature, GraphicsDevice};
use crate::renderers::{
    BufferUsage, FeatureRequest, InitializeOptions, PipelineHandle, Renderer, RendererError,
    RendererResult, ResourceBinding, ResourceBindingType, Settings,
};
use crate::shader::ShaderSource;
use crate::{HEPHGL_ENGINE_NAME, HEPHGL_ENGINE_VERSION, Version};

/// Indicates that the thread context index is invalid.
const INVALID_THREAD_CONTEXT_INDEX: usize = usize::MAX;
thread_local! {
    /// Index of the current `thread_context`. We use the same index for graphics, transfer, and compute.
    static THREAD_CONTEXT_INDEX: UnsafeCell<usize> = const { UnsafeCell::new(INVALID_THREAD_CONTEXT_INDEX) };
}

/// Represents a Vulkan queue family.
struct QueueFamily {
    /// The index of the graphics family.
    index: u32,
    /// The number of queues available for this family.
    queue_count: u32,
    queue_flags: QueueFlags,
    /// Indicates whether this family supports presenting to a device.
    present_supported: bool,
}

#[derive(Default)]
/// Represents the resources and synchronization state for a single frame.
struct Frame {
    /// The primary command buffer used to record commands for this frame.
    command_buffer: CommandBuffer,
    /// The fence used to synchronize CPU and GPU execution for this frame.
    fence: Fence,
    /// The descriptor pool allocated for resources used during this frame.
    descriptor_pool: DescriptorPool,
    /// Indicates whether the frame is currently executing on the GPU and has an active fence in flight.
    is_in_flight: bool,
}

/// Represents the per-thread resources used for recording commands.
#[derive(Default)]
struct ThreadContext {
    /// The command pool allocated exclusively for this thread and queue family.
    command_pool: CommandPool,
    /// Contains the resources per frame.
    ///
    /// ### Note
    /// Length of this must always be equal to `settings.frames_in_flight`.
    frames: Vec<Frame>,
}

/// Represents context and state for a Vulkan queue.
struct QueueContext {
    /// The Vulkan queue instance.
    queue: Queue,
    /// The index of the queue family.
    queue_family_index: u32,
    /// Contains the resources used for recording commands per thread.
    thread_contexts: [ThreadContext; 64],
    /// A bitmask indicating the availability of thread contexts.
    /// `0` means the context at that index is available, `1` means it is currently in use.
    ///
    /// ### Note
    /// Replace this with an array of `AtomicU64`s if more than 64 concurrent threads are required.
    thread_mask: AtomicU64,
}

#[derive(Debug, Copy, Clone)]
/// Defines the types of queues.
enum QueueType {
    Graphics,
    Transfer,
    Compute,
}

/// Encapsulates the Vulkan device state.
///
/// ### Note
/// Order of the fields matter as it determines the destruction order.
struct DeviceContext {
    /// The currently active graphics device.
    graphics_device: GraphicsDevice,

    /// The memory allocator.
    vma_allocator: vk_mem::Allocator,

    graphics_queue_context: QueueContext,
    transfer_queue_context: Option<QueueContext>,
    compute_queue_context: Option<QueueContext>,

    /// The logical Vulkan device.
    logical_device: ash::Device,
}

#[derive(Debug, Copy, Clone)]
pub struct VulkanShader {
    module: ShaderModule,
}

/// Represents a Vulkan buffer with additional information.
#[derive(Debug, Copy, Clone)]
pub struct VulkanBuffer {
    /// The Vulkan buffer.
    buffer: Buffer,
    /// The memory allocation.
    vma_allocation: vk_mem::Allocation,
    /// The size of the buffer in bytes.
    size: u64,
}

/// Represents a Vulkan graphics pipeline.
#[derive(Debug, Copy, Clone)]
pub struct VulkanGraphicsPipeline {
    // TODO
}

/// Represents a Vulkan compute pipeline.
#[derive(Debug, Copy, Clone)]
pub struct VulkanComputePipeline {
    pipeline: Pipeline,
    layout: PipelineLayout,
    descriptor_layout: DescriptorSetLayout,
}

/// Represents a resource set compatible to a specific shader.
#[derive(Debug, Copy, Clone)]
pub struct VulkanResourceSet {
    /// The Vulkan descriptor set.
    descriptor_set: DescriptorSet,
}

/// Represents a recorded Vulkan command.
#[derive(Debug, Copy, Clone)]
pub struct VulkanRecordedCommand {
    queue: Queue,
    queue_type: QueueType,
    frame_index: u32,
    thread_context_index: usize,
}

/// The Vulkan implementation of the `Renderer` trait.
pub struct VulkanRenderer {
    settings: Settings,
    current_frame_index: u32,

    entry: Option<Entry>,
    instance: Option<Instance>,

    window_surface: Option<SurfaceKHR>,
    window_surface_loader: Option<ash::khr::surface::Instance>,

    device_queue_families: HashMap<u32, Vec<QueueFamily>>,
    device_context: Option<DeviceContext>,

    main_thread_id: std::thread::ThreadId,
}

impl Renderer for VulkanRenderer {
    type ShaderHandle = VulkanShader;
    type BufferHandle = VulkanBuffer;
    type GraphicsPipelineHandle = VulkanGraphicsPipeline;
    type ComputePipelineHandle = VulkanComputePipeline;
    type ResourceSetHandle = VulkanResourceSet;
    type RecordedCommand = VulkanRecordedCommand;

    fn new() -> Self {
        Self {
            settings: Settings::default(),
            current_frame_index: 0,

            entry: None,
            instance: None,

            window_surface: None,
            window_surface_loader: None,

            device_queue_families: HashMap::default(),
            device_context: None,

            main_thread_id: std::thread::current().id(),
        }
    }

    fn get_settings(&self) -> &Settings {
        &self.settings
    }

    fn set_settings(&mut self, settings: Settings) -> RendererResult<()> {
        self.settings = settings;
        self.current_frame_index = 0;

        if self.device_context.is_some() {
            self.create_command_buffers()?;
            self.create_fences()?;
            self.create_descriptor_pools()?;
        }

        Ok(())
    }

    fn initialize(&mut self, options: &InitializeOptions) -> RendererResult<()> {
        self.main_thread_only()?;
        if self.entry.is_some() || self.instance.is_some() {
            panic!("VulkanRenderer is already initialized.");
        }

        // Create instance.

        let c_app_name =
            CString::new(options.app_name).map_err(|_| RendererError::InvalidAppName)?;
        let required_extension_names =
            ash_window::enumerate_required_extensions(options.display_handle).map_err(|_| {
                RendererError::FailedToCreateSurface(
                    "Failed to enumerate WSI extensions".to_owned(),
                )
            })?;
        let app_info = ApplicationInfo {
            s_type: StructureType::APPLICATION_INFO,
            p_engine_name: HEPHGL_ENGINE_NAME.as_ptr(),
            p_application_name: c_app_name.as_ptr(),
            application_version: ash::vk::make_api_version(
                0,
                HEPHGL_ENGINE_VERSION.major,
                HEPHGL_ENGINE_VERSION.minor,
                HEPHGL_ENGINE_VERSION.patch,
            ),
            api_version: VulkanRenderer::VK_API_VERSION,
            ..Default::default()
        };
        let instance_create_info = InstanceCreateInfo {
            s_type: StructureType::INSTANCE_CREATE_INFO,
            p_application_info: &app_info,
            enabled_extension_count: required_extension_names.len() as u32,
            pp_enabled_extension_names: required_extension_names.as_ptr(),
            ..Default::default()
        };

        let entry = unsafe {
            Entry::load().map_err(|_| {
                RendererError::FailedToInitialize(
                    "Failed to load Vulkan graphics driver library".to_owned(),
                )
            })?
        };
        let instance = unsafe {
            entry
                .create_instance(&instance_create_info, None)
                .map_err(|_| {
                    RendererError::FailedToInitialize("Failed to create Vulkan Instance".to_owned())
                })?
        };

        // WSI for rendering to the native window.

        self.window_surface = Some(unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                options.display_handle,
                options.window_handle,
                None,
            )
            .map_err(|_| {
                RendererError::FailedToCreateSurface("Failed to create WSI Surface".to_owned())
            })?
        });
        self.window_surface_loader = Some(ash::khr::surface::Instance::new(&entry, &instance));

        self.entry = Some(entry);
        self.instance = Some(instance);

        Ok(())
    }

    fn uninitialize(&mut self) -> RendererResult<()> {
        self.main_thread_only()?;
        self.uninitialize_device()?;

        if let (Some(surface), Some(surface_loader)) =
            (self.window_surface, self.window_surface_loader.as_ref())
        {
            unsafe {
                surface_loader.destroy_surface(surface, None);
            }
        }

        if let Some(instance) = self.instance.as_ref() {
            unsafe {
                instance.destroy_instance(None);
            }
        }

        self.window_surface_loader = None;
        self.window_surface = None;
        self.instance = None;
        self.entry = None;

        Ok(())
    }

    fn enumerate_devices(&mut self) -> RendererResult<Vec<GraphicsDevice>> {
        let instance = self
            .instance
            .as_ref()
            .ok_or(RendererError::InvalidOperation(
                "Renderer is not initialized".to_string(),
            ))?;
        let window_surface_loader =
            self.window_surface_loader
                .as_ref()
                .ok_or(RendererError::InvalidOperation(
                    "Renderer is not initialize.".to_string(),
                ))?;
        let window_surface =
            self.window_surface
                .as_ref()
                .ok_or(RendererError::InvalidOperation(
                    "Renderer is not initialize.".to_string(),
                ))?;

        let mut devices = Vec::<GraphicsDevice>::new();
        let physical_devices = unsafe {
            instance.enumerate_physical_devices().map_err(|_| {
                RendererError::FailedToEnumerateDevices(
                    "Failed to enumerate physical Vulkan devices.".to_owned(),
                )
            })?
        };

        for physical_device in physical_devices {
            let mut properties2 = PhysicalDeviceProperties2::default();
            let mut memory_properties2 = PhysicalDeviceMemoryProperties2::default();
            let mut physical_features2 = PhysicalDeviceFeatures2::default();
            let mut queue_family_properties2_vec = Vec::<QueueFamilyProperties2>::default();
            unsafe {
                instance.get_physical_device_properties2(physical_device, &mut properties2);
                instance.get_physical_device_memory_properties2(
                    physical_device,
                    &mut memory_properties2,
                );
                instance.get_physical_device_features2(physical_device, &mut physical_features2);

                let queue_family_properties2_vec_size =
                    instance.get_physical_device_queue_family_properties2_len(physical_device);
                queue_family_properties2_vec.resize(
                    queue_family_properties2_vec_size,
                    QueueFamilyProperties2::default(),
                );
                instance.get_physical_device_queue_family_properties2(
                    physical_device,
                    &mut queue_family_properties2_vec,
                );
            };

            let device_name = unsafe {
                CStr::from_ptr(properties2.properties.device_name.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            };

            let device_type = match properties2.properties.device_type {
                PhysicalDeviceType::DISCRETE_GPU => crate::graphics_device::Type::DiscreteGpu,
                PhysicalDeviceType::INTEGRATED_GPU => crate::graphics_device::Type::IntegratedGpu,
                PhysicalDeviceType::VIRTUAL_GPU => crate::graphics_device::Type::VirtualGpu,
                PhysicalDeviceType::CPU => crate::graphics_device::Type::Cpu,
                PhysicalDeviceType::OTHER => crate::graphics_device::Type::Other,
                _ => crate::graphics_device::Type::Invalid,
            };

            let device_vendor_id = properties2.properties.vendor_id;
            let device_id = properties2.properties.device_id;
            self.device_queue_families.insert(device_id, Vec::default());

            let device_api_version =
                VulkanRenderer::vk_api_version_to_heph_version(properties2.properties.api_version);
            let device_driver_version = VulkanRenderer::vk_driver_version_to_heph_version(
                GraphicsDevice::vendor_from_id(properties2.properties.vendor_id),
                properties2.properties.driver_version,
            );

            // VRAM is the sum of the sizes of all DEVICE_LOCAL heaps
            let mut device_vram: u64 = 0;
            let heap_count = memory_properties2.memory_properties.memory_heap_count as usize;
            for heap in memory_properties2.memory_properties.memory_heaps[..heap_count].iter() {
                if heap.flags.contains(MemoryHeapFlags::DEVICE_LOCAL) {
                    device_vram += heap.size;
                }
            }

            let mut supported_features = HashSet::<crate::graphics_device::Feature>::default();
            let extension_properties = unsafe {
                instance
                    .enumerate_device_extension_properties(physical_device)
                    .map_err(|_| {
                        RendererError::FailedToEnumerateSupportedFeatures(
                            "Failed to enumerate the supported device features.".to_owned(),
                        )
                    })?
            };
            if physical_features2.features.geometry_shader == ash::vk::TRUE {
                supported_features.insert(crate::graphics_device::Feature::GeometryShaders);
            }
            if physical_features2.features.fill_mode_non_solid == ash::vk::TRUE {
                supported_features.insert(crate::graphics_device::Feature::WireframeMode);
            }
            if physical_features2.features.wide_lines == ash::vk::TRUE {
                supported_features.insert(crate::graphics_device::Feature::WideLines);
            }
            if physical_features2.features.sampler_anisotropy == ash::vk::TRUE {
                supported_features.insert(crate::graphics_device::Feature::AnisotropicFiltering);
            }
            for ext in extension_properties {
                let name = unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) };
                if name.to_string_lossy() == "VK_KHR_ray_tracing_pipeline" {
                    supported_features.insert(crate::graphics_device::Feature::RayTracing);
                }
            }
            for (index, queue_family_properties2) in queue_family_properties2_vec.iter().enumerate()
            {
                let queue_flags = queue_family_properties2.queue_family_properties.queue_flags;
                if queue_flags.contains(QueueFlags::COMPUTE) {
                    supported_features.insert(crate::graphics_device::Feature::ComputeShaders);
                }
                if queue_flags.contains(QueueFlags::VIDEO_DECODE_KHR) {
                    supported_features.insert(crate::graphics_device::Feature::VideoDecoding);
                }
                if queue_flags.contains(QueueFlags::VIDEO_ENCODE_KHR) {
                    supported_features.insert(crate::graphics_device::Feature::VideoEncoding);
                }
                if queue_flags.contains(QueueFlags::OPTICAL_FLOW_NV) {
                    supported_features.insert(crate::graphics_device::Feature::OpticalFlow);
                }
                // Vulkan guarantees that the main graphics family will also support
                // TRANSFER. Thus, we only consider families that support TRANSFER
                // but do not support GRAPHICS to be dedicated async (DMA) transfer queues.
                if queue_flags.contains(QueueFlags::TRANSFER)
                    && !queue_flags.contains(QueueFlags::GRAPHICS)
                {
                    supported_features.insert(crate::graphics_device::Feature::AsyncTransfer);
                }

                let queue_family = QueueFamily {
                    index: index as u32,
                    queue_count: queue_family_properties2.queue_family_properties.queue_count,
                    queue_flags,
                    present_supported: unsafe {
                        window_surface_loader
                            .get_physical_device_surface_support(
                                physical_device,
                                index as u32,
                                *window_surface,
                            )
                            .map_err(|_| {
                                RendererError::FailedToEnumerateSupportedFeatures(
                                    "Failed to query presentation support for queue family"
                                        .to_owned(),
                                )
                            })?
                    },
                };
                self.device_queue_families
                    .entry(device_id)
                    .and_modify(|queue_families| queue_families.push(queue_family));
            }

            devices.push(GraphicsDevice {
                name: device_name,
                device_type,
                vendor_id: device_vendor_id,
                device_id,
                api_version: device_api_version,
                driver_version: device_driver_version,
                vram: device_vram,
                supported_features,
            });
        }

        Ok(devices)
    }

    fn get_device(&self) -> Option<&GraphicsDevice> {
        self.device_context
            .as_ref()
            .map(|device_context| &device_context.graphics_device)
    }

    fn set_device(
        &mut self,
        device: &GraphicsDevice,
        requested_features: &[FeatureRequest],
    ) -> RendererResult<()> {
        self.main_thread_only()?;
        if self.device_context.is_some() {
            self.uninitialize_device()?;
        }

        let instance = self
            .instance
            .as_ref()
            .ok_or(RendererError::InvalidOperation(
                "Renderer is not initialized".to_string(),
            ))?;

        // Create logical device and queues.

        let mut available_features = Vec::<Feature>::default();
        for requested_feature in requested_features {
            if device
                .supported_features
                .contains(&requested_feature.feature)
            {
                available_features.push(requested_feature.feature);
            } else if requested_feature.required {
                return Err(RendererError::UnsupportedRequiredFeature(
                    requested_feature.feature,
                ));
            }
        }

        let queue_families = self
            .device_queue_families
            .get(&device.device_id)
            .ok_or_else(|| {
                RendererError::InvalidOperation(
                    "Provided device is not properly enumerated.".to_string(),
                )
            })?;
        let mut queue_family_queue_counts: HashMap<u32, u32> = HashMap::new();
        let mut request_queue = |family_index: u32, max_queues: u32| {
            let count = queue_family_queue_counts.entry(family_index).or_insert(0);
            if *count < max_queues {
                *count += 1;
            }
        };

        let graphics_family = queue_families
            .iter()
            .find(|f| f.queue_flags.contains(QueueFlags::GRAPHICS) && f.present_supported)
            .ok_or_else(|| {
                RendererError::InvalidOperation(
                    "No queue family supports both graphics and presentation.".to_string(),
                )
            })?;
        request_queue(graphics_family.index, graphics_family.queue_count);

        // Use DMA Transfer family if available. Otherwise, use the main graphics family.
        let mut transfer_family = None;
        if available_features.contains(&Feature::AsyncTransfer) {
            let transfer_family = transfer_family.insert(
                queue_families
                    .iter()
                    .find(|f| {
                        f.queue_flags.contains(QueueFlags::TRANSFER)
                            && !f.queue_flags.contains(QueueFlags::GRAPHICS)
                            && !f.queue_flags.contains(QueueFlags::COMPUTE)
                    })
                    .or_else(|| {
                        queue_families.iter().find(|f| {
                            f.queue_flags.contains(QueueFlags::TRANSFER)
                                && !f.queue_flags.contains(QueueFlags::GRAPHICS)
                        })
                    })
                    .unwrap_or(graphics_family),
            );
            request_queue(transfer_family.index, transfer_family.queue_count);
        }

        // Use the pure compute family if available. Otherwise, use the main graphics family.
        let mut compute_family = None;
        if available_features.contains(&Feature::ComputeShaders) {
            let compute_family = compute_family.insert(
                queue_families
                    .iter()
                    .find(|f| {
                        f.queue_flags.contains(QueueFlags::COMPUTE)
                            && !f.queue_flags.contains(QueueFlags::GRAPHICS)
                    })
                    .unwrap_or(graphics_family),
            );
            request_queue(compute_family.index, compute_family.queue_count);
        }

        let queue_setup_data: Vec<(u32, Vec<f32>)> = queue_family_queue_counts
            .into_iter()
            .map(|(index, count)| (index, vec![1.0; count as usize]))
            .collect();
        let mut queue_create_infos = Vec::new();
        for (family_index, priorities) in &queue_setup_data {
            let create_info = DeviceQueueCreateInfo::default()
                .queue_family_index(*family_index)
                .queue_priorities(priorities);
            queue_create_infos.push(create_info);
        }

        let physical_devices = unsafe {
            instance
                .enumerate_physical_devices()
                .map_err(|_| RendererError::Fail("Failed to create logical device.".to_owned()))?
        };
        let mut vk_physical_device = None;
        for pd in physical_devices {
            let mut properties2 = PhysicalDeviceProperties2::default();
            unsafe {
                instance.get_physical_device_properties2(pd, &mut properties2);
            }
            if properties2.properties.device_id == device.device_id {
                vk_physical_device = Some(pd);
                break;
            }
        }
        let vk_physical_device = vk_physical_device.ok_or_else(|| {
            RendererError::InvalidArgument(format!(
                "Graphics device with the id {:#06X} not found.",
                device.device_id
            ))
        })?;

        let device_extension_names = [ash::vk::KHR_SWAPCHAIN_NAME.as_ptr()];
        let mut physical_features2 = PhysicalDeviceFeatures2::default();
        if available_features.contains(&Feature::GeometryShaders) {
            physical_features2.features.geometry_shader = ash::vk::TRUE;
        }
        if available_features.contains(&Feature::WireframeMode) {
            physical_features2.features.fill_mode_non_solid = ash::vk::TRUE;
        }
        if available_features.contains(&Feature::WideLines) {
            physical_features2.features.wide_lines = ash::vk::TRUE;
        }
        if available_features.contains(&Feature::AnisotropicFiltering) {
            physical_features2.features.sampler_anisotropy = ash::vk::TRUE;
        }

        let mut device_create_info = DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_extension_names);
        device_create_info.p_next = &physical_features2 as *const _ as *const std::ffi::c_void;
        let logical_device = unsafe {
            instance
                .create_device(vk_physical_device, &device_create_info, None)
                .map_err(|e| {
                    RendererError::Fail(format!("Failed to create Vulkan logical device: {}", e))
                })?
        };

        let mut extracted_queue_counts: HashMap<u32, u32> = HashMap::new();
        let mut get_next_queue = |family_index: u32, max_queues: u32| {
            let queue_index = extracted_queue_counts.entry(family_index).or_insert(0);
            let queue = unsafe { logical_device.get_device_queue(family_index, *queue_index) };
            *queue_index = (*queue_index + 1) % max_queues;
            queue
        };

        let graphics_queue = get_next_queue(graphics_family.index, graphics_family.queue_count);

        let mut transfer_queue_handle = None;
        if let Some(transfer_family) = transfer_family {
            transfer_queue_handle = Some(get_next_queue(
                transfer_family.index,
                transfer_family.queue_count,
            ));
        }

        let mut compute_queue_handle = None;
        if let Some(compute_family) = compute_family {
            compute_queue_handle = Some(get_next_queue(
                compute_family.index,
                compute_family.queue_count,
            ));
        }

        // Initialize VMA.

        let mut allocator_create_info =
            vk_mem::AllocatorCreateInfo::new(instance, &logical_device, vk_physical_device);
        allocator_create_info.vulkan_api_version = VulkanRenderer::VK_API_VERSION;
        let vma_allocator = unsafe {
            vk_mem::Allocator::new(allocator_create_info)
                .map_err(|e| RendererError::Fail(format!("Failed to initialize VMA: {}", e)))?
        };

        self.device_context = Some(DeviceContext {
            graphics_device: device.clone(),

            vma_allocator,

            graphics_queue_context: QueueContext {
                queue: graphics_queue,
                queue_family_index: graphics_family.index,
                thread_contexts: std::array::from_fn(|_| ThreadContext::default()),
                thread_mask: AtomicU64::new(0),
            },
            transfer_queue_context: transfer_queue_handle.map(|queue| QueueContext {
                queue,
                queue_family_index: transfer_family.unwrap().index,
                thread_contexts: std::array::from_fn(|_| ThreadContext::default()),
                thread_mask: AtomicU64::new(0),
            }),
            compute_queue_context: compute_queue_handle.map(|queue| QueueContext {
                queue,
                queue_family_index: compute_family.unwrap().index,
                thread_contexts: std::array::from_fn(|_| ThreadContext::default()),
                thread_mask: AtomicU64::new(0),
            }),

            logical_device,
        });

        Ok(())
    }

    fn initialize_frames(&mut self) -> RendererResult<()> {
        self.uninitialize_frames()?;

        let device_context =
            self.device_context
                .as_mut()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;

        THREAD_CONTEXT_INDEX.with(|cell| unsafe {
            let index = cell.get();

            let mask = &device_context.graphics_queue_context.thread_mask;
            let mut current = mask.load(Ordering::Relaxed);
            *index = loop {
                let new_index = if self.main_thread_id == std::thread::current().id() {
                    0
                } else {
                    // Index `0` is reserved for the main thread, which might not be initialized yet.
                    // Force the LSB to 1 to prevent assigning index `0` to a worker thread.
                    (current | 1).trailing_ones() as usize
                };

                if new_index >= 64 {
                    break INVALID_THREAD_CONTEXT_INDEX;
                }

                let new = current | (1 << new_index);
                match mask.compare_exchange_weak(current, new, Ordering::AcqRel, Ordering::Acquire)
                {
                    Ok(_) => break new_index as usize,
                    Err(actual) => current = actual,
                }
            };

            if *index == INVALID_THREAD_CONTEXT_INDEX {
                Err(RendererError::Fail(
                    "Failed to initialize frames: maximum threads reached".to_string(),
                ))
            } else {
                Ok(())
            }
        })?;

        let thread_context_index = Self::thread_context_index()?;

        let create_command_pool = |family_index: u32| {
            let pool_info = CommandPoolCreateInfo::default()
                .flags(CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(family_index);
            unsafe {
                let command_pool = device_context
                    .logical_device
                    .create_command_pool(&pool_info, None)
                    .map_err(|e| {
                        RendererError::Fail(format!("Failed to create command pool: {}", e))
                    })?;
                Ok(command_pool)
            }
        };
        device_context.graphics_queue_context.thread_contexts[thread_context_index].command_pool =
            create_command_pool(device_context.graphics_queue_context.queue_family_index)?;
        if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
            transfer_queue_context.thread_contexts[thread_context_index].command_pool =
                create_command_pool(transfer_queue_context.queue_family_index)?;
        }
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            compute_queue_context.thread_contexts[thread_context_index].command_pool =
                create_command_pool(compute_queue_context.queue_family_index)?;
        }

        self.create_frames()?;

        Ok(())
    }

    fn uninitialize_frames(&mut self) -> RendererResult<()> {
        let Ok(thread_context_index) = Self::thread_context_index() else {
            // Err means the index is set to `INVALID_THREAD_CONTEXT_INDEX`, which means the thread is already uninitialized.
            return Ok(());
        };

        if self.device_context.as_ref().is_none_or(|ctx| {
            ctx.graphics_queue_context.thread_contexts[thread_context_index]
                .command_pool
                .is_null()
        }) {
            // Already uninitialized.
            return Ok(());
        }

        self.destroy_fences()?;
        self.destroy_descriptor_pools()?;
        self.destroy_command_buffers()?;

        let device_context =
            self.device_context
                .as_mut()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;

        THREAD_CONTEXT_INDEX.with(|cell| unsafe {
            let index = cell.get();

            let mask = &device_context.graphics_queue_context.thread_mask;
            mask.fetch_and(!(1 << *index), Ordering::Release);
            *index = INVALID_THREAD_CONTEXT_INDEX;
        });

        device_context.graphics_queue_context.thread_contexts[thread_context_index]
            .frames
            .clear();
        if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
            transfer_queue_context.thread_contexts[thread_context_index]
                .frames
                .clear();
        }
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            compute_queue_context.thread_contexts[thread_context_index]
                .frames
                .clear();
        }

        unsafe {
            // Destroy the command pools
            device_context.logical_device.destroy_command_pool(
                device_context.graphics_queue_context.thread_contexts[thread_context_index]
                    .command_pool,
                None,
            );
            device_context.graphics_queue_context.thread_contexts[thread_context_index]
                .command_pool = CommandPool::default();
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                device_context.logical_device.destroy_command_pool(
                    transfer_queue_context.thread_contexts[thread_context_index].command_pool,
                    None,
                );
                transfer_queue_context.thread_contexts[thread_context_index].command_pool =
                    CommandPool::default();
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                device_context.logical_device.destroy_command_pool(
                    compute_queue_context.thread_contexts[thread_context_index].command_pool,
                    None,
                );
                compute_queue_context.thread_contexts[thread_context_index].command_pool =
                    CommandPool::default();
            }
        }

        Ok(())
    }

    fn create_shader(&self, source: &ShaderSource) -> RendererResult<Self::ShaderHandle> {
        let device_context =
            self.device_context
                .as_ref()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;

        let (prefix, code_u32, suffix) = unsafe { source.data.align_to::<u32>() };

        // Data is not aligned properly.
        if !prefix.is_empty() || !suffix.is_empty() {
            return Err(RendererError::Fail(format!(
                "Shader data from '{}' is not valid SPIR-V (not 4-byte aligned).",
                source.file_path
            )));
        }

        let create_info = ShaderModuleCreateInfo::default().code(code_u32);
        unsafe {
            let shader_module = device_context
                .logical_device
                .create_shader_module(&create_info, None)
                .map_err(|e| {
                    RendererError::Fail(format!("Failed to create shader module: {}", e))
                })?;
            Ok(Self::ShaderHandle {
                module: shader_module,
            })
        }
    }

    fn destroy_shader(&self, shader: &Self::ShaderHandle) -> RendererResult<()> {
        let device_context =
            self.device_context
                .as_ref()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;

        unsafe {
            device_context
                .logical_device
                .destroy_shader_module(shader.module, None);
        }
        Ok(())
    }

    fn create_resource_set(
        &self,
        pipeline_handle: &PipelineHandle<Self::GraphicsPipelineHandle, Self::ComputePipelineHandle>,
        bindings: &[ResourceBinding<Self::BufferHandle>],
    ) -> RendererResult<Self::ResourceSetHandle> {
        match pipeline_handle {
            PipelineHandle::Compute(pipeline) => {
                for binding in bindings {
                    match binding.resource {
                        ResourceBindingType::Buffer {
                            handle,
                            offset,
                            size,
                            ..
                        } => {
                            if (offset + size) > handle.size {
                                return Err(RendererError::InvalidArgument(
                                    "Buffer overflow when binding resources.".to_string(),
                                ));
                            }
                        }
                    }
                }

                let device_context =
                    self.device_context
                        .as_ref()
                        .ok_or(RendererError::InvalidOperation(
                            "Device is not set.".to_string(),
                        ))?;
                let compute_queue_context = device_context.compute_queue_context.as_ref().ok_or(
                    RendererError::InvalidOperation(
                        "Device is not initialized with `ComputeShaders` feature.".to_string(),
                    ),
                )?;
                let thread_context_index = Self::thread_context_index()?;
                let current_frame = &compute_queue_context.thread_contexts[thread_context_index]
                    .frames[self.current_frame_index as usize];
                let descriptor_pool = current_frame.descriptor_pool;

                let alloc_info = ash::vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(std::slice::from_ref(&pipeline.descriptor_layout));

                let descriptor_set = unsafe {
                    device_context
                        .logical_device
                        .allocate_descriptor_sets(&alloc_info)
                        .map_err(|e| RendererError::Fail(e.to_string()))?[0]
                };

                let buffer_infos: Vec<_> = bindings
                    .iter()
                    .map(|binding| match &binding.resource {
                        ResourceBindingType::Buffer {
                            handle,
                            offset,
                            size,
                            ..
                        } => ash::vk::DescriptorBufferInfo::default()
                            .buffer(handle.buffer)
                            .offset(*offset)
                            .range(*size),
                    })
                    .collect();

                let writes: Vec<_> = buffer_infos
                    .iter()
                    .enumerate()
                    .map(|(i, info)| {
                        ash::vk::WriteDescriptorSet::default()
                            .dst_set(descriptor_set)
                            .dst_binding(i as u32)
                            // Note: You can dynamically map this by checking `binding.usage`
                            // if you need UNIFORM_BUFFER support later.
                            .descriptor_type(ash::vk::DescriptorType::STORAGE_BUFFER)
                            .buffer_info(std::slice::from_ref(info))
                    })
                    .collect();

                // 5. Submit the Update
                unsafe {
                    device_context
                        .logical_device
                        .update_descriptor_sets(&writes, &[]);
                }

                // 6. Return the Handle wrapper
                Ok(Self::ResourceSetHandle { descriptor_set })
            }
            _ => unimplemented!(),
        }
    }

    fn create_buffer(&self, size: u64, usage: BufferUsage) -> RendererResult<Self::BufferHandle> {
        let device_context =
            self.device_context
                .as_ref()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;

        let vk_usage = match usage {
            BufferUsage::Storage => BufferUsageFlags::STORAGE_BUFFER,
            BufferUsage::Uniform => BufferUsageFlags::UNIFORM_BUFFER,
            BufferUsage::Vertex => BufferUsageFlags::VERTEX_BUFFER,
            BufferUsage::Index => BufferUsageFlags::INDEX_BUFFER,
        };

        let buffer_info = BufferCreateInfo::default().size(size).usage(vk_usage);
        let alloc_info = vk_mem::AllocationCreateInfo {
            usage: vk_mem::MemoryUsage::Auto,
            flags: vk_mem::AllocationCreateFlags::MAPPED
                | vk_mem::AllocationCreateFlags::HOST_ACCESS_RANDOM,
            ..Default::default()
        };
        let (buffer, vma_allocation) = unsafe {
            device_context
                .vma_allocator
                .create_buffer(&buffer_info, &alloc_info)
                .map_err(|e| RendererError::Fail(format!("VMA Allocation failed: {}", e)))?
        };

        Ok(VulkanBuffer {
            buffer,
            vma_allocation,
            size,
        })
    }

    fn write_buffer(&self, buffer: &Self::BufferHandle, data: &[u8]) -> RendererResult<()> {
        if data.len() as u64 > buffer.size {
            return Err(RendererError::Fail("Data exceeds buffer size!".to_string()));
        }

        let device_context =
            self.device_context
                .as_ref()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;

        unsafe {
            let alloc_info = device_context
                .vma_allocator
                .get_allocation_info(&buffer.vma_allocation);
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                alloc_info.mapped_data as *mut u8,
                data.len(),
            );
        }

        Ok(())
    }

    fn read_buffer(&self, buffer: &Self::BufferHandle, dest: &mut [u8]) -> RendererResult<()> {
        if dest.len() as u64 > buffer.size {
            return Err(RendererError::Fail(
                "Destination slice is larger than buffer!".to_string(),
            ));
        }

        let device_context =
            self.device_context
                .as_ref()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;

        unsafe {
            let alloc_info = device_context
                .vma_allocator
                .get_allocation_info(&buffer.vma_allocation);
            std::ptr::copy_nonoverlapping(
                alloc_info.mapped_data as *const u8,
                dest.as_mut_ptr(),
                dest.len(),
            );
        }

        Ok(())
    }

    fn destroy_buffer(&self, buffer: &mut Self::BufferHandle) -> RendererResult<()> {
        let device_context =
            self.device_context
                .as_ref()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;

        unsafe {
            device_context
                .vma_allocator
                .destroy_buffer(buffer.buffer, &mut buffer.vma_allocation);
        }
        Ok(())
    }

    fn create_compute_pipeline(
        &self,
        shader: &Self::ShaderHandle,
    ) -> RendererResult<Self::ComputePipelineHandle> {
        let device_context =
            self.device_context
                .as_ref()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;

        let bindings = (0..4)
            .map(|i| {
                DescriptorSetLayoutBinding::default()
                    .binding(i)
                    .descriptor_type(DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(ShaderStageFlags::COMPUTE)
            })
            .collect::<Vec<_>>();

        let layout_info = DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_layout = unsafe {
            device_context
                .logical_device
                .create_descriptor_set_layout(&layout_info, None)
                .map_err(|e| {
                    RendererError::Fail(format!("Failed to create the descriptor layer: {}", e))
                })?
        };

        let pipeline_layout_info = PipelineLayoutCreateInfo::default()
            .set_layouts(std::slice::from_ref(&descriptor_layout));
        let layout = unsafe {
            device_context
                .logical_device
                .create_pipeline_layout(&pipeline_layout_info, None)
                .map_err(|e| {
                    RendererError::Fail(format!("Failed to create the pipeline layout: {}", e))
                })?
        };

        let entry_name =
            std::ffi::CString::new("main").map_err(|e| RendererError::Fail(e.to_string()))?;
        let stage_info = PipelineShaderStageCreateInfo::default()
            .stage(ShaderStageFlags::COMPUTE)
            .module(shader.module)
            .name(&entry_name);

        let compute_info = ComputePipelineCreateInfo::default()
            .layout(layout)
            .stage(stage_info);
        let pipeline = unsafe {
            device_context
                .logical_device
                .create_compute_pipelines(PipelineCache::null(), &[compute_info], None)
                .map_err(|e| {
                    RendererError::Fail(format!("Failed to create the compute pipeline: {}", e.1))
                })?[0]
        };

        Ok(VulkanComputePipeline {
            pipeline,
            layout,
            descriptor_layout,
        })
    }

    fn destroy_compute_pipeline(
        &self,
        pipeline: &Self::ComputePipelineHandle,
    ) -> RendererResult<()> {
        let device_context =
            self.device_context
                .as_ref()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;

        unsafe {
            device_context
                .logical_device
                .destroy_pipeline(pipeline.pipeline, None);
            device_context
                .logical_device
                .destroy_pipeline_layout(pipeline.layout, None);
            device_context
                .logical_device
                .destroy_descriptor_set_layout(pipeline.descriptor_layout, None);
        }
        Ok(())
    }

    fn record_compute_pass(
        &mut self,
        pipeline: &Self::ComputePipelineHandle,
        resource_sets: &[&Self::ResourceSetHandle],
        group_count: (u32, u32, u32),
    ) -> RendererResult<Self::RecordedCommand> {
        let device_context =
            self.device_context
                .as_mut()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;
        let compute_queue_context = device_context.compute_queue_context.as_mut().ok_or(
            RendererError::InvalidOperation(
                "Device is not initialized with `ComputeShaders` feature.".to_string(),
            ),
        )?;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame = &mut compute_queue_context.thread_contexts[thread_context_index].frames
            [self.current_frame_index as usize];

        let begin_info =
            CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        let mapped_sets: Vec<ash::vk::DescriptorSet> =
            resource_sets.iter().map(|set| set.descriptor_set).collect();
        unsafe {
            device_context
                .logical_device
                .begin_command_buffer(current_frame.command_buffer, &begin_info)
                .map_err(|e| RendererError::Fail(e.to_string()))?;
            device_context.logical_device.cmd_bind_pipeline(
                current_frame.command_buffer,
                PipelineBindPoint::COMPUTE,
                pipeline.pipeline,
            );
            device_context.logical_device.cmd_bind_descriptor_sets(
                current_frame.command_buffer,
                PipelineBindPoint::COMPUTE,
                pipeline.layout,
                0,
                &mapped_sets,
                &[],
            );
            device_context.logical_device.cmd_dispatch(
                current_frame.command_buffer,
                group_count.0,
                group_count.1,
                group_count.2,
            );
            device_context
                .logical_device
                .end_command_buffer(current_frame.command_buffer)
                .map_err(|e| RendererError::Fail(e.to_string()))?;
        }

        Ok(Self::RecordedCommand {
            queue: compute_queue_context.queue,
            queue_type: QueueType::Compute,
            frame_index: self.current_frame_index,
            thread_context_index,
        })
    }

    fn submit_commands(
        &mut self,
        recorded_commands: &[Self::RecordedCommand],
    ) -> RendererResult<()> {
        self.main_thread_only()?;

        let device_context =
            self.device_context
                .as_mut()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;

        for recorded_command in recorded_commands {
            let frame_index = recorded_command.frame_index as usize;
            let frame = match recorded_command.queue_type {
                QueueType::Graphics => {
                    &mut device_context.graphics_queue_context.thread_contexts
                        [recorded_command.thread_context_index]
                        .frames[frame_index]
                }
                QueueType::Transfer => {
                    &mut device_context
                        .transfer_queue_context
                        .as_mut()
                        .ok_or(RendererError::InvalidOperation(
                            "Device is not initialized with `AsyncTransfer` feature.".to_string(),
                        ))?
                        .thread_contexts[recorded_command.thread_context_index]
                        .frames[frame_index]
                }
                QueueType::Compute => {
                    &mut device_context
                        .compute_queue_context
                        .as_mut()
                        .ok_or(RendererError::InvalidOperation(
                            "Device is not initialized with `ComputeShaders` feature.".to_string(),
                        ))?
                        .thread_contexts[recorded_command.thread_context_index]
                        .frames[frame_index]
                }
            };
            let submit_info =
                SubmitInfo::default().command_buffers(std::slice::from_ref(&frame.command_buffer));
            unsafe {
                device_context
                    .logical_device
                    .queue_submit(recorded_command.queue, &[submit_info], frame.fence)
                    .map_err(|e| {
                        RendererError::Fail(format!("Failed to submit recorded commands: {}", e))
                    })?;
            }
            frame.is_in_flight = true;
        }
        Ok(())
    }

    fn begin_frame(&mut self) -> RendererResult<()> {
        let device_context =
            self.device_context
                .as_ref()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;
        let thread_context_index = Self::thread_context_index()?;
        let current_frame_index = self.current_frame_index as usize;

        // Wait for fences.
        let mut fences = Vec::with_capacity(3);
        if device_context.graphics_queue_context.thread_contexts[thread_context_index].frames
            [current_frame_index]
            .is_in_flight
        {
            fences.push(
                device_context.graphics_queue_context.thread_contexts[thread_context_index].frames
                    [current_frame_index]
                    .fence,
            );
        }
        if let Some(transfer_queue_context) = &device_context.transfer_queue_context
            && transfer_queue_context.thread_contexts[thread_context_index].frames
                [current_frame_index]
                .is_in_flight
        {
            fences.push(
                transfer_queue_context.thread_contexts[thread_context_index].frames
                    [current_frame_index]
                    .fence,
            );
        }
        if let Some(compute_queue_context) = &device_context.compute_queue_context
            && compute_queue_context.thread_contexts[thread_context_index].frames
                [current_frame_index]
                .is_in_flight
        {
            fences.push(
                compute_queue_context.thread_contexts[thread_context_index].frames
                    [current_frame_index]
                    .fence,
            );
        }
        unsafe {
            device_context
                .logical_device
                .wait_for_fences(&fences, true, u64::MAX)
                .map_err(|e| RendererError::Fail(e.to_string()))?;
            device_context
                .logical_device
                .reset_fences(&fences)
                .map_err(|e| RendererError::Fail(e.to_string()))?;
        }

        // Reset the command buffers.
        let reset_command_buffer = |queue_context: &QueueContext| unsafe {
            device_context
                .logical_device
                .reset_command_buffer(
                    queue_context.thread_contexts[thread_context_index].frames[current_frame_index]
                        .command_buffer,
                    ash::vk::CommandBufferResetFlags::empty(),
                )
                .map_err(|e| RendererError::Fail(e.to_string()))?;
            Ok(())
        };
        reset_command_buffer(&device_context.graphics_queue_context)?;
        if let Some(transfer_queue_context) = &device_context.transfer_queue_context {
            reset_command_buffer(transfer_queue_context)?;
        }
        if let Some(compute_queue_context) = &device_context.compute_queue_context {
            reset_command_buffer(compute_queue_context)?;
        }

        // Reset the descriptor pools.
        let reset_descriptor_pool = |queue_context: &QueueContext| unsafe {
            device_context
                .logical_device
                .reset_descriptor_pool(
                    queue_context.thread_contexts[thread_context_index].frames[current_frame_index]
                        .descriptor_pool,
                    DescriptorPoolResetFlags::empty(),
                )
                .map_err(|e| RendererError::Fail(e.to_string()))?;
            Ok(())
        };
        reset_descriptor_pool(&device_context.graphics_queue_context)?;
        if let Some(transfer_queue_context) = &device_context.transfer_queue_context {
            reset_descriptor_pool(transfer_queue_context)?;
        }
        if let Some(compute_queue_context) = &device_context.compute_queue_context {
            reset_descriptor_pool(compute_queue_context)?;
        }

        Ok(())
    }

    fn end_frame(&mut self) -> RendererResult<()> {
        self.current_frame_index = (self.current_frame_index + 1) % self.settings.frames_in_flight;
        Ok(())
    }

    fn wait_idle(&self) -> RendererResult<()> {
        let device_context =
            self.device_context
                .as_ref()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;

        unsafe {
            device_context
                .logical_device
                .device_wait_idle()
                .map_err(|e| RendererError::Fail(format!("Wait idle failed: {}", e)))?;
        }
        Ok(())
    }

    fn clear(&mut self, _color: RGB<f32>) -> RendererResult<()> {
        // TODO
        unimplemented!();
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        if let Err(e) = self.uninitialize() {
            eprintln!("Failed to uninitialize renderer on drop: {}", e);
        }
    }
}

impl VulkanRenderer {
    /// The Vulkan API version used internally.
    // TODO: Get this from the user.
    const VK_API_VERSION: u32 = ash::vk::make_api_version(0, 1, 4, 0);

    /// Checks whether the call is being made from the main thread. If not, returns an error.
    fn main_thread_only(&self) -> RendererResult<()> {
        (self.main_thread_id == std::thread::current().id())
            .then_some(())
            .ok_or_else(|| {
                RendererError::InvalidOperation(
                    "This action can only be performed in the main thread.".to_string(),
                )
            })
    }

    fn thread_context_index() -> RendererResult<usize> {
        let index = THREAD_CONTEXT_INDEX.with(|cell| unsafe { *cell.get() });
        if index == INVALID_THREAD_CONTEXT_INDEX {
            Err(RendererError::InvalidOperation(
                "Frames are uninitialized. `initialize_frames` must \
                be called on this thread before access."
                    .to_string(),
            ))
        } else {
            Ok(index)
        }
    }

    /// Converts the Vulkan API version to `crate::Version`.
    fn vk_api_version_to_heph_version(vk_api_version: u32) -> Version {
        Version {
            major: ash::vk::api_version_major(vk_api_version),
            minor: ash::vk::api_version_minor(vk_api_version),
            patch: ash::vk::api_version_patch(vk_api_version),
        }
    }

    /// Converts the Vulkan driver version to `crate::Version`.
    fn vk_driver_version_to_heph_version(
        vk_vendor: crate::graphics_device::Vendor,
        vk_driver_version: u32,
    ) -> Version {
        match vk_vendor {
            crate::graphics_device::Vendor::Nvidia => Version {
                major: (vk_driver_version >> 22) & 0x3FF,
                minor: (vk_driver_version >> 14) & 0x0FF,
                patch: (vk_driver_version >> 6) & 0x0FF,
            },

            crate::graphics_device::Vendor::Intel => Version {
                major: vk_driver_version >> 14,
                minor: vk_driver_version & 0x3FFF,
                patch: 0,
            },

            _ => Version {
                major: ash::vk::api_version_major(vk_driver_version),
                minor: ash::vk::api_version_minor(vk_driver_version),
                patch: ash::vk::api_version_patch(vk_driver_version),
            },
        }
    }

    fn create_frames(&mut self) -> RendererResult<()> {
        let device_context =
            self.device_context
                .as_mut()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;
        let create_frames = |queue_context: &mut QueueContext| {
            queue_context.thread_contexts[thread_context_index]
                .frames
                .resize_with(fif, Frame::default);
        };
        create_frames(&mut device_context.graphics_queue_context);
        if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
            create_frames(transfer_queue_context);
        }
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            create_frames(compute_queue_context);
        }
        self.create_command_buffers()?;
        self.create_fences()?;
        self.create_descriptor_pools()?;
        Ok(())
    }

    /// Creates a command buffer for each frame.
    fn create_command_buffers(&mut self) -> RendererResult<()> {
        self.destroy_command_buffers()?;

        let device_context =
            self.device_context
                .as_mut()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;

        let allocate_buffers = |queue_context: &mut QueueContext| {
            let alloc_info = CommandBufferAllocateInfo::default()
                .command_pool(queue_context.thread_contexts[thread_context_index].command_pool)
                .level(CommandBufferLevel::PRIMARY)
                .command_buffer_count(self.settings.frames_in_flight);
            unsafe {
                let command_buffers = device_context
                    .logical_device
                    .allocate_command_buffers(&alloc_info)
                    .map_err(|e| {
                        RendererError::Fail(format!("Failed to allocate command buffers: {}", e))
                    })?;
                for (frame_index, command_buffer) in command_buffers.iter().enumerate().take(fif) {
                    queue_context.thread_contexts[thread_context_index].frames[frame_index]
                        .command_buffer = *command_buffer;
                }
            }
            Ok(())
        };

        allocate_buffers(&mut device_context.graphics_queue_context)?;
        if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
            allocate_buffers(transfer_queue_context)?;
        }
        if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
            allocate_buffers(compute_queue_context)?;
        }

        Ok(())
    }

    /// Creates a fence for each frame.
    fn create_fences(&mut self) -> RendererResult<()> {
        self.destroy_fences()?;

        let device_context =
            self.device_context
                .as_mut()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;

        let fence_info =
            ash::vk::FenceCreateInfo::default().flags(ash::vk::FenceCreateFlags::SIGNALED);
        let create_fence = |queue_context: &mut QueueContext, frame_index: usize| unsafe {
            queue_context.thread_contexts[thread_context_index].frames[frame_index].fence =
                device_context
                    .logical_device
                    .create_fence(&fence_info, None)
                    .map_err(|e| RendererError::Fail(format!("Failed to create fences: {}", e)))?;
            Ok(())
        };

        for frame_index in 0..fif {
            create_fence(&mut device_context.graphics_queue_context, frame_index)?;
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                create_fence(transfer_queue_context, frame_index)?;
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                create_fence(compute_queue_context, frame_index)?;
            }
        }

        Ok(())
    }

    /// Creates descriptor pools for each frame.
    fn create_descriptor_pools(&mut self) -> RendererResult<()> {
        const DESCRIPTOR_COUNT: u32 = 1000;
        const DESCRIPTOR_MAX_SETS: u32 = 1000;

        self.destroy_descriptor_pools()?;

        let device_context =
            self.device_context
                .as_mut()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;

        let pool_sizes = [
            DescriptorPoolSize::default()
                .ty(DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(DESCRIPTOR_COUNT),
            DescriptorPoolSize::default()
                .ty(DescriptorType::STORAGE_BUFFER)
                .descriptor_count(DESCRIPTOR_COUNT),
        ];

        let pool_info = DescriptorPoolCreateInfo::default()
            .max_sets(DESCRIPTOR_MAX_SETS)
            .pool_sizes(&pool_sizes);

        let allocate_pool = |queue_context: &mut QueueContext, frame_index: usize| unsafe {
            queue_context.thread_contexts[thread_context_index].frames[frame_index]
                .descriptor_pool = device_context
                .logical_device
                .create_descriptor_pool(&pool_info, None)
                .map_err(|e| {
                    RendererError::Fail(format!("Failed to create descriptor pool: {}", e))
                })?;
            Ok(())
        };

        for frame_index in 0..fif {
            allocate_pool(&mut device_context.graphics_queue_context, frame_index)?;
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                allocate_pool(transfer_queue_context, frame_index)?;
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                allocate_pool(compute_queue_context, frame_index)?;
            }
        }

        Ok(())
    }

    /// Frees the resources used by the graphics device, and sets the currently active device to `None`.
    fn uninitialize_device(&mut self) -> RendererResult<()> {
        if self.device_context.is_some() {
            self.uninitialize_frames()?;
        }

        if let Some(device_context) = self.device_context.take() {
            unsafe {
                drop(device_context.vma_allocator);
                device_context.logical_device.destroy_device(None);
            }
        }

        Ok(())
    }

    /// Destroys the command buffers if there are any.
    fn destroy_command_buffers(&mut self) -> RendererResult<()> {
        let device_context =
            self.device_context
                .as_mut()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;

        let destroy_command_buffer = |queue_context: &mut QueueContext, frame_index: usize| unsafe {
            let frame =
                &mut queue_context.thread_contexts[thread_context_index].frames[frame_index];
            device_context.logical_device.free_command_buffers(
                queue_context.thread_contexts[thread_context_index].command_pool,
                &[frame.command_buffer],
            );
            frame.command_buffer = CommandBuffer::default();
        };

        unsafe {
            device_context
                .logical_device
                .device_wait_idle()
                .map_err(|e| RendererError::Fail(e.to_string()))?
        };
        for frame_index in 0..fif {
            destroy_command_buffer(&mut device_context.graphics_queue_context, frame_index);
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                destroy_command_buffer(transfer_queue_context, frame_index);
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                destroy_command_buffer(compute_queue_context, frame_index);
            }
        }

        Ok(())
    }

    /// Destroys the fences if there are any.
    fn destroy_fences(&mut self) -> RendererResult<()> {
        let device_context =
            self.device_context
                .as_mut()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;

        let destroy_fence = |queue_context: &mut QueueContext, frame_index: usize| unsafe {
            let frame =
                &mut queue_context.thread_contexts[thread_context_index].frames[frame_index];
            if frame.is_in_flight {
                device_context
                    .logical_device
                    .wait_for_fences(&[frame.fence], true, u64::MAX)
                    .map_err(|e| {
                        RendererError::Fail(format!("Failed to wait for fences: {}", e))
                    })?;
                device_context
                    .logical_device
                    .destroy_fence(frame.fence, None);
                frame.fence = Fence::default();
            }
            Ok(())
        };

        for frame_index in 0..fif {
            destroy_fence(&mut device_context.graphics_queue_context, frame_index)?;
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                destroy_fence(transfer_queue_context, frame_index)?;
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                destroy_fence(compute_queue_context, frame_index)?;
            }
        }

        Ok(())
    }

    /// Destroys the descriptor pools if there are any.
    fn destroy_descriptor_pools(&mut self) -> RendererResult<()> {
        let device_context =
            self.device_context
                .as_mut()
                .ok_or(RendererError::InvalidOperation(
                    "Device is not set.".to_string(),
                ))?;
        let fif = self.settings.frames_in_flight as usize;
        let thread_context_index = Self::thread_context_index()?;

        let destroy_desc_pool = |queue_context: &mut QueueContext, frame_index: usize| unsafe {
            let frame =
                &mut queue_context.thread_contexts[thread_context_index].frames[frame_index];
            device_context
                .logical_device
                .destroy_descriptor_pool(frame.descriptor_pool, None);
            frame.descriptor_pool = DescriptorPool::default();
        };

        for frame_index in 0..fif {
            destroy_desc_pool(&mut device_context.graphics_queue_context, frame_index);
            if let Some(transfer_queue_context) = &mut device_context.transfer_queue_context {
                destroy_desc_pool(transfer_queue_context, frame_index);
            }
            if let Some(compute_queue_context) = &mut device_context.compute_queue_context {
                destroy_desc_pool(compute_queue_context, frame_index);
            }
        }

        Ok(())
    }
}

impl VulkanBuffer {
    pub fn size(&self) -> u64 {
        self.size
    }
}
