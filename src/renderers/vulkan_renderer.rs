use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};

use ash::vk::{
    ApplicationInfo, CommandBuffer, CommandPool, DeviceQueueCreateInfo, InstanceCreateInfo,
    MemoryHeapFlags, PhysicalDeviceFeatures2, PhysicalDeviceMemoryProperties2,
    PhysicalDeviceProperties2, PhysicalDeviceType, Queue, QueueFamilyProperties2, QueueFlags,
    ShaderModuleCreateInfo, StructureType, SurfaceKHR,
};
use ash::{Entry, Instance};
use renkrs::RGB;

use crate::graphics_device::{Feature, GraphicsDevice};
use crate::renderers::{FeatureRequest, InitializeOptions, Renderer, RendererError, Settings};
use crate::shader::ShaderSource;
use crate::{HEPHGL_ENGINE_NAME, HEPHGL_ENGINE_VERSION, Version};

struct QueueFamily {
    pub index: u32,
    pub queue_count: u32,
    pub queue_flags: QueueFlags,
    pub present_supported: bool,
}

// Order of the fields matter as it determines the destruction order.
struct DeviceContext {
    pub graphics_device: GraphicsDevice,

    pub vma_allocator: vk_mem::Allocator,

    pub graphics_queue: Queue,
    pub graphics_family_index: u32,
    pub graphics_command_pool: CommandPool,
    pub graphics_command_buffers: Vec<CommandBuffer>,

    pub transfer_queue: Option<Queue>,
    pub transfer_family_index: Option<u32>,
    pub transfer_command_pool: Option<CommandPool>,
    pub transfer_command_buffers: Vec<CommandBuffer>,

    pub compute_queue: Option<Queue>,
    pub compute_family_index: Option<u32>,
    pub compute_command_pool: Option<CommandPool>,
    pub compute_command_buffers: Vec<CommandBuffer>,

    pub logical_device: ash::Device,
}

pub struct VulkanRenderer {
    settings: Settings,

    entry: Option<Entry>,
    instance: Option<Instance>,

    window_surface: Option<SurfaceKHR>,
    window_surface_loader: Option<ash::khr::surface::Instance>,

    device_queue_families: HashMap<u32, Vec<QueueFamily>>,
    device_context: Option<DeviceContext>,
}

impl Renderer for VulkanRenderer {
    type ShaderHandle = ash::vk::ShaderModule;

    fn new() -> Self {
        Self {
            settings: Settings::default(),

            entry: None,
            instance: None,

            window_surface: None,
            window_surface_loader: None,

            device_queue_families: HashMap::default(),
            device_context: None,
        }
    }

    fn get_settings(&self) -> &Settings {
        &self.settings
    }

    fn set_settings(&mut self, settings: Settings) -> Result<(), RendererError> {
        self.settings = settings;

        if self.device_context.is_some() {
            // Reallocate command buffers.
            self.create_command_buffers()?;
        }

        Ok(())
    }

    fn initialize(&mut self, options: &InitializeOptions) -> Result<(), RendererError> {
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

        self.entry = Some(unsafe {
            Entry::load().map_err(|_| {
                RendererError::FailedToInitialize(
                    "Failed to load Vulkan graphics driver library".to_owned(),
                )
            })?
        });
        self.instance = Some(unsafe {
            self.entry()
                .create_instance(&instance_create_info, None)
                .map_err(|_| {
                    RendererError::FailedToInitialize("Failed to create Vulkan Instance".to_owned())
                })?
        });

        // WSI for rendering to the native window.

        self.window_surface = Some(unsafe {
            ash_window::create_surface(
                self.entry(),
                self.instance(),
                options.display_handle,
                options.window_handle,
                None,
            )
            .map_err(|_| {
                RendererError::FailedToCreateSurface("Failed to create WSI Surface".to_owned())
            })?
        });
        self.window_surface_loader = Some(ash::khr::surface::Instance::new(
            self.entry(),
            self.instance(),
        ));

        Ok(())
    }

    fn uninitialize(&mut self) {
        self.uninitialize_device();

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
    }

    fn enumerate_devices(&mut self) -> Result<Vec<GraphicsDevice>, RendererError> {
        let mut devices = Vec::<GraphicsDevice>::new();
        let physical_devices = unsafe {
            self.instance().enumerate_physical_devices().map_err(|_| {
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
                self.instance()
                    .get_physical_device_properties2(physical_device, &mut properties2);
                self.instance().get_physical_device_memory_properties2(
                    physical_device,
                    &mut memory_properties2,
                );
                self.instance()
                    .get_physical_device_features2(physical_device, &mut physical_features2);

                let queue_family_properties2_vec_size = self
                    .instance()
                    .get_physical_device_queue_family_properties2_len(physical_device);
                queue_family_properties2_vec.resize(
                    queue_family_properties2_vec_size,
                    QueueFamilyProperties2::default(),
                );
                self.instance()
                    .get_physical_device_queue_family_properties2(
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
                self.instance()
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
                    queue_flags: queue_flags,
                    present_supported: unsafe {
                        self.window_surface_loader()
                            .get_physical_device_surface_support(
                                physical_device,
                                index as u32,
                                *self.window_surface(),
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
                device_type: device_type,
                vendor_id: device_vendor_id,
                device_id: device_id,
                api_version: device_api_version,
                driver_version: device_driver_version,
                vram: device_vram,
                supported_features: supported_features,
            });
        }

        Ok(devices)
    }

    fn get_device(&self) -> Option<GraphicsDevice> {
        if let Some(device_context) = &self.device_context {
            Some(device_context.graphics_device.clone())
        } else {
            None
        }
    }

    fn set_device(
        &mut self,
        device: &GraphicsDevice,
        requested_features: &Vec<FeatureRequest>,
    ) -> Result<(), RendererError> {
        if self.device_context.is_some() {
            self.uninitialize_device();
        }

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
            self.instance()
                .enumerate_physical_devices()
                .map_err(|_| RendererError::Fail("Failed to create logical device.".to_owned()))?
        };
        let mut vk_physical_device = None;
        for pd in physical_devices {
            let mut properties2 = ash::vk::PhysicalDeviceProperties2::default();
            unsafe {
                self.instance()
                    .get_physical_device_properties2(pd, &mut properties2);
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
        let mut physical_features2 = ash::vk::PhysicalDeviceFeatures2::default();
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

        let mut device_create_info = ash::vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&device_extension_names);
        device_create_info.p_next = &physical_features2 as *const _ as *const std::ffi::c_void;
        let logical_device = unsafe {
            self.instance()
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
        let mut transfer_family_idx = None;
        if let Some(transfer_family) = transfer_family {
            transfer_queue_handle = Some(get_next_queue(
                transfer_family.index,
                transfer_family.queue_count,
            ));
            transfer_family_idx = Some(transfer_family.index);
        }

        let mut compute_queue_handle = None;
        let mut compute_family_idx = None;
        if let Some(compute_family) = compute_family {
            compute_queue_handle = Some(get_next_queue(
                compute_family.index,
                compute_family.queue_count,
            ));
            compute_family_idx = Some(compute_family.index);
        }

        // Initialize VMA.

        let mut allocator_create_info =
            vk_mem::AllocatorCreateInfo::new(self.instance(), &logical_device, vk_physical_device);
        allocator_create_info.vulkan_api_version = VulkanRenderer::VK_API_VERSION;
        let vma_allocator = unsafe {
            vk_mem::Allocator::new(allocator_create_info)
                .map_err(|e| RendererError::Fail(format!("Failed to initialize VMA: {}", e)))?
        };

        // Create command pools.

        let graphics_pool_info = ash::vk::CommandPoolCreateInfo::default()
            .flags(ash::vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(graphics_family.index);
        let graphics_command_pool = unsafe {
            logical_device
                .create_command_pool(&graphics_pool_info, None)
                .map_err(|e| {
                    RendererError::Fail(format!("Failed to create Graphics Command Pool: {}", e))
                })?
        };

        let mut transfer_command_pool = None;
        if let Some(transfer_family) = transfer_family {
            let transfer_pool_info = ash::vk::CommandPoolCreateInfo::default()
                .flags(ash::vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(transfer_family.index);
            transfer_command_pool = Some(unsafe {
                logical_device
                    .create_command_pool(&transfer_pool_info, None)
                    .map_err(|e| {
                        RendererError::Fail(format!(
                            "Failed to create Transfer Command Pool: {}",
                            e
                        ))
                    })?
            });
        }

        let mut compute_command_pool = None;
        if let Some(compute_family) = compute_family {
            let compute_pool_info = ash::vk::CommandPoolCreateInfo::default()
                .flags(ash::vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(compute_family.index);
            compute_command_pool = Some(unsafe {
                logical_device
                    .create_command_pool(&compute_pool_info, None)
                    .map_err(|e| {
                        RendererError::Fail(format!("Failed to create Compute Command Pool: {}", e))
                    })?
            });
        }

        self.device_context = Some(DeviceContext {
            graphics_device: device.clone(),

            vma_allocator: vma_allocator,

            graphics_queue,
            graphics_family_index: graphics_family.index,
            graphics_command_pool: graphics_command_pool,
            graphics_command_buffers: Vec::default(),

            transfer_queue: transfer_queue_handle,
            transfer_family_index: transfer_family_idx,
            transfer_command_pool: transfer_command_pool,
            transfer_command_buffers: Vec::default(),

            compute_queue: compute_queue_handle,
            compute_family_index: compute_family_idx,
            compute_command_pool: compute_command_pool,
            compute_command_buffers: Vec::default(),

            logical_device: logical_device,
        });
        self.create_command_buffers()?;

        Ok(())
    }

    fn create_shader(&self, source: &ShaderSource) -> Result<Self::ShaderHandle, RendererError> {
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
            self.device_context()
                .logical_device
                .create_shader_module(&create_info, None)
                .map_err(|e| RendererError::Fail(format!("Failed to create shader module: {}", e)))
        }
    }

    fn destroy_shader(&self, shader: Self::ShaderHandle) {
        unsafe {
            self.device_context()
                .logical_device
                .destroy_shader_module(shader, None);
        }
    }

    fn clear(&mut self, color: RGB<f32>) -> Result<(), RendererError> {
        // TODO
        unimplemented!();
    }
}

impl VulkanRenderer {
    // TODO: Get this from the user.
    const VK_API_VERSION: u32 = ash::vk::make_api_version(0, 1, 4, 0);

    #[inline]
    fn entry(&self) -> &Entry {
        self.entry
            .as_ref()
            .expect("VulkanRenderer is not initialized: Missing Entry.")
    }

    #[inline]
    fn instance(&self) -> &Instance {
        self.instance
            .as_ref()
            .expect("VulkanRenderer is not initialized: Missing Instance.")
    }

    #[inline]
    fn window_surface(&self) -> &SurfaceKHR {
        self.window_surface
            .as_ref()
            .expect("VulkanRenderer is not initialized: Missing Window Surface.")
    }

    #[inline]
    fn window_surface_loader(&self) -> &ash::khr::surface::Instance {
        self.window_surface_loader
            .as_ref()
            .expect("VulkanRenderer is not initialized: Missing Window Surface Loader.")
    }

    #[inline]
    fn device_context(&self) -> &DeviceContext {
        self.device_context.as_ref().expect("Device is not set.")
    }
    #[inline]
    fn device_context_mut(&mut self) -> &mut DeviceContext {
        self.device_context.as_mut().expect("Device is not set.")
    }

    fn vk_api_version_to_heph_version(vk_api_version: u32) -> Version {
        Version {
            major: ash::vk::api_version_major(vk_api_version),
            minor: ash::vk::api_version_minor(vk_api_version),
            patch: ash::vk::api_version_patch(vk_api_version),
        }
    }

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

    fn create_command_buffers(&mut self) -> Result<(), RendererError> {
        let fif = self.settings.frames_in_flight;
        let device_context = self.device_context_mut();
        let allocate_buffers = |pool: ash::vk::CommandPool,
                                count: u32|
         -> Result<Vec<ash::vk::CommandBuffer>, RendererError> {
            let alloc_info = ash::vk::CommandBufferAllocateInfo::default()
                .command_pool(pool)
                .level(ash::vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(count);
            unsafe {
                device_context
                    .logical_device
                    .allocate_command_buffers(&alloc_info)
                    .map_err(|e| {
                        RendererError::Fail(format!("Failed to allocate command buffers: {}", e))
                    })
            }
        };

        // Free the old buffers.
        if device_context.graphics_command_buffers.len() > 0 {
            unsafe {
                let _ = device_context.logical_device.device_wait_idle();

                device_context.logical_device.free_command_buffers(
                    device_context.graphics_command_pool,
                    &device_context.graphics_command_buffers,
                );
                device_context.graphics_command_buffers.clear();

                if device_context.transfer_command_buffers.len() > 0 {
                    device_context.logical_device.free_command_buffers(
                        device_context.transfer_command_pool.unwrap(),
                        &device_context.transfer_command_buffers,
                    );
                    device_context.transfer_command_buffers.clear();
                }

                if device_context.compute_command_buffers.len() > 0 {
                    device_context.logical_device.free_command_buffers(
                        device_context.compute_command_pool.unwrap(),
                        &device_context.compute_command_buffers,
                    );
                    device_context.compute_command_buffers.clear();
                }
            }
        }

        device_context.graphics_command_buffers =
            allocate_buffers(device_context.graphics_command_pool, fif)?;

        if let Some(pool) = device_context.transfer_command_pool {
            device_context.transfer_command_buffers = allocate_buffers(pool, fif)?;
        }

        if let Some(pool) = device_context.compute_command_pool {
            device_context.compute_command_buffers = allocate_buffers(pool, fif)?;
        }

        Ok(())
    }

    fn uninitialize_device(&mut self) {
        if let Some(device_context) = self.device_context.take() {
            unsafe {
                // Wait for the GPU to finish all pending operations before uninitializing to prevent segfaults.
                let _ = device_context.logical_device.device_wait_idle();

                // Destroy the command pools
                device_context
                    .logical_device
                    .destroy_command_pool(device_context.graphics_command_pool, None);
                if let Some(transfer_command_pool) = device_context.transfer_command_pool {
                    device_context
                        .logical_device
                        .destroy_command_pool(transfer_command_pool, None);
                }
                if let Some(compute_command_pool) = device_context.compute_command_pool {
                    device_context
                        .logical_device
                        .destroy_command_pool(compute_command_pool, None);
                }

                drop(device_context.vma_allocator);
                device_context.logical_device.destroy_device(None);
            }
        }
    }
}
